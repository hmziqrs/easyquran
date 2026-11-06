use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Device registry (devices.md D2). `user_devices` is the account's device list,
// keyed (user_id, id) so one client id under two accounts is two unrelated rows.
// Push tokens (`devices`) and sessions (`user_sessions`) gain a nullable
// `device_id`: a device owns its tokens and its sessions. Rows expire 90 days
// after last seen; the sweeper (A32) deletes them, so retention is not account
// lifetime. Nothing here stores UA, screen, model or IP.
const COLUMNS: &[(&str, &str, &str)] = &[
    ("devices", "device_id", "TEXT"),
    ("user_sessions", "device_id", "TEXT"),
];

const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS user_devices (
        user_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        display_name TEXT,
        platform TEXT NOT NULL,
        client TEXT NOT NULL,
        form_factor TEXT NOT NULL,
        browser TEXT,
        os_major INTEGER,
        app_version TEXT,
        name_stamp TEXT,
        merged_into TEXT,
        created_at TEXT NOT NULL,
        last_seen_at TEXT NOT NULL,
        retired_at TEXT,
        revoked_at TEXT,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX IF NOT EXISTS idx_user_devices_expires ON user_devices (expires_at)",
    "CREATE INDEX IF NOT EXISTS idx_devices_user_device ON devices (user_id, device_id)",
    "CREATE INDEX IF NOT EXISTS idx_user_sessions_user_device ON user_sessions (user_id, device_id)",
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
            "DROP INDEX IF EXISTS idx_user_sessions_user_device",
            "DROP INDEX IF EXISTS idx_devices_user_device",
            "DROP INDEX IF EXISTS idx_user_devices_expires",
            "DROP TABLE IF EXISTS user_devices",
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
        assert!(has_column(&db, "devices", "device_id").await.unwrap());
        assert!(has_column(&db, "user_sessions", "device_id").await.unwrap());
        assert!(has_column(&db, "user_devices", "expires_at").await.unwrap());
        assert!(has_column(&db, "user_devices", "retention_class")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn down_then_up_restores_the_registry() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.down(&mgr).await.unwrap();
        assert!(!has_column(&db, "devices", "device_id").await.unwrap());
        Migration.up(&mgr).await.unwrap();
        assert!(has_column(&db, "user_sessions", "device_id").await.unwrap());
    }
}
