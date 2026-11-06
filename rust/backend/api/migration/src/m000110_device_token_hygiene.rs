use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Device token hygiene (A36). Push tokens that Firebase rejects with a non-terminal
// error (INVALID_ARGUMENT, SENDER_ID_MISMATCH) count strikes and prune on the second
// consecutive failure. `invalid_strikes` is a constant default 0; `invalid_at` is
// NULL until the first strike. Both are additive and populated-table safe.
//
// SQLite has no ADD COLUMN IF NOT EXISTS, so each add is guarded by a column probe.
const ADD_STRIKES: &str =
    "ALTER TABLE devices ADD COLUMN invalid_strikes INTEGER NOT NULL DEFAULT 0";
const ADD_INVALID_AT: &str = "ALTER TABLE devices ADD COLUMN invalid_at TEXT";

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
        if !has_column(&txn, "devices", "invalid_strikes").await? {
            txn.execute_unprepared(ADD_STRIKES).await?;
        }
        if !has_column(&txn, "devices", "invalid_at").await? {
            txn.execute_unprepared(ADD_INVALID_AT).await?;
        }
        txn.commit().await
    }

    // Keeps the columns on rollback: strike counts are not evidence, and dropping
    // columns on a live table would need a rebuild for no gain.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection};
    use sea_orm_migration::MigratorTrait;

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                sql.to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    #[tokio::test]
    async fn up_twice_adds_columns_once_with_zero_default() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'invalid_strikes' AND \"notnull\" = 1 AND dflt_value = '0'"
            )
            .await,
            1
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('devices') WHERE name = 'invalid_at'"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn existing_rows_get_zero_strikes_and_no_invalid_at() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        // No users row here: the legacy row only needs the new columns' defaults.
        db.execute_unprepared("PRAGMA foreign_keys = OFF")
            .await
            .unwrap();
        db.execute_unprepared(
            "INSERT INTO devices (user_id, token, platform, created_at, updated_at, last_seen_at)
             VALUES (1, 'legacy-token', 'web', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
        )
        .await
        .unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM devices WHERE token = 'legacy-token' AND invalid_strikes = 0 AND invalid_at IS NULL"
            )
            .await,
            1
        );
    }
}
