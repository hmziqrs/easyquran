use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Device fingerprint and continuity (devices.md D3). A device row gains its public
// key (JWK text), the RFC 7638 thumbprint of that key, the store label the client
// reported, the optional per-account continuity hint, and the conflict flag set when
// two live machines present the same hint. `user_device_salts` holds the per-account
// salt the hint is keyed with; it is account-lifetime data, so it follows the
// standing table rules and erasure removes it with the user. Nothing here is an
// identifier for analytics, rate limits or bans.
const COLUMNS: &[(&str, &str, &str)] = &[
    ("user_devices", "public_key", "TEXT"),
    ("user_devices", "key_thumbprint", "TEXT"),
    ("user_devices", "key_store", "TEXT NOT NULL DEFAULT 'none'"),
    ("user_devices", "continuity_hint", "TEXT"),
    (
        "user_devices",
        "hint_conflict",
        "INTEGER NOT NULL DEFAULT 0",
    ),
    ("user_devices", "merged_key_thumbprint", "TEXT"),
];

const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS user_device_salts (
        user_id INTEGER NOT NULL,
        salt TEXT NOT NULL,
        created_at TEXT NOT NULL,
        expires_at INTEGER NOT NULL DEFAULT 253402300799,
        retention_class TEXT NOT NULL DEFAULT 'account_lifetime',
        PRIMARY KEY (user_id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX IF NOT EXISTS idx_user_devices_thumbprint ON user_devices (user_id, key_thumbprint)",
    "CREATE INDEX IF NOT EXISTS idx_user_devices_hint ON user_devices (user_id, continuity_hint)",
];

async fn has_column<C: ConnectionTrait>(
    conn: &C,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 FROM pragma_table_info(?) WHERE name = ?",
            [table.into(), column.into()],
        ))
        .await?;
    Ok(row.is_some())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        for (table, column, ty) in COLUMNS {
            if !has_column(&txn, table, column).await? {
                txn.execute_unprepared(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty}"))
                    .await?;
            }
        }
        for stmt in STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        for stmt in [
            "DROP INDEX IF EXISTS idx_user_devices_hint",
            "DROP INDEX IF EXISTS idx_user_devices_thumbprint",
            "DROP TABLE IF EXISTS user_device_salts",
        ] {
            txn.execute_unprepared(stmt).await?;
        }
        for (table, column, _) in COLUMNS {
            if has_column(&txn, table, column).await? {
                txn.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN {column}"))
                    .await?;
            }
        }
        txn.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    #[tokio::test]
    async fn up_twice_is_safe_after_a_full_run() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();
        for column in [
            "public_key",
            "key_thumbprint",
            "key_store",
            "continuity_hint",
            "hint_conflict",
            "merged_key_thumbprint",
        ] {
            assert!(has_column(&db, "user_devices", column).await.unwrap());
        }
        assert!(has_column(&db, "user_device_salts", "salt").await.unwrap());
    }

    #[tokio::test]
    async fn down_then_up_restores_the_key_columns() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.down(&mgr).await.unwrap();
        assert!(!has_column(&db, "user_devices", "key_thumbprint")
            .await
            .unwrap());
        Migration.up(&mgr).await.unwrap();
        assert!(has_column(&db, "user_devices", "key_thumbprint")
            .await
            .unwrap());
    }
}
