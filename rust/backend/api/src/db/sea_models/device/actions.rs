use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*,
    sea_query::{Expr, OnConflict},
    DbErr, Order, QueryOrder, Set,
};
use tracing::{info, instrument};

use super::{ActiveModel, Column, Entity, Model, NewDevice};

impl Entity {
    #[instrument(skip(conn, new))]
    pub async fn upsert(conn: &DbConn, new: NewDevice) -> DbResult<Model> {
        let now = chrono::Utc::now().fixed_offset();

        let token = new.token.clone();
        let am = ActiveModel {
            user_id: Set(new.user_id),
            token: Set(new.token),
            platform: Set(new.platform),
            device_id: Set(new.device_id),
            created_at: Set(now),
            updated_at: Set(now),
            last_seen_at: Set(now),
            invalid_strikes: Set(0),
            invalid_at: Set(None),
            ..Default::default()
        };
        // Read the row back by token. `exec_with_returning` on an ON CONFLICT update
        // looks up the wrong row in SQLite, so a token that moved accounts 404ed.
        Self::insert(am)
            .on_conflict(
                OnConflict::column(Column::Token)
                    .update_columns([
                        Column::UserId,
                        Column::Platform,
                        Column::DeviceId,
                        Column::UpdatedAt,
                        Column::LastSeenAt,
                        Column::InvalidStrikes,
                        Column::InvalidAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(conn)
            .await
            .map_err_to_response()?;
        let registered = Self::find()
            .filter(Column::Token.eq(token))
            .one(conn)
            .await
            .map_err_to_response()?
            .ok_or(DbErr::RecordNotFound("device token".into()))
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

    /// Drop every push token a device owns (sign-out, device delete, eviction).
    pub async fn delete_for_device(conn: &DbConn, user_id: i32, device_id: &str) -> DbResult<u64> {
        let res = Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeviceId.eq(device_id))
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected)
    }

    /// The account that holds this push token, if any. The token is unique, so at
    /// most one row. Used to detect a token moving between accounts (A36).
    pub async fn owner_of_token(conn: &DbConn, token: &str) -> DbResult<Option<i32>> {
        let row = Self::find()
            .filter(Column::Token.eq(token))
            .one(conn)
            .await
            .map_err_to_response()?;
        Ok(row.map(|r| r.user_id))
    }

    /// Count one non-terminal rejection and return the new strike total.
    pub async fn record_invalid(conn: &DbConn, id: i32) -> DbResult<i32> {
        let row = Self::find_by_id(id).one(conn).await.map_err_to_response()?;
        let Some(row) = row else {
            return Ok(0);
        };
        let strikes = row.invalid_strikes.saturating_add(1);
        Self::update_many()
            .col_expr(Column::InvalidStrikes, Expr::value(strikes))
            .col_expr(
                Column::InvalidAt,
                Expr::value(chrono::Utc::now().fixed_offset()),
            )
            .filter(Column::Id.eq(id))
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(strikes)
    }

    /// A successful send ends any strike run.
    pub async fn clear_invalid(conn: &DbConn, id: i32) -> DbResult<()> {
        Self::update_many()
            .col_expr(Column::InvalidStrikes, Expr::value(0))
            .col_expr(
                Column::InvalidAt,
                Expr::value(Option::<DateTimeWithTimeZone>::None),
            )
            .filter(Column::Id.eq(id))
            .filter(Column::InvalidStrikes.gt(0))
            .exec(conn)
            .await
            .map_err_to_response()?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    async fn test_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            r#"CREATE TABLE "devices" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER NOT NULL,
                "token" TEXT NOT NULL UNIQUE,
                "platform" TEXT NOT NULL,
                "device_id" TEXT,
                "created_at" TEXT NOT NULL,
                "updated_at" TEXT NOT NULL,
                "last_seen_at" TEXT NOT NULL,
                "invalid_strikes" INTEGER NOT NULL DEFAULT 0,
                "invalid_at" TEXT
            )"#,
        )
        .await
        .unwrap();
        db
    }

    fn new_device(user_id: i32, token: &str) -> NewDevice {
        NewDevice {
            user_id,
            token: token.into(),
            platform: "web".into(),
            device_id: None,
        }
    }

    #[tokio::test]
    async fn strikes_count_up_clear_on_success_and_reset_on_reregister() {
        let db = test_db().await;
        let row = Entity::upsert(&db, new_device(1, "tok-strike"))
            .await
            .unwrap();
        assert_eq!(Entity::record_invalid(&db, row.id).await.unwrap(), 1);
        assert_eq!(Entity::record_invalid(&db, row.id).await.unwrap(), 2);
        let stored = Entity::find_by_id(row.id).one(&db).await.unwrap().unwrap();
        assert_eq!(stored.invalid_strikes, 2);
        assert!(stored.invalid_at.is_some());

        Entity::clear_invalid(&db, row.id).await.unwrap();
        let cleared = Entity::find_by_id(row.id).one(&db).await.unwrap().unwrap();
        assert_eq!(cleared.invalid_strikes, 0);
        assert!(cleared.invalid_at.is_none());

        Entity::record_invalid(&db, row.id).await.unwrap();
        Entity::upsert(&db, new_device(1, "tok-strike"))
            .await
            .unwrap();
        let again = Entity::find_by_id(row.id).one(&db).await.unwrap().unwrap();
        assert_eq!(
            again.invalid_strikes, 0,
            "a fresh registration resets strikes"
        );
    }

    #[tokio::test]
    async fn owner_of_token_follows_a_transfer() {
        let db = test_db().await;
        Entity::upsert(&db, new_device(1, "tok-move"))
            .await
            .unwrap();
        assert_eq!(
            Entity::owner_of_token(&db, "tok-move").await.unwrap(),
            Some(1)
        );
        Entity::upsert(&db, new_device(2, "tok-move"))
            .await
            .unwrap();
        assert_eq!(
            Entity::owner_of_token(&db, "tok-move").await.unwrap(),
            Some(2)
        );
        assert_eq!(Entity::owner_of_token(&db, "missing").await.unwrap(), None);
    }
}
