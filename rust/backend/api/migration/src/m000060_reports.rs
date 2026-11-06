use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Incident intake (A24). `reports` holds what readers send through /report.
// Evidence table: no FK to users (the reporter id stays as a plain value and
// survives an account delete), no IP or device column. `snapshot` holds the
// reported item captured at intake for P0 classes (csam, threat). Retention:
// 12 months, expires_at is epoch seconds for the sweeper (A32).
//
// Every statement is IF NOT EXISTS and the whole block is one transaction, so a
// re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS reports (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        reporter_user_id INTEGER,
        target_kind TEXT NOT NULL,
        target_id TEXT NOT NULL,
        reason TEXT NOT NULL,
        priority TEXT NOT NULL,
        details TEXT,
        contact_email TEXT,
        snapshot TEXT,
        status TEXT NOT NULL DEFAULT 'open',
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_reports_status_created ON reports (status, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_reports_expires_at ON reports (expires_at)",
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
            .execute_unprepared("DROP TABLE IF EXISTS reports")
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
    async fn up_twice_is_safe_and_table_has_no_fk_or_ip_column() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('reports')"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('reports') WHERE name IN ('ip_address', 'user_agent', 'device_id')"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('reports') WHERE name IN ('expires_at', 'retention_class') AND \"notnull\" = 1"
            )
            .await,
            2
        );
    }
}
