use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// passkey_credentials was written by the WebAuthn/passkey code paths but no
// migration owned the table — every real DB (fresh or provisioned) lacked it,
// so any passkey write AND the identities listing (COUNT(*) for the
// last-method guard) failed with "no such table". IF NOT EXISTS keeps any
// out-of-band-provisioned DB that already carries it untouched.
//
// Column shapes mirror sea_models::passkey_credential::Model: public_key is
// opaque BLOB key material (never serialized to clients), transports is JSON
// text, timestamps are TEXT like every other table in this schema.
const CREATE_PASSKEY_CREDENTIALS: &str = "CREATE TABLE IF NOT EXISTS passkey_credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    credential_id TEXT NOT NULL,
    public_key BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    device_type TEXT,
    transports TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE CASCADE
)";

// WebAuthn credential ids are globally unique; the authenticator's counter
// updates address rows by this id, so duplicates would be ambiguous.
const CREATE_PASSKEY_CREDENTIALS_UNIQUE: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_passkey_credentials_credential_id \
     ON passkey_credentials (credential_id)";

const CREATE_PASSKEY_CREDENTIALS_USER_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_passkey_credentials_user \
     ON passkey_credentials (user_id)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(CREATE_PASSKEY_CREDENTIALS).await?;
        conn.execute_unprepared(CREATE_PASSKEY_CREDENTIALS_UNIQUE)
            .await?;
        conn.execute_unprepared(CREATE_PASSKEY_CREDENTIALS_USER_INDEX)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS passkey_credentials")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, Statement};
    use sea_orm_migration::SchemaManager;

    async fn mem_db() -> sea_orm::DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        Database::connect(opt).await.unwrap()
    }

    async fn fixture(db: &sea_orm::DatabaseConnection) {
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO users (id, name) VALUES (1, 'a')")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn creates_table_and_enforces_credential_uniqueness() {
        let db = mem_db().await;
        fixture(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        db.execute_unprepared(
            "INSERT INTO passkey_credentials (user_id, credential_id, public_key, created_at) \
             VALUES (1, 'cred-1', X'0102', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();

        let dup = db
            .execute_unprepared(
                "INSERT INTO passkey_credentials (user_id, credential_id, public_key, created_at) \
                 VALUES (1, 'cred-1', X'0304', '2026-01-01T00:00:00Z')",
            )
            .await;
        assert!(dup.is_err(), "duplicate credential_id must be rejected");

        let count = db
            .query_one(Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "SELECT COUNT(*) FROM passkey_credentials WHERE user_id = 1",
                Vec::<sea_orm::Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index::<i64>(0)
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn rerun_is_idempotent_and_down_drops() {
        let db = mem_db().await;
        fixture(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();

        Migration.down(&mgr).await.unwrap();
        let exists = db
            .query_one(Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type='table' AND name='passkey_credentials'"
                    .to_string(),
            ))
            .await
            .unwrap();
        assert!(exists.is_none(), "down must drop the table");
    }
}
