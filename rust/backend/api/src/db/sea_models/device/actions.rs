use crate::error::{DbResult, DbResultExt};
use sea_orm::{entity::prelude::*, sea_query::OnConflict, Order, QueryOrder, Set};
use tracing::{info, instrument};

use super::{ActiveModel, Column, Entity, Model, NewDevice};

impl Entity {
    #[instrument(skip(conn, new))]
    pub async fn upsert(conn: &DbConn, new: NewDevice) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();

        let am = ActiveModel {
            user_id: Set(new.user_id),
            token: Set(new.token),
            platform: Set(new.platform),
            created_at: Set(now),
            updated_at: Set(now),
            last_seen_at: Set(now),
            ..Default::default()
        };
        let registered = Self::insert(am)
            .on_conflict(
                OnConflict::column(Column::Token)
                    .update_columns([
                        Column::UserId,
                        Column::Platform,
                        Column::UpdatedAt,
                        Column::LastSeenAt,
                    ])
                    .to_owned(),
            )
            .exec_with_returning(conn)
            .await
            .map_err_to_response()?;
        info!(device_id = registered.id, "Device registered");
        Ok(registered)
    }

    pub async fn list_for_user(conn: &DbConn, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::CreatedAt, Order::Desc)
            .all(conn)
            .await
            .map_err_to_response()
    }

    pub async fn delete_for_user(conn: &DbConn, user_id: i32, token: &str) -> DbResult<u64> {
        let res = Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Token.eq(token))
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected)
    }

    pub async fn prune_owned_by_id(conn: &DbConn, id: i32, user_id: i32) -> DbResult<()> {
        Self::delete_many()
            .filter(Column::Id.eq(id))
            .filter(Column::UserId.eq(user_id))
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(())
    }
}
