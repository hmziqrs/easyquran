use crate::error::DbResult;
use sea_orm::{entity::prelude::*, Order, QueryOrder, Set};
use tracing::{info, instrument, warn};

use super::*;

impl Entity {
    pub async fn list_for_user<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<Vec<Model>> {
        match Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::CreatedAt, Order::Asc)
            .order_by(Column::Id, Order::Asc)
            .all(conn)
            .await
        {
            Ok(rows) => Ok(rows),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn delete_for_user_provider<T: ConnectionTrait>(
        conn: &T,
        user_id: i32,
        provider: &str,
    ) -> DbResult<u64> {
        match Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Provider.eq(provider))
            .exec(conn)
            .await
        {
            Ok(res) => Ok(res.rows_affected),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn find_by_provider<T: ConnectionTrait>(
        conn: &T,
        provider: &str,
        provider_user_id: &str,
    ) -> DbResult<Option<Model>> {
        match Self::find()
            .filter(Column::Provider.eq(provider))
            .filter(Column::ProviderUserId.eq(provider_user_id))
            .one(conn)
            .await
        {
            Ok(model) => Ok(model),
            Err(err) => Err(err.into()),
        }
    }

    #[instrument(skip(conn), fields(user_id = new.user_id, provider = %new.provider))]
    pub async fn link<T: ConnectionTrait>(conn: &T, new: NewOauthIdentity) -> DbResult<Model> {
        let active = ActiveModel {
            user_id: Set(new.user_id),
            provider: Set(new.provider),
            provider_user_id: Set(new.provider_user_id),
            created_at: Set(new.created_at),
            ..Default::default()
        };

        match active.insert(conn).await {
            Ok(model) => {
                info!(identity_id = model.id, "Linked OAuth provider identity");
                Ok(model)
            }
            Err(err) => {
                warn!(error = %err, "Failed to link OAuth provider identity");
                Err(err.into())
            }
        }
    }

    pub async fn delete_for_user<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<u64> {
        match Self::delete_many()
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
    use crate::db::sea_models::user;
    use chrono::TimeZone;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database, DatabaseConnection};

    async fn mem_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn insert_user(db: &DatabaseConnection, email: &str) -> user::Model {
        let now = chrono::Utc::now().fixed_offset();
        user::ActiveModel {
            name: Set(email.to_string()),
            email: Set(email.to_string()),
            password: Set(Some("password-hash".to_string())),
            role: Set(user::UserRole::User),
            is_verified: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    fn at(day: u32) -> chrono::DateTime<chrono::FixedOffset> {
        chrono::Utc
            .with_ymd_and_hms(2026, 1, day, 0, 0, 0)
            .unwrap()
            .fixed_offset()
    }

    async fn link(
        db: &DatabaseConnection,
        user_id: i32,
        provider: &str,
        created_at: chrono::DateTime<chrono::FixedOffset>,
    ) {
        Entity::link(
            db,
            NewOauthIdentity {
                user_id,
                provider: provider.to_string(),
                provider_user_id: format!("{provider}-{user_id}"),
                created_at,
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn list_for_user_returns_only_that_users_rows_in_created_at_order() {
        let db = mem_db().await;
        let alice = insert_user(&db, "alice@example.com").await;
        let bob = insert_user(&db, "bob@example.com").await;

        link(&db, alice.id, "github", at(2)).await;
        link(&db, alice.id, "discord", at(1)).await;
        link(&db, bob.id, "google", at(1)).await;

        let rows = Entity::list_for_user(&db, alice.id).await.unwrap();
        let providers: Vec<&str> = rows.iter().map(|r| r.provider.as_str()).collect();
        assert_eq!(
            providers,
            vec!["discord", "github"],
            "only alice's rows, oldest first"
        );
        assert!(rows.iter().all(|r| r.user_id == alice.id));
    }

    #[tokio::test]
    async fn list_for_user_is_empty_for_unlinked_user() {
        let db = mem_db().await;
        let alice = insert_user(&db, "alice@example.com").await;

        assert!(Entity::list_for_user(&db, alice.id)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn delete_for_user_provider_removes_only_the_match() {
        let db = mem_db().await;
        let alice = insert_user(&db, "alice@example.com").await;
        let bob = insert_user(&db, "bob@example.com").await;

        link(&db, alice.id, "github", at(1)).await;
        link(&db, alice.id, "discord", at(2)).await;
        link(&db, bob.id, "github", at(3)).await;

        let removed = Entity::delete_for_user_provider(&db, alice.id, "github")
            .await
            .unwrap();
        assert_eq!(removed, 1);

        let alice_rows = Entity::list_for_user(&db, alice.id).await.unwrap();
        let providers: Vec<&str> = alice_rows.iter().map(|r| r.provider.as_str()).collect();
        assert_eq!(
            providers,
            vec!["discord"],
            "the sibling provider for the same user must survive"
        );

        let bob_rows = Entity::list_for_user(&db, bob.id).await.unwrap();
        assert_eq!(bob_rows.len(), 1, "another user's github link must survive");
        assert_eq!(bob_rows[0].provider, "github");

        assert_eq!(
            Entity::delete_for_user_provider(&db, alice.id, "github")
                .await
                .unwrap(),
            0,
            "a second delete matches nothing"
        );
    }
}
