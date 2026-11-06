use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// The multi-device protocol (contracts/sync.md): one stamp per register on each
// synced row, plus a server-assigned `rev` for delta pulls. NULL stamps rank
// lowest, so rows from before this migration lose to any real write. `rev` is
// per user and per domain, from `sync_counters`, which also holds the tombstone
// floor. Pull order is by `rev`, and lists and staleness use `cursor_stamp`.
const COLUMNS: &[(&str, &str, &str)] = &[
    ("reading_sessions", "identity_stamp", "TEXT"),
    ("reading_sessions", "cursor_stamp", "TEXT"),
    ("reading_sessions", "lifecycle_stamp", "TEXT"),
    ("reading_sessions", "state", "TEXT NOT NULL DEFAULT 'open'"),
    ("reading_sessions", "device_id", "TEXT"),
    ("reading_sessions", "rev", "INTEGER NOT NULL DEFAULT 0"),
    ("reading_progress", "rev", "INTEGER NOT NULL DEFAULT 0"),
    ("bookmarks", "stamp", "TEXT"),
    ("bookmarks", "rev", "INTEGER NOT NULL DEFAULT 0"),
    ("bookmark_folders", "name_stamp", "TEXT"),
    ("bookmark_folders", "position_stamp", "TEXT"),
    ("bookmark_folders", "deleted_stamp", "TEXT"),
    ("bookmark_folders", "rev", "INTEGER NOT NULL DEFAULT 0"),
];

const STMTS: &[&str] = &[
    "UPDATE reading_sessions SET state = 'deleted' WHERE deleted_at IS NOT NULL AND state = 'open'",
    "UPDATE reading_sessions SET state = 'ended' WHERE ended_at IS NOT NULL AND state = 'open'",
    "CREATE TABLE IF NOT EXISTS sync_counters (
        user_id INTEGER NOT NULL,
        domain TEXT NOT NULL,
        rev INTEGER NOT NULL DEFAULT 0,
        floor_rev INTEGER NOT NULL DEFAULT 0,
        floor_ms INTEGER NOT NULL DEFAULT 0,
        expires_at INTEGER NOT NULL DEFAULT 253402300799,
        retention_class TEXT NOT NULL DEFAULT 'account_lifetime',
        PRIMARY KEY (user_id, domain),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX IF NOT EXISTS idx_reading_sessions_user_rev ON reading_sessions (user_id, rev)",
    "CREATE INDEX IF NOT EXISTS idx_reading_sessions_user_cursor ON reading_sessions (user_id, cursor_stamp)",
    "CREATE INDEX IF NOT EXISTS idx_reading_progress_user_rev ON reading_progress (user_id, rev)",
    "CREATE INDEX IF NOT EXISTS idx_bookmarks_user_rev ON bookmarks (user_id, rev)",
    "CREATE INDEX IF NOT EXISTS idx_bookmark_folders_user_rev ON bookmark_folders (user_id, rev)",
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
            "DROP INDEX IF EXISTS idx_reading_sessions_user_rev",
            "DROP INDEX IF EXISTS idx_reading_sessions_user_cursor",
            "DROP INDEX IF EXISTS idx_reading_progress_user_rev",
            "DROP INDEX IF EXISTS idx_bookmarks_user_rev",
            "DROP INDEX IF EXISTS idx_bookmark_folders_user_rev",
            "DROP TABLE IF EXISTS sync_counters",
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
        assert!(has_column(&db, "bookmark_folders", "deleted_stamp")
            .await
            .unwrap());
        assert!(has_column(&db, "reading_sessions", "state").await.unwrap());
    }
}
