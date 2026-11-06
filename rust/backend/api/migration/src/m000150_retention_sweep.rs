use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Retention sweeper (A32). No new tables: every swept table already carries
// `expires_at INTEGER` and `retention_class TEXT`. This adds the two indexes the
// sweeper's per-batch guards read: the live-ban check (`user_bans` by user and
// revoke state) and the audit-subject lookup (`audit_logs` by user).
//
// Every statement is IF NOT EXISTS and the block is one transaction, so a re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_user_bans_user_revoked ON user_bans (user_id, revoked_at)",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs (user_id)",
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
        let conn = manager.get_connection();
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_user_bans_user_revoked")
            .await?;
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_audit_logs_user_id")
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
    async fn up_twice_is_safe_and_creates_both_indexes() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let indexes = count(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN ('idx_user_bans_user_revoked', 'idx_audit_logs_user_id')",
        )
        .await;
        assert_eq!(indexes, 2);
    }
}
