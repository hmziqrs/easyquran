use super::*;
use ruxlog_types::PaginatedList;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, DeleteResult, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Process-global in-memory route cache (replaces the prior Redis SETs). Keyed
/// by `{known_routes_key}:{pattern}` / `{blocked_routes_key}:{pattern}` so the
/// two logical sets share one map without collision. Readers consult this
/// directly; there is no Redis round-trip on the default build.
static ROUTE_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn route_cache() -> &'static Mutex<HashMap<String, String>> {
    ROUTE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl Entity {
    pub const PER_PAGE: u64 = 20;
    pub async fn find_by_pattern(
        db: &DatabaseConnection,
        pattern: &str,
    ) -> Result<Option<Model>, DbErr> {
        Entity::find()
            .filter(Column::RoutePattern.eq(pattern))
            .one(db)
            .await
    }

    pub async fn find_blocked_routes(db: &DatabaseConnection) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::IsBlocked.eq(true))
            .order_by_asc(Column::RoutePattern)
            .all(db)
            .await
    }

    pub async fn create_or_update(
        db: &DatabaseConnection,
        route_pattern: String,
        is_blocked: bool,
        reason: Option<String>,
    ) -> Result<Model, DbErr> {
        if let Some(existing) = Self::find_by_pattern(db, &route_pattern).await? {
            let mut active_model: ActiveModel = existing.into();
            active_model.is_blocked = Set(is_blocked);
            active_model.reason = Set(reason);
            active_model.updated_at = Set(chrono::Utc::now().fixed_offset());
            active_model.update(db).await
        } else {
            let new_route = ActiveModel {
                route_pattern: Set(route_pattern),
                is_blocked: Set(is_blocked),
                reason: Set(reason),
                ..Default::default()
            };
            new_route.insert(db).await
        }
    }

    pub async fn ensure_exists(
        db: &DatabaseConnection,
        route_pattern: &str,
    ) -> Result<Model, DbErr> {
        if let Some(existing) = Self::find_by_pattern(db, route_pattern).await? {
            return Ok(existing);
        }

        let new_route = ActiveModel {
            route_pattern: Set(route_pattern.to_string()),
            is_blocked: Set(false),
            reason: Set(None),
            ..Default::default()
        };

        match new_route.insert(db).await {
            Ok(model) => Ok(model),
            Err(DbErr::Exec(exec_err)) => match Self::find_by_pattern(db, route_pattern).await? {
                Some(existing) => Ok(existing),
                None => Err(DbErr::Exec(exec_err)),
            },
            Err(err) => Err(err),
        }
    }

    pub async fn search(
        db: &DatabaseConnection,
        query: RouteStatusQuery,
    ) -> Result<PaginatedList<Model>, DbErr> {
        let mut route_query = Entity::find();

        match BlockFilter::resolve(query.block_filter) {
            BlockFilter::All => {}
            BlockFilter::Blocked => {
                route_query = route_query.filter(Column::IsBlocked.eq(true));
            }
            BlockFilter::Unblocked => {
                route_query = route_query.filter(Column::IsBlocked.eq(false));
            }
        }

        if let Some(search_term) = &query.search {
            route_query = route_query.filter(Column::RoutePattern.contains(search_term));
        }

        if let Some(ts) = query.created_at_gt {
            route_query = route_query.filter(Column::CreatedAt.gt(ts));
        }
        if let Some(ts) = query.created_at_lt {
            route_query = route_query.filter(Column::CreatedAt.lt(ts));
        }
        if let Some(ts) = query.updated_at_gt {
            route_query = route_query.filter(Column::UpdatedAt.gt(ts));
        }
        if let Some(ts) = query.updated_at_lt {
            route_query = route_query.filter(Column::UpdatedAt.lt(ts));
        }

        if let Some(sorts) = query.sorts {
            for sort in sorts {
                let column = match sort.field.as_str() {
                    "route_pattern" => Some(Column::RoutePattern),
                    "is_blocked" => Some(Column::IsBlocked),
                    "created_at" => Some(Column::CreatedAt),
                    "updated_at" => Some(Column::UpdatedAt),
                    _ => None,
                };

                if let Some(col) = column {
                    route_query = match sort.order {
                        sea_orm::Order::Asc => route_query.order_by_asc(col),
                        sea_orm::Order::Desc => route_query.order_by_desc(col),
                        _ => route_query,
                    };
                }
            }
        } else {
            route_query = route_query.order_by_asc(Column::RoutePattern);
        }

        let page = query.page.unwrap_or(1);
        let total = route_query.clone().count(db).await?;

        let items = route_query
            .offset((page - 1) * Self::PER_PAGE)
            .limit(Self::PER_PAGE)
            .all(db)
            .await?;

        Ok(PaginatedList::new(items, total, page, Self::PER_PAGE))
    }

    pub async fn delete_by_pattern(
        db: &DatabaseConnection,
        pattern: &str,
    ) -> Result<DeleteResult, DbErr> {
        Entity::delete_many()
            .filter(Column::RoutePattern.eq(pattern))
            .exec(db)
            .await
    }

    /// Rebuild the process-global route cache from the DB. `known_routes_key`
    /// and `blocked_routes_key` are used as namespace prefixes for the two
    /// logical sets (previously Redis SET keys). The whole rebuild happens
    /// under one lock acquisition so a reader never observes a half-built cache.
    pub async fn sync_all_to_cache(
        db: &DatabaseConnection,
        known_routes_key: &str,
        blocked_routes_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let routes = Entity::find()
            .order_by_asc(Column::RoutePattern)
            .all(db)
            .await?;

        let mut cache = route_cache()
            .lock()
            .map_err(|e| format!("route cache lock poisoned: {e}"))?;
        // Drop only this cache's namespaces, leaving any unrelated keys intact.
        cache.retain(|k, _| !(k.starts_with(known_routes_key) || k.starts_with(blocked_routes_key)));

        for route in routes {
            cache.insert(
                format!("{}:{}", known_routes_key, route.route_pattern),
                route.route_pattern.clone(),
            );
            if route.is_blocked {
                cache.insert(
                    format!("{}:{}", blocked_routes_key, route.route_pattern),
                    route.route_pattern.clone(),
                );
            }
        }

        Ok(())
    }
}
