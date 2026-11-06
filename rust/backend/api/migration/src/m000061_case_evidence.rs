use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Case evidence and legal holds (A25). Evidence tables: no FK to users, no
// ON DELETE CASCADE from users, every row carries epoch-second expiry and a
// retention class for the A32 sweeper. `case_evidence` is append-only and
// self-contained: each row is one snapshot object (comment, flags, user,
// sessions, views, media metadata, audit rows) copied before removal.
//
// `legal_holds.retention_class` names the hold rule:
//   csam_hold_1y              CSAM hold, 1 year, never auto-released
//   law_enforcement_90d       law-enforcement preservation, 90 days, renewable
//   enforcement_12m           enforcement record, 12 months
// `legal_hold_subjects` copies subject kind and id at hold time, no FK to users.
//
// Every statement is IF NOT EXISTS and the whole block is one transaction, so a
// re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS case_evidence (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        case_ref TEXT NOT NULL,
        kind TEXT NOT NULL,
        subject_id TEXT NOT NULL,
        snapshot TEXT NOT NULL,
        captured_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_case_evidence_case_ref ON case_evidence (case_ref)",
    "CREATE INDEX IF NOT EXISTS idx_case_evidence_expires_at ON case_evidence (expires_at)",
    "CREATE TABLE IF NOT EXISTS legal_holds (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        case_ref TEXT,
        authority TEXT NOT NULL,
        reason TEXT NOT NULL,
        owner TEXT NOT NULL,
        auto_release INTEGER NOT NULL,
        placed_at INTEGER NOT NULL,
        preserve_until INTEGER,
        renewals INTEGER NOT NULL DEFAULT 0,
        released_at INTEGER,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_legal_holds_expires_at ON legal_holds (expires_at)",
    "CREATE TABLE IF NOT EXISTS legal_hold_subjects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        hold_id INTEGER NOT NULL REFERENCES legal_holds (id),
        subject_kind TEXT NOT NULL,
        subject_id TEXT NOT NULL,
        subject_user_id INTEGER,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_legal_hold_subjects_hold ON legal_hold_subjects (hold_id)",
    "CREATE INDEX IF NOT EXISTS idx_legal_hold_subjects_subject ON legal_hold_subjects (subject_kind, subject_id)",
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
        conn.execute_unprepared("DROP TABLE IF EXISTS legal_hold_subjects")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS legal_holds")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS case_evidence")
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
    async fn up_twice_is_safe_and_evidence_tables_have_no_fk_to_users() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        for table in ["case_evidence", "legal_holds", "legal_hold_subjects"] {
            let fks = count(
                &db,
                &format!(
                    "SELECT COUNT(*) FROM pragma_foreign_key_list('{table}') WHERE \"table\" = 'users'"
                ),
            )
            .await;
            assert_eq!(fks, 0, "{table} must not reference users");
            let not_null = count(
                &db,
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name IN ('expires_at', 'retention_class') AND \"notnull\" = 1"
                ),
            )
            .await;
            assert_eq!(not_null, 2, "{table} expiry and class must be NOT NULL");
        }
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('legal_hold_subjects') WHERE \"table\" = 'legal_holds'"
            )
            .await,
            1
        );
    }
}
