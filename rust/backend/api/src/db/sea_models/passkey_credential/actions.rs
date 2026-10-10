use crate::error::{DbResult, ErrorCode, ErrorResponse};
use chrono::Utc;
use sea_orm::{entity::prelude::*, Order, QueryOrder, Set};

use super::*;

pub fn encode_credential_id(cred_id: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    URL_SAFE_NO_PAD.encode(cred_id)
}

impl Entity {
    pub async fn create<T: ConnectionTrait>(
        conn: &T,
        user_id: i32,
        passkey: &webauthn_rs::prelude::Passkey,
        device_type: Option<String>,
        transports: Option<Json>,
    ) -> DbResult<Model> {
        let credential_id = encode_credential_id(passkey.cred_id().as_slice());
        let public_key = serde_json::to_vec(passkey).map_err(|err| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to serialize passkey")
                .with_details(err.to_string())
        })?;
        let now = Utc::now().fixed_offset();

        let model = ActiveModel {
            user_id: Set(user_id),
            credential_id: Set(credential_id),
            public_key: Set(public_key),
            counter: Set(0),
            device_type: Set(device_type),
            transports: Set(transports),
            created_at: Set(now),
            last_used_at: Set(None),
            ..Default::default()
        };

        match model.insert(conn).await {
            Ok(m) => Ok(m),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn list_by_user<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<Vec<Model>> {
        match Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::CreatedAt, Order::Asc)
            .all(conn)
            .await
        {
            Ok(rows) => Ok(rows),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn count_for_user<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<u64> {
        match Self::find()
            .filter(Column::UserId.eq(user_id))
            .count(conn)
            .await
        {
            Ok(count) => Ok(count),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn find_by_credential_id<T: ConnectionTrait>(
        conn: &T,
        credential_id: &str,
    ) -> DbResult<Option<Model>> {
        match Self::find()
            .filter(Column::CredentialId.eq(credential_id))
            .one(conn)
            .await
        {
            Ok(opt) => Ok(opt),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn find_by_credential_id_for_user<T: ConnectionTrait>(
        conn: &T,
        credential_id: &str,
        user_id: i32,
    ) -> DbResult<Option<Model>> {
        match Self::find()
            .filter(Column::CredentialId.eq(credential_id))
            .filter(Column::UserId.eq(user_id))
            .one(conn)
            .await
        {
            Ok(opt) => Ok(opt),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn touch_counter<T: ConnectionTrait>(
        conn: &T,
        id: i32,
        new_counter: u32,
    ) -> DbResult<()> {
        let now = Utc::now().fixed_offset();
        let mut active: ActiveModel = Self::find()
            .filter(Column::Id.eq(id))
            .one(conn)
            .await
            .map_err(ErrorResponse::from)?
            .ok_or_else(|| {
                ErrorResponse::new(ErrorCode::RecordNotFound)
                    .with_message("Passkey credential not found")
            })?
            .into();
        active.counter = Set(new_counter as i64);
        active.last_used_at = Set(Some(now));
        active
            .update(conn)
            .await
            .map(|_| ())
            .map_err(ErrorResponse::from)
    }

    pub async fn delete_by_credential_id_for_user<T: ConnectionTrait>(
        conn: &T,
        credential_id: &str,
        user_id: i32,
    ) -> DbResult<u64> {
        match Self::delete_many()
            .filter(Column::CredentialId.eq(credential_id))
            .filter(Column::UserId.eq(user_id))
            .exec(conn)
            .await
        {
            Ok(res) => Ok(res.rows_affected),
            Err(err) => Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, Database, DatabaseConnection};

    // passkey_credentials has no migration yet (same pre-existing gap as the
    // device/notification/OAuth tables before m000005); create it here to match
    // the sea-orm model so the count path is testable.
    async fn mem_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE passkey_credentials (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                credential_id TEXT NOT NULL,
                public_key BLOB NOT NULL,
                counter INTEGER NOT NULL DEFAULT 0,
                device_type TEXT,
                transports TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_used_at TEXT
            )",
        )
        .await
        .unwrap();
        db
    }

    async fn insert_credential(db: &DatabaseConnection, user_id: i32, credential_id: &str) {
        db.execute_unprepared(&format!(
            "INSERT INTO passkey_credentials (user_id, credential_id, public_key, counter, created_at) \
             VALUES ({user_id}, '{credential_id}', X'00', 0, '2026-01-01T00:00:00+00:00')"
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn count_for_user_counts_only_that_users_credentials() {
        let db = mem_db().await;
        insert_credential(&db, 1, "cred-a").await;
        insert_credential(&db, 1, "cred-b").await;
        insert_credential(&db, 2, "cred-c").await;

        assert_eq!(Entity::count_for_user(&db, 1).await.unwrap(), 2);
        assert_eq!(Entity::count_for_user(&db, 2).await.unwrap(), 1);
        assert_eq!(Entity::count_for_user(&db, 3).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn find_by_credential_id_for_user_is_user_scoped() {
        let db = mem_db().await;
        insert_credential(&db, 1, "cred-a").await;
        insert_credential(&db, 2, "cred-b").await;

        let own = Entity::find_by_credential_id_for_user(&db, "cred-a", 1)
            .await
            .unwrap();
        assert_eq!(own.map(|m| m.user_id), Some(1));

        let other_users = Entity::find_by_credential_id_for_user(&db, "cred-a", 2)
            .await
            .unwrap();
        assert!(
            other_users.is_none(),
            "another user's credential_id must not resolve"
        );

        let missing = Entity::find_by_credential_id_for_user(&db, "nope", 1)
            .await
            .unwrap();
        assert!(missing.is_none());
    }
}
