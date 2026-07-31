use sea_orm::{
    entity::prelude::*, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};

use crate::error::DbResult;
use ruxlog_types::PaginatedList;

use super::*;

impl Entity {
    pub const PER_PAGE: u64 = 10;
    pub const MAX_REVISIONS_PER_POST: u64 = 10;

    pub async fn create(
        conn: &DbConn,
        post_id: i32,
        content: String,
        metadata: Option<serde_json::Value>,
    ) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();

        let txn = conn.begin().await?;

        let active = ActiveModel {
            post_id: Set(post_id),
            content: Set(content),
            metadata: Set(metadata),
            created_at: Set(now),
            ..Default::default()
        };

        let created = active.insert(&txn).await?;

        Self::enforce_max_inner(&txn, post_id, Self::MAX_REVISIONS_PER_POST).await?;

        txn.commit().await?;
        Ok(created)
    }

    pub async fn list_by_post(
        conn: &DbConn,
        post_id: i32,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> DbResult<PaginatedList<Model>> {
        let per_page = per_page.unwrap_or(Self::PER_PAGE);
        let page = match page {
            Some(p) if p > 0 => p,
            _ => 1,
        };

        let query = Entity::find()
            .filter(Column::PostId.eq(post_id))
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id);

        let paginator = query.paginate(conn, per_page);
        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(page - 1).await?;

        Ok(PaginatedList::new(items, total, page, per_page))
    }

    pub async fn enforce_max(conn: &DbConn, post_id: i32, max: u64) -> DbResult<u64> {
        let txn = conn.begin().await?;
        let deleted = Self::enforce_max_inner(&txn, post_id, max).await?;
        txn.commit().await?;
        Ok(deleted)
    }

    async fn enforce_max_inner<C>(conn: &C, post_id: i32, max: u64) -> DbResult<u64>
    where
        C: ConnectionTrait,
    {
        let total: u64 = Entity::find()
            .filter(Column::PostId.eq(post_id))
            .count(conn)
            .await?;

        if total <= max {
            return Ok(0);
        }

        let to_delete = (total - max) as i64;

        let old_ids: Vec<i32> = Entity::find()
            .filter(Column::PostId.eq(post_id))
            .order_by_asc(Column::CreatedAt)
            .order_by_asc(Column::Id)
            .limit(to_delete as u64)
            .all(conn)
            .await?
            .into_iter()
            .map(|m| m.id)
            .collect();

        if old_ids.is_empty() {
            return Ok(0);
        }

        let res = Entity::delete_many()
            .filter(Column::Id.is_in(old_ids))
            .exec(conn)
            .await?;

        Ok(res.rows_affected)
    }
}
