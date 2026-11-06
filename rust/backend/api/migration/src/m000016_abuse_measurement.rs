use sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Abuse measurement (A5). Two append-only ops tables, app SQLite only.
//
// `rate_limit_events` is the durable block ledger. `rate_limit_state` deletes a
// row once its window and block both expire, so this table is the only durable
// record of a block. One row per TRANSITION, never per request: `block_created`
// when a block appears (block_until_at 0 -> >0), `block_expired` once when it
// lapses. Written inside the 10 s flush transaction. Retention 180 days.
//
// `job_runs` records one row per run of each periodic job with a real cadence.
// Retention 90 days. Both carry `expires_at` and `retention_class` per the
// migration rules; the sweeper that deletes expired rows ships with A32.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS rate_limit_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        key TEXT NOT NULL,
        key_class TEXT NOT NULL,
        scope TEXT NOT NULL,
        event TEXT NOT NULL,
        issued_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_rate_limit_events_event_class_issued \
     ON rate_limit_events (event, key_class, issued_at)",
    "CREATE INDEX IF NOT EXISTS idx_rate_limit_events_expires_at \
     ON rate_limit_events (expires_at)",
    "CREATE TABLE IF NOT EXISTS job_runs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        job TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        finished_at INTEGER NOT NULL,
        outcome TEXT NOT NULL,
        detail TEXT,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_job_runs_job_started ON job_runs (job, started_at)",
    "CREATE INDEX IF NOT EXISTS idx_job_runs_expires_at ON job_runs (expires_at)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.transaction::<_, (), DbErr>(|txn| {
            Box::pin(async move {
                for stmt in STMTS {
                    txn.execute_unprepared(stmt).await?;
                }
                Ok(())
            })
        })
        .await
        .map_err(|e| match e {
            sea_orm::TransactionError::Connection(e)
            | sea_orm::TransactionError::Transaction(e) => e,
        })
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("DROP TABLE IF EXISTS job_runs")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS rate_limit_events")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, Value};
    use sea_orm_migration::SchemaManager;

    async fn mem_db() -> sea_orm::DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        Database::connect(opt).await.unwrap()
    }

    async fn columns(db: &sea_orm::DatabaseConnection, table: &str) -> Vec<String> {
        db.query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("PRAGMA table_info({table})"),
            Vec::<Value>::new(),
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.try_get_by_index::<String>(1).unwrap())
        .collect()
    }

    #[tokio::test]
    async fn creates_both_tables_with_retention_columns() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let events = columns(&db, "rate_limit_events").await;
        for c in [
            "id",
            "key",
            "key_class",
            "scope",
            "event",
            "issued_at",
            "expires_at",
            "retention_class",
        ] {
            assert!(
                events.iter().any(|x| x == c),
                "rate_limit_events missing {c}"
            );
        }
        let jobs = columns(&db, "job_runs").await;
        for c in [
            "id",
            "job",
            "started_at",
            "finished_at",
            "outcome",
            "detail",
            "expires_at",
            "retention_class",
        ] {
            assert!(jobs.iter().any(|x| x == c), "job_runs missing {c}");
        }
    }

    #[tokio::test]
    async fn up_twice_is_idempotent_and_keeps_rows() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        db.execute_unprepared(
            "INSERT INTO job_runs (job, started_at, finished_at, outcome, expires_at, retention_class) \
             VALUES ('j', 1, 2, 'ok', 3, 'job_log_90d')",
        )
        .await
        .unwrap();
        Migration.up(&mgr).await.unwrap();

        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM job_runs".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get_by_index::<i64>(0).unwrap(), 1);
    }

    #[tokio::test]
    async fn not_null_expiry_and_retention_are_enforced() {
        let db = mem_db().await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let missing_expiry = db
            .execute_unprepared(
                "INSERT INTO rate_limit_events (key, key_class, scope, event, issued_at, retention_class) \
                 VALUES ('k', 'ip4_unit', 'Temp', 'block_created', 1, 'abuse_180d')",
            )
            .await;
        assert!(missing_expiry.is_err(), "expires_at must be NOT NULL");
    }
}
