use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Erasure receipts (A33). One row per subject erasure: the subject ref (kind and id, no
// digest), the scope, `retained` (JSON list of what a live hold kept, with basis and
// expiry), and the write time. Boot replay reads this table to re-delete rows that a
// restore brought back. No FK to users: the subject row is the thing being erased.
// 24-month retention class.
//
// Every statement is IF NOT EXISTS and the block is one transaction, so a re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS erasure_receipts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        subject_kind TEXT NOT NULL,
        subject_id TEXT NOT NULL,
        scope TEXT NOT NULL,
        retained TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_erasure_receipts_subject ON erasure_receipts (subject_kind, subject_id)",
    "CREATE INDEX IF NOT EXISTS idx_erasure_receipts_expires_at ON erasure_receipts (expires_at)",
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
            .execute_unprepared("DROP TABLE IF EXISTS erasure_receipts")
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
    async fn up_twice_is_safe_and_receipts_have_no_fk_and_expiry_not_null() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let fks = count(
            &db,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('erasure_receipts')",
        )
        .await;
        assert_eq!(fks, 0, "erasure_receipts must not reference any table");
        let not_null = count(
            &db,
            "SELECT COUNT(*) FROM pragma_table_info('erasure_receipts') WHERE name IN ('expires_at', 'retention_class', 'subject_id', 'retained', 'created_at') AND \"notnull\" = 1",
        )
        .await;
        assert_eq!(not_null, 5);
    }
}
