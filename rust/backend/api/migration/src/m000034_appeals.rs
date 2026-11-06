use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Appeal intake (A35). One appeal per enforcement notice (unique on notice_id), so
// a repeat submit is a no-op. due_at is the 7-day decision clock from submission.
// No FK to users or notices: the appeal is evidence and outlives the account, the
// same as enforcement_notices. The token itself is never stored: appeal_token_hash
// (added in A21's notices table shape) holds a keyed HMAC, and the partial unique
// index makes the lookup by hash a single probe.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS appeals (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        notice_id INTEGER NOT NULL,
        user_id INTEGER NOT NULL,
        statement TEXT NOT NULL CHECK (length(statement) BETWEEN 1 AND 2000),
        status TEXT NOT NULL CHECK (status IN ('open', 'decided')),
        submitted_at INTEGER NOT NULL,
        due_at INTEGER NOT NULL CHECK (due_at >= submitted_at),
        decided_at INTEGER,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_appeals_one_per_notice ON appeals (notice_id)",
    "CREATE INDEX IF NOT EXISTS idx_appeals_open_due ON appeals (status, due_at)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_enforcement_notices_appeal_token \
     ON enforcement_notices (appeal_token_hash) WHERE appeal_token_hash IS NOT NULL",
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

    // Drops nothing. Appeals are evidence, like the notices they answer.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
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
    async fn up_twice_is_safe_and_has_no_user_fk() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('appeals')"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'idx_appeals_one_per_notice'"
            )
            .await,
            1
        );
    }
}
