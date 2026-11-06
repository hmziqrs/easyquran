use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Evidence-safe audit_logs (A23). m000001 gave audit_logs a FK to users with
// ON DELETE SET NULL, so an account delete erased who acted. This rebuilds the
// table with no foreign keys (same shape as enforcement_notices) and adds the
// evidence columns: expires_at (epoch seconds, sweeper key) and retention_class.
//
// Existing rows get a 12-month class, expiry = created_at + 365 days. A row
// whose created_at does not parse falls back to migration time, so the NOT NULL
// column never fails the rebuild. No row is deleted.
//
// Re-runnable: the rebuild runs on every call, and the copy reads only columns
// that exist in both shapes, so a second run over the new shape loses nothing.
// The whole rebuild is one transaction.
const CREATE_REBUILT: &str = "CREATE TABLE IF NOT EXISTS audit_logs_rebuilt (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    metadata TEXT,
    ip_address TEXT,
    user_agent TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at INTEGER NOT NULL,
    retention_class TEXT NOT NULL
)";

const COPY_ROWS: &str = "INSERT OR IGNORE INTO audit_logs_rebuilt \
     (id, user_id, action, resource_type, resource_id, metadata, ip_address, user_agent, \
      created_at, expires_at, retention_class) \
     SELECT id, user_id, action, resource_type, resource_id, metadata, ip_address, user_agent, \
      created_at, \
      CAST(COALESCE(strftime('%s', created_at), strftime('%s', 'now')) AS INTEGER) + 31536000, \
      'audit_12m' \
     FROM audit_logs";

// Carry the id high-water mark across the rebuild so an audit id is never reused.
// sqlite_sequence has no unique key on name, hence the NOT EXISTS guard.
const SEQ_CARRY_INSERT: &str = "INSERT INTO sqlite_sequence (name, seq) \
     SELECT 'audit_logs_rebuilt', 0 \
     WHERE EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'audit_logs') \
       AND NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'audit_logs_rebuilt')";
const SEQ_CARRY_UPDATE: &str = "UPDATE sqlite_sequence SET seq = MAX(seq, \
     (SELECT seq FROM sqlite_sequence WHERE name = 'audit_logs')) \
     WHERE name = 'audit_logs_rebuilt'";

const FINISH: &[&str] = &[
    "DROP TABLE IF EXISTS audit_logs",
    "ALTER TABLE audit_logs_rebuilt RENAME TO audit_logs",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs (resource_type, resource_id)",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at)",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_expires_at ON audit_logs (expires_at)",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs (user_id)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared(CREATE_REBUILT).await?;
        txn.execute_unprepared(COPY_ROWS).await?;
        txn.execute_unprepared(SEQ_CARRY_INSERT).await?;
        txn.execute_unprepared(SEQ_CARRY_UPDATE).await?;
        for stmt in FINISH {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    // Keeps the evidence-safe shape and every row. Rolling back does not
    // restore the cascade.
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

    async fn connect() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn up_twice_is_safe_after_a_full_run() {
        let db = connect().await;
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('audit_logs')"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('audit_logs') WHERE name IN ('expires_at', 'retention_class')"
            )
            .await,
            2
        );
    }

    #[tokio::test]
    async fn rebuild_keeps_rows_backfills_expiry_and_drops_user_fk() {
        let db = connect().await;
        // Everything up to m000032, then the old shape with a row and a user FK.
        // Counted by name, so migrations appended after this one do not shift it.
        let upto = Migrator::migrations()
            .iter()
            .position(|m| m.name() == "m000032_user_bans_evidence_safe")
            .unwrap() as u32
            + 1;
        Migrator::up(&db, Some(upto)).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('audit_logs')"
            )
            .await,
            1,
            "old shape has the user FK"
        );
        db.execute_unprepared(
            "INSERT INTO users (id, name, email, role, is_verified, two_fa_enabled, \
             session_auth_secret, created_at, updated_at) VALUES \
             (7, 'u7', 'u7@x.test', 0, 1, 0, 's', \
             '2024-01-01T00:00:00+00:00', '2024-01-01T00:00:00+00:00')",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at) VALUES \
             (1, 7, 'ban.create', 'user_ban', '3', '2024-01-01T00:00:00+00:00')",
        )
        .await
        .unwrap();

        Migrator::up(&db, None).await.unwrap();

        assert_eq!(count(&db, "SELECT COUNT(*) FROM audit_logs").await, 1);
        assert_eq!(
            count(
                &db,
                "SELECT expires_at - CAST(strftime('%s', '2024-01-01T00:00:00+00:00') AS INTEGER) FROM audit_logs WHERE id = 1"
            )
            .await,
            31536000,
            "12-month expiry from created_at"
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM audit_logs WHERE id = 1 AND retention_class = 'audit_12m'"
            )
            .await,
            1
        );

        // Deleting the actor keeps the audit row; the actor id stays as evidence.
        db.execute_unprepared("DELETE FROM users WHERE id = 7")
            .await
            .unwrap();
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM audit_logs WHERE user_id = 7").await,
            1
        );
    }
}
