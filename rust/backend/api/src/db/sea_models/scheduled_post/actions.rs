use sea_orm::{
    entity::prelude::*, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};

use crate::error::DbResult;
use ruxlog_types::PaginatedList;

use super::*;

use super::model::ScheduledPostStatus;

impl Entity {
    pub const PER_PAGE: u64 = 10;

    pub async fn create(
        conn: &DbConn,
        post_id: i32,
        publish_at: DateTimeWithTimeZone,
        status: Option<ScheduledPostStatus>,
    ) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();

        let active = ActiveModel {
            post_id: Set(post_id),
            publish_at: Set(publish_at),
            status: Set(status.unwrap_or(ScheduledPostStatus::Pending)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let model = active.insert(conn).await?;
        Ok(model)
    }

    pub async fn upsert(
        conn: &DbConn,
        post_id: i32,
        publish_at: DateTimeWithTimeZone,
    ) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();
        let txn = conn.begin().await?;

        let existing = Entity::find()
            .filter(Column::PostId.eq(post_id))
            .filter(Column::Status.eq(ScheduledPostStatus::Pending))
            .order_by_desc(Column::UpdatedAt)
            .order_by_desc(Column::Id)
            .one(&txn)
            .await?;

        let result = if let Some(existing_model) = existing {
            let mut active: ActiveModel = existing_model.into();
            active.publish_at = Set(publish_at);
            active.status = Set(ScheduledPostStatus::Pending);
            active.updated_at = Set(now);
            active.update(&txn).await?
        } else {
            let active = ActiveModel {
                post_id: Set(post_id),
                publish_at: Set(publish_at),
                status: Set(ScheduledPostStatus::Pending),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            };
            active.insert(&txn).await?
        };

        txn.commit().await?;
        Ok(result)
    }

    pub async fn find_by_post_id(conn: &DbConn, post_id: i32) -> DbResult<Option<Model>> {
        let model = Entity::find()
            .filter(Column::PostId.eq(post_id))
            .order_by_desc(Column::UpdatedAt)
            .order_by_desc(Column::Id)
            .one(conn)
            .await?;
        Ok(model)
    }

    pub async fn due_pending(
        conn: &DbConn,
        until: DateTimeWithTimeZone,
        limit: Option<u64>,
    ) -> DbResult<Vec<Model>> {
        let mut query = Entity::find()
            .filter(Column::Status.eq(ScheduledPostStatus::Pending))
            .filter(Column::PublishAt.lte(until))
            .order_by_asc(Column::PublishAt)
            .order_by_asc(Column::Id);

        if let Some(lim) = limit {
            query = query.limit(lim);
        }

        let items = query.all(conn).await?;
        Ok(items)
    }

    pub async fn list_by_status(
        conn: &DbConn,
        status: ScheduledPostStatus,
        page: Option<u64>,
        per_page: Option<u64>,
    ) -> DbResult<PaginatedList<Model>> {
        let per_page = per_page.unwrap_or(Self::PER_PAGE);
        let page = match page {
            Some(p) if p > 0 => p,
            _ => 1,
        };

        let query = Entity::find()
            .filter(Column::Status.eq(status))
            .order_by_desc(Column::UpdatedAt)
            .order_by_desc(Column::Id);

        let paginator = query.paginate(conn, per_page);
        let total = paginator.num_items().await?;
        let items = paginator.fetch_page(page - 1).await?;

        Ok(PaginatedList::new(items, total, page, per_page))
    }
}
