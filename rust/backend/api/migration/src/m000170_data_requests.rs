use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Data requests (A38). One row per subject request with its one-month response clock:
// `due_at` = `received_at` + 30 days. `closed_at` and `receipt_id` are set when the
// erase answers it. `subject_id` is text, like the receipts it links to. No FK to any
// table: the request outlives the subject row it names. One open request per subject.
// 24-month retention class, the same as erasure receipts.
//
// Every statement is IF NOT EXISTS and the block is one transaction, so a re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS data_requests (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        subject_kind TEXT NOT NULL,
        subject_id TEXT NOT NULL,
        origin TEXT NOT NULL,
        received_at INTEGER NOT NULL,
        due_at INTEGER NOT NULL,
        closed_at INTEGER,
        receipt_id INTEGER,
        opened_by INTEGER,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_data_requests_queue ON data_requests (closed_at, due_at)",
    "CREATE INDEX IF NOT EXISTS idx_data_requests_expires_at ON data_requests (expires_at)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_data_requests_one_open ON data_requests (subject_kind, subject_id) WHERE closed_at IS NULL",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        for stmt in STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS data_requests")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DbBackend, Statement};
    use sea_orm_migration::MigratorTrait;

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    #[tokio::test]
    async fn up_twice_is_safe_and_requests_have_no_fk_and_expiry_not_null() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let fks = count(
            &db,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('data_requests')",
        )
        .await;
        assert_eq!(fks, 0, "data_requests must not reference any table");
        let not_null = count(
            &db,
            "SELECT COUNT(*) FROM pragma_table_info('data_requests') WHERE name IN ('expires_at', 'retention_class', 'subject_id', 'received_at', 'due_at') AND \"notnull\" = 1",
        )
        .await;
        assert_eq!(not_null, 5);
    }
}
