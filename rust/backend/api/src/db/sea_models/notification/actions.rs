use crate::error::{DbResult, DbResultExt};
use ruxlog_types::PaginatedList;
use sea_orm::{entity::prelude::*, Order, QueryOrder, Set};
use tracing::{info, instrument};

use super::{ActiveModel, Column, Entity, Model, NewNotification};

impl Entity {
    pub const PER_PAGE: u64 = 20;

    #[instrument(skip(conn, new), fields(user_id = new.user_id))]
    pub async fn create(conn: &DbConn, new: NewNotification) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();
        let am = ActiveModel {
            user_id: Set(new.user_id),
            kind: Set(new.kind),
            title: Set(new.title),
            body: Set(new.body),
            data: Set(new.data),
            read_at: Set(None),
            created_at: Set(now),
            ..Default::default()
        };
        let inserted = am.insert(conn).await.map_err_to_response()?;
        info!(notification_id = inserted.id, "Notification created");
        Ok(inserted)
    }

    pub async fn list_for_user(
        conn: &DbConn,
        user_id: i32,
        page: u64,
        per_page: u64,
    ) -> DbResult<PaginatedList<Model>> {
        let per_page = if per_page == 0 {
            Self::PER_PAGE
        } else {
            per_page.min(100)
        };
        let page = if page == 0 { 1 } else { page };

        let paginator = Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::CreatedAt, Order::Desc)
            .paginate(conn, per_page);
        let total = paginator.num_items().await.map_err_to_response()?;
        let items = paginator.fetch_page(page - 1).await.map_err_to_response()?;
        Ok(PaginatedList::new(items, total, page, per_page))
    }

    pub async fn unread_count(conn: &DbConn, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ReadAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()
    }

    pub async fn mark_read(conn: &DbConn, user_id: i32, id: i32) -> DbResult<Option<Model>> {
        let now = chrono::Utc::now().fixed_offset();
        let model = Self::find()
            .filter(Column::Id.eq(id))
            .filter(Column::UserId.eq(user_id))
            .one(conn)
            .await
            .map_err_to_response()?;
        if let Some(m) = model {
            let mut am: ActiveModel = m.into();
            am.read_at = Set(Some(now));
            let updated = am.update(conn).await.map_err_to_response()?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    pub async fn mark_all_read(conn: &DbConn, user_id: i32) -> DbResult<u64> {
        let now = chrono::Utc::now().fixed_offset();
        let unread = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ReadAt.is_null())
            .all(conn)
            .await
            .map_err_to_response()?;
        let count = unread.len() as u64;
        for m in unread {
            let mut am: ActiveModel = m.into();
            am.read_at = Set(Some(now));
            am.update(conn).await.map_err_to_response()?;
        }
        Ok(count)
    }
}
