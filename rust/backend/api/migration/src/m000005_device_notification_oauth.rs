use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// devices + notifications + user_oauth_identities: written by device_v1,
// notification_v1 and the OAuth login path since their models landed, but no
// earlier migration ever created them — every one of those endpoints 500s on a
// freshly migrated DB. Shapes mirror the sea-orm models exactly
// (DateTimeWithTimeZone -> TEXT, NotificationKind -> TEXT via active enum,
// JsonBinary data -> TEXT under the SQLite backend). IF NOT EXISTS everywhere:
// a DB already carrying any of these tables keeps its data untouched.
const CREATE_DEVICES: &str = "CREATE TABLE IF NOT EXISTS devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    platform TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE CASCADE
)";

const CREATE_DEVICES_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_devices_user_token ON devices (user_id, token)";

const CREATE_NOTIFICATIONS: &str = "CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    data TEXT,
    read_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE CASCADE
)";

const CREATE_NOTIFICATIONS_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_notifications_user_created \
     ON notifications (user_id, created_at)";

// provider_user_id stays plaintext: the UNIQUE (provider, provider_user_id)
// index matches on it at login (see the model doc in sea_models).
const CREATE_OAUTH_IDENTITIES: &str = "CREATE TABLE IF NOT EXISTS user_oauth_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    provider TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE CASCADE
)";

const CREATE_OAUTH_IDENTITIES_UNIQUE: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_oauth_identities_provider_uid \
     ON user_oauth_identities (provider, provider_user_id)";

const CREATE_OAUTH_IDENTITIES_USER_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_user_oauth_identities_user \
     ON user_oauth_identities (user_id)";

const STMTS: &[&str] = &[
    CREATE_DEVICES,
    CREATE_DEVICES_INDEX,
    CREATE_NOTIFICATIONS,
    CREATE_NOTIFICATIONS_INDEX,
    CREATE_OAUTH_IDENTITIES,
    CREATE_OAUTH_IDENTITIES_UNIQUE,
    CREATE_OAUTH_IDENTITIES_USER_INDEX,
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        for stmt in STMTS {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        for stmt in [
            "DROP TABLE IF EXISTS devices",
            "DROP TABLE IF EXISTS notifications",
            "DROP TABLE IF EXISTS user_oauth_identities",
        ] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, Value};
    use sea_orm_migration::SchemaManager;

    async fn mem_db() -> sea_orm::DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        Database::connect(opt).await.unwrap()
    }

    async fn column_names(db: &sea_orm::DatabaseConnection, table: &str) -> Vec<String> {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!("PRAGMA table_info({table})"),
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|r| r.try_get_by_index::<String>(1).unwrap())
            .collect()
    }

    async fn index_exists(db: &sea_orm::DatabaseConnection, table: &str, name: &str) -> bool {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!("PRAGMA index_list('{table}')"),
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter().any(|r| {
            let n: Option<String> = r.try_get_by_index(1).ok();
            n.as_deref() == Some(name)
        })
    }

    #[tokio::test]
    async fn fresh_migration_creates_all_three_tables() {
        let db = mem_db().await;
        // users must exist first: the FKs reference it.
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        for (table, expected) in [
            (
                "devices",
                vec![
                    "id",
                    "user_id",
                    "token",
                    "platform",
                    "created_at",
                    "updated_at",
                    "last_seen_at",
                ],
            ),
            (
                "notifications",
                vec![
                    "id",
                    "user_id",
                    "kind",
                    "title",
                    "body",
                    "data",
                    "read_at",
                    "created_at",
                ],
            ),
            (
                "user_oauth_identities",
                vec![
                    "id",
                    "user_id",
                    "provider",
                    "provider_user_id",
                    "created_at",
                ],
            ),
        ] {
            let cols = column_names(&db, table).await;
            for name in expected {
                assert!(
                    cols.iter().any(|c| c == name),
                    "{table} missing column {name}: {cols:?}"
                );
            }
        }

        assert!(index_exists(&db, "devices", "idx_devices_user_token").await);
        assert!(index_exists(&db, "notifications", "idx_notifications_user_created").await);
        assert!(
            index_exists(
                &db,
                "user_oauth_identities",
                "idx_user_oauth_identities_provider_uid"
            )
            .await
        );
        assert!(
            index_exists(
                &db,
                "user_oauth_identities",
                "idx_user_oauth_identities_user"
            )
            .await
        );
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            column_names(&db, "devices").await.len(),
            7,
            "no duplicate columns on re-run"
        );
        assert_eq!(column_names(&db, "notifications").await.len(), 8);
        assert_eq!(column_names(&db, "user_oauth_identities").await.len(), 5);
    }

    #[tokio::test]
    async fn preexisting_table_is_left_untouched() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        // Out-of-band provisioned DB already carries its own devices table.
        db.execute_unprepared(
            "CREATE TABLE devices (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL, \
             token TEXT NOT NULL, platform TEXT NOT NULL, created_at TEXT NOT NULL, \
             updated_at TEXT NOT NULL, last_seen_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO devices (user_id, token, platform, created_at, updated_at, last_seen_at) \
             VALUES (1, 'tok', 'ios', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let count = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM devices",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index::<i64>(0)
            .unwrap();
        assert_eq!(count, 1, "existing device rows must survive the migration");
    }

    #[tokio::test]
    async fn oauth_provider_identity_uniqueness_enforced() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        db.execute_unprepared("INSERT INTO users (id, name) VALUES (1, 'a'), (2, 'b')")
            .await
            .unwrap();
        db.execute_unprepared(
            "INSERT INTO user_oauth_identities (user_id, provider, provider_user_id, created_at) \
             VALUES (1, 'github', 'gh-1', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();
        let err = db
            .execute_unprepared(
                "INSERT INTO user_oauth_identities (user_id, provider, provider_user_id, created_at) \
                 VALUES (2, 'github', 'gh-1', '2026-01-01T00:00:00Z')",
            )
            .await;
        assert!(
            err.is_err(),
            "UNIQUE(provider, provider_user_id) must reject a second account for the same identity"
        );
    }

    #[tokio::test]
    async fn down_drops_all_three_tables() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.down(&mgr).await.unwrap();
        for table in ["devices", "notifications", "user_oauth_identities"] {
            assert!(!mgr.has_table(table).await.unwrap());
        }
    }

    #[tokio::test]
    async fn full_migrator_run_creates_all_three_tables() {
        use crate::Migrator;
        use sea_orm_migration::MigratorTrait;

        let db = mem_db().await;
        Migrator::up(&db, None).await.unwrap();
        for table in ["devices", "notifications", "user_oauth_identities"] {
            assert!(
                SchemaManager::new(&db).has_table(table).await.unwrap(),
                "Migrator::up must create {table}"
            );
        }
    }
}
