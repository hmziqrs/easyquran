//! One `job_runs` row per run of a periodic job (abuse measurement, A5).
//!
//! Only jobs with a real cadence (minutes or longer) call this. The 10 s
//! rate-limit flush does not: it already writes the durable state, and a row per
//! tick would be a write storm. Writes are best-effort: a failed insert is logged
//! and never fails the job.

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};
use tracing::warn;

/// Retention for `job_runs` rows. The sweeper that deletes them ships with A32.
pub const JOB_RUNS_RETENTION_SECS: i64 = 90 * 86_400;
pub const JOB_RUNS_RETENTION_CLASS: &str = "job_log_90d";

pub const OUTCOME_OK: &str = "ok";
pub const OUTCOME_ERROR: &str = "error";

/// Epoch seconds now. Clock failure reads as 0 rather than panicking a job.
pub fn epoch_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Insert one run record. `detail` must carry counts or short codes only, never
/// identifiers from user data.
pub async fn record(
    db: &DatabaseConnection,
    job: &str,
    started_at: i64,
    finished_at: i64,
    outcome: &str,
    detail: Option<&str>,
) {
    let result = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO job_runs \
             (job, started_at, finished_at, outcome, detail, expires_at, retention_class) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            vec![
                Value::from(job),
                Value::from(started_at),
                Value::from(finished_at),
                Value::from(outcome),
                Value::from(detail.map(str::to_string)),
                Value::from(finished_at + JOB_RUNS_RETENTION_SECS),
                Value::from(JOB_RUNS_RETENTION_CLASS),
            ],
        ))
        .await;
    if let Err(e) = result {
        warn!(job, error = %e, "failed to record job run (non-fatal)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

    #[tokio::test]
    async fn record_writes_row_with_retention_fields() {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();

        record(&db, "job_under_test", 100, 105, OUTCOME_OK, Some("due=0")).await;
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT job, outcome, detail, expires_at, retention_class FROM job_runs"
                    .to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get_by_index::<String>(0).unwrap(), "job_under_test");
        assert_eq!(row.try_get_by_index::<String>(1).unwrap(), "ok");
        assert_eq!(row.try_get_by_index::<String>(2).unwrap(), "due=0");
        assert_eq!(
            row.try_get_by_index::<i64>(3).unwrap(),
            105 + JOB_RUNS_RETENTION_SECS
        );
        assert_eq!(row.try_get_by_index::<String>(4).unwrap(), "job_log_90d");
    }

    #[tokio::test]
    async fn record_never_panics_when_table_is_missing() {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        // No migration: the insert fails and must be swallowed.
        record(&db, "orphan", 1, 2, OUTCOME_ERROR, None).await;
    }
}
