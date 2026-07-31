use sea_orm::{
    entity::prelude::*, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::error::DbResult;
use ruxlog_types::PaginatedList;

use super::*;

impl Entity {
    pub const PER_PAGE: u64 = 10;

    pub async fn create(
        conn: &DbConn,
        name: String,
        slug: String,
        description: Option<String>,
    ) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();

        let active = ActiveModel {
            name: Set(name),
            slug: Set(slug),
            description: Set(description),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let model = active.insert(conn).await?;
        Ok(model)
    }

    /// `description: None` leaves the column unchanged (not cleared to NULL).
    pub async fn update(
        conn: &DbConn,
        series_id: i32,
        name: Option<String>,
        slug: Option<String>,
        description: Option<String>,
    ) -> DbResult<Option<Model>> {
        if let Some(existing) = Entity::find_by_id(series_id).one(conn).await? {
            let mut active: ActiveModel = existing.into();

            if let Some(n) = name {
                active.name = Set(n);
            }
            if let Some(s) = slug {
                active.slug = Set(s);
            }
            if let Some(d) = description {
                active.description = Set(Some(d));
            }

            active.updated_at = Set(chrono::Utc::now().fixed_offset());

            let updated = active.update(conn).await?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    pub async fn delete(conn: &DbConn, series_id: i32) -> DbResult<u64> {
        let res = Entity::delete_by_id(series_id).exec(conn).await?;
        Ok(res.rows_affected)
    }

    pub async fn list(
        conn: &DbConn,
        page: Option<u64>,
        per_page: Option<u64>,
        search: Option<String>,
    ) -> DbResult<PaginatedList<Model>> {
        let per_page = per_page.unwrap_or(Self::PER_PAGE);
        let page = match page {
            Some(p) if p > 0 => p,
            _ => 1,
        };

        let mut query = Entity::find()
            .order_by_desc(Column::UpdatedAt)
            .order_by_desc(Column::Id);

        if let Some(s) = search {
            let pattern = format!("%{}%", s);
            query = query.filter(
                Condition::any()
                    .add(Column::Name.contains(&pattern))
                    .add(Column::Slug.contains(&pattern)),
            );
        }

        let paginator = query.paginate(conn, per_page);
        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(page - 1).await?;

        Ok(PaginatedList::new(items, total, page, per_page))
    }

    pub async fn find_by_slug(conn: &DbConn, slug: String) -> DbResult<Option<Model>> {
        let model = Entity::find()
            .filter(Column::Slug.eq(slug))
            .one(conn)
            .await?;
        Ok(model)
    }
}
