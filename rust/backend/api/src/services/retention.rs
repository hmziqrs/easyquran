//! Retention sweeper (A32). Deletes rows past `expires_at` on the swept tables.
//!
//! Dark by default: `RETENTION_SWEEP_MODE` unset or unknown means off, and the task
//! never starts. `dry_run` counts eligible rows and deletes nothing. `enforce`
//! deletes, but only after a `dry_run` run has been recorded in `job_runs`.
//!
//! Every delete keeps rows under a live legal hold (A25, `legal_holds` and
//! `legal_hold_subjects`) and rows whose owning user has a live ban
//! (`user_bans.revoked_at IS NULL`). Rows with `retention_class = 'account_lifetime'`
//! are never swept. Batches are at most 500 rows, at most 20 per table per run,
//! with a 50 ms pause between batches. A failing table is logged and skipped.
//!
//! A canary row is inserted at boot with an expiry in the past. An enforced run
//! must delete it, or the run raises an alert. A run that deletes nothing while
//! eligible rows exist also raises an alert.
//!
//! Counts only reach logs, `job_runs.detail` and audit metadata. Row contents
//! never do.

use std::time::Duration;

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement};
use serde_json::json;
use tracing::{error, info, warn};

use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::services::job_log::{self, OUTCOME_ERROR, OUTCOME_OK};
use crate::state::AppState;

pub const MODE_ENV: &str = "RETENTION_SWEEP_MODE";
pub const JOB: &str = "retention_sweep";
pub const CANARY_JOB: &str = "retention_canary";
pub const INTERVAL_SECS: u64 = 6 * 3_600;
pub const JITTER_MAX_SECS: u64 = 300;
pub const BATCH_SIZE: i64 = 500;
pub const MAX_BATCHES_PER_TABLE: usize = 20;
const BATCH_PAUSE: Duration = Duration::from_millis(50);
const ACCOUNT_LIFETIME: &str = "account_lifetime";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    DryRun,
    Enforce,
}

impl Mode {
    /// Anything other than `dry_run` or `enforce` is off. Fail dark.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "dry_run" => Self::DryRun,
            "enforce" => Self::Enforce,
            _ => Self::Off,
        }
    }

    pub fn from_env() -> Self {
        Self::parse(&std::env::var(MODE_ENV).unwrap_or_default())
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::DryRun => "dry_run",
            Self::Enforce => "enforce",
        }
    }
}

/// A legal-hold subject kind and the SQL condition that ties a row to a subject.
/// The condition sees `s` (`legal_hold_subjects`) and `t` (the swept row).
pub struct Hold {
    pub kind: &'static str,
    pub match_sql: &'static str,
}

/// One swept table. `user_col` names the owning user for the live-ban guard.
pub struct Table {
    pub name: &'static str,
    pub user_col: Option<&'static str>,
    pub hold: Option<Hold>,
}

/// Tables with `expires_at` and `retention_class` columns. Operational tables have
/// no subject, so only the expiry and class apply to them.
pub const TABLES: &[Table] = &[
    Table {
        name: "rate_limit_events",
        user_col: None,
        hold: None,
    },
    Table {
        name: "job_runs",
        user_col: None,
        hold: None,
    },
    Table {
        name: "newsletter_subscribers",
        user_col: None,
        hold: None,
    },
    Table {
        name: "media_deletions",
        user_col: Some("uploader_id"),
        hold: Some(Hold {
            kind: "media",
            match_sql: "s.subject_id = CAST(t.media_id AS TEXT)",
        }),
    },
    Table {
        name: "audit_logs",
        user_col: Some("user_id"),
        hold: Some(Hold {
            kind: "user",
            match_sql: "s.subject_id = CAST(t.user_id AS TEXT) \
                OR (t.resource_type = 'user' AND s.subject_id = t.resource_id)",
        }),
    },
];

/// The WHERE clause for one table: expired, not account-lifetime, and every guard.
fn where_clause(table: &Table, now: i64) -> String {
    let mut clause = format!("t.expires_at < {now} AND t.retention_class <> '{ACCOUNT_LIFETIME}'");
    if let Some(col) = table.user_col {
        clause.push_str(&format!(
            " AND NOT EXISTS (SELECT 1 FROM user_bans b \
             WHERE b.user_id = t.{col} AND b.revoked_at IS NULL)"
        ));
    }
    if let Some(hold) = &table.hold {
        clause.push_str(&format!(
            " AND NOT EXISTS (SELECT 1 FROM legal_holds h \
             JOIN legal_hold_subjects s ON s.hold_id = h.id \
             WHERE h.released_at IS NULL AND (h.auto_release = 0 OR h.expires_at > {now}) \
             AND s.subject_kind = '{kind}' AND ({m}))",
            kind = hold.kind,
            m = hold.match_sql,
        ));
    }
    clause
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableReport {
    pub name: &'static str,
    pub eligible: i64,
    pub deleted: i64,
    pub failed: bool,
}

#[derive(Debug, Default)]
pub struct Report {
    pub tables: Vec<TableReport>,
    pub alerts: Vec<String>,
}

impl Report {
    pub fn deleted_total(&self) -> i64 {
        self.tables.iter().map(|t| t.deleted).sum()
    }

    pub fn failed(&self) -> bool {
        self.tables.iter().any(|t| t.failed)
    }

    /// `name:eligible` pairs, counts only.
    pub fn eligible_summary(&self) -> String {
        self.tables
            .iter()
            .map(|t| format!("{}:{}", t.name, t.eligible))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Alert when an enforced table had eligible rows and deleted none.
pub fn zero_delete_alert(name: &str, eligible: i64, deleted: i64) -> Option<String> {
    if eligible > 0 && deleted == 0 {
        return Some(format!("zero_delete table={name} eligible={eligible}"));
    }
    None
}

async fn count(db: &DatabaseConnection, sql: &str) -> Result<i64, DbErr> {
    let Some(row) = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await?
    else {
        return Ok(0);
    };
    row.try_get_by_index::<i64>(0)
}

async fn sweep_table(
    db: &DatabaseConnection,
    table: &Table,
    now: i64,
    delete: bool,
) -> Result<TableReport, DbErr> {
    let name = table.name;
    let clause = where_clause(table, now);
    let eligible = count(
        db,
        &format!("SELECT COUNT(*) FROM {name} AS t WHERE {clause}"),
    )
    .await?;
    let mut deleted = 0;
    if delete {
        let sql = format!(
            "DELETE FROM {name} WHERE id IN \
             (SELECT t.id FROM {name} AS t WHERE {clause} LIMIT {BATCH_SIZE})"
        );
        for _ in 0..MAX_BATCHES_PER_TABLE {
            let n = db
                .execute(Statement::from_string(DatabaseBackend::Sqlite, sql.clone()))
                .await?
                .rows_affected() as i64;
            deleted += n;
            if n < BATCH_SIZE {
                break;
            }
            tokio::time::sleep(BATCH_PAUSE).await;
        }
    }
    Ok(TableReport {
        name,
        eligible,
        deleted,
        failed: false,
    })
}

/// One pass over every table. `delete` false is a dry run: counts only.
pub async fn run_once(db: &DatabaseConnection, delete: bool, now: i64) -> Report {
    let mut report = Report::default();
    for table in TABLES {
        match sweep_table(db, table, now, delete).await {
            Ok(tr) => {
                let alert = zero_delete_alert(tr.name, tr.eligible, tr.deleted);
                if let Some(alert) = alert.filter(|_| delete) {
                    error!(target: "retention_alert", %alert, "retention alert");
                    report.alerts.push(alert);
                }
                report.tables.push(tr);
            }
            Err(e) => {
                warn!(table = table.name, error = %e, "retention table failed (non-fatal)");
                report.tables.push(TableReport {
                    name: table.name,
                    eligible: 0,
                    deleted: 0,
                    failed: true,
                });
            }
        }
    }
    report
}

/// Insert the boot canary. Its expiry is already past, so the first enforced run
/// must delete it.
pub async fn insert_canary(db: &DatabaseConnection, now: i64) -> Result<(), DbErr> {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO job_runs \
         (job, started_at, finished_at, outcome, detail, expires_at, retention_class) \
         VALUES (?, ?, ?, 'canary', NULL, ?, 'job_log_90d')",
        [CANARY_JOB.into(), now.into(), now.into(), (now - 1).into()],
    ))
    .await?;
    Ok(())
}

async fn dry_run_recorded(db: &DatabaseConnection) -> Result<bool, DbErr> {
    let n = count(
        db,
        &format!(
            "SELECT COUNT(*) FROM job_runs WHERE job = '{JOB}' AND outcome = '{OUTCOME_OK}' \
             AND detail LIKE 'mode=dry_run%'"
        ),
    )
    .await?;
    Ok(n > 0)
}

async fn canary_survived(db: &DatabaseConnection) -> Result<bool, DbErr> {
    let n = count(
        db,
        &format!("SELECT COUNT(*) FROM job_runs WHERE job = '{CANARY_JOB}'"),
    )
    .await?;
    Ok(n > 0)
}

/// One recorded run: gate check, sweep, canary check, audit row, `job_runs` row.
pub async fn run_recorded(db: &DatabaseConnection, requested: Mode, now: i64) -> Report {
    let started = job_log::epoch_now();
    let mut effective = requested;
    let mut gate_alerts = Vec::new();
    if requested == Mode::Enforce && !dry_run_recorded(db).await.unwrap_or(false) {
        let alert = "enforce_blocked no_prior_dry_run".to_string();
        error!(target: "retention_alert", %alert, "retention enforce blocked");
        gate_alerts.push(alert);
        effective = Mode::DryRun;
    }
    let delete = effective == Mode::Enforce;
    let mut report = run_once(db, delete, now).await;
    report.alerts.extend(gate_alerts);

    if delete && canary_survived(db).await.unwrap_or(false) {
        let alert = "canary_survived".to_string();
        error!(target: "retention_alert", %alert, "retention canary not deleted");
        report.alerts.push(alert);
    }
    if delete && report.deleted_total() > 0 {
        let res = audit::record(
            db,
            AuditEvent {
                actor_id: None,
                action: AuditAction::RetentionSweep,
                resource_id: JOB.to_string(),
                metadata: json!({ "deleted": report.deleted_total() }),
            },
        )
        .await;
        if let Err(e) = res {
            warn!(error = %e, "retention audit row failed (non-fatal)");
        }
    }

    let outcome = if report.failed() {
        OUTCOME_ERROR
    } else {
        OUTCOME_OK
    };
    let detail = format!(
        "mode={} eligible={} deleted={} alerts={}",
        effective.label(),
        report.eligible_summary(),
        report.deleted_total(),
        report.alerts.len()
    );
    info!(detail = %detail, "retention sweep finished");
    job_log::record(
        db,
        JOB,
        started,
        job_log::epoch_now(),
        outcome,
        Some(&detail),
    )
    .await;
    report
}

fn jitter_secs() -> u64 {
    use rand::Rng;
    rand::rng().random_range(0..=JITTER_MAX_SECS)
}

/// Spawned from `scheduler::start_scheduler`. Returns at once when the mode is off.
pub async fn run(state: AppState) {
    let mode = Mode::from_env();
    if mode == Mode::Off {
        info!("Retention sweeper off ({MODE_ENV} unset or off)");
        return;
    }
    if let Err(e) = insert_canary(&state.sea_db, job_log::epoch_now()).await {
        warn!(error = %e, "retention canary insert failed (non-fatal)");
    }
    info!(mode = mode.label(), "Retention sweeper started");
    let mut wait = jitter_secs();
    loop {
        tokio::time::sleep(Duration::from_secs(wait)).await;
        run_recorded(&state.sea_db, mode, job_log::epoch_now()).await;
        wait = INTERVAL_SECS + jitter_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sea_models::legal_hold::{actions, HoldRule, HoldSubject, NewHold};
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

    async fn test_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn exec(db: &DatabaseConnection, sql: &str) {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .unwrap();
    }

    async fn insert_media_deletion(
        db: &DatabaseConnection,
        media_id: i64,
        uploader: Option<i64>,
        expires_at: i64,
    ) {
        let uploader = uploader.map_or("NULL".to_string(), |u| u.to_string());
        exec(
            db,
            &format!(
                "INSERT INTO media_deletions (media_id, bucket, object_keys, uploader_id, \
                 deleted_by, deleted_at, expires_at, retention_class) \
                 VALUES ({media_id}, NULL, '[]', {uploader}, NULL, 0, {expires_at}, 'media_12m')"
            ),
        )
        .await;
    }

    async fn media_ids(db: &DatabaseConnection) -> Vec<i64> {
        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT media_id FROM media_deletions ORDER BY media_id".to_string(),
            ))
            .await
            .unwrap();
        rows.iter()
            .map(|r| r.try_get_by_index::<i64>(0).unwrap())
            .collect()
    }

    #[test]
    fn mode_defaults_off_and_unknown_is_off() {
        assert_eq!(Mode::parse(""), Mode::Off);
        assert_eq!(Mode::parse("on"), Mode::Off);
        assert_eq!(Mode::parse("ENFORCE "), Mode::Enforce);
        assert_eq!(Mode::parse("dry_run"), Mode::DryRun);
    }

    #[test]
    fn zero_delete_alert_only_when_eligible_and_none_deleted() {
        assert!(zero_delete_alert("job_runs", 3, 0).is_some());
        assert!(zero_delete_alert("job_runs", 0, 0).is_none());
        assert!(zero_delete_alert("job_runs", 3, 3).is_none());
    }

    #[test]
    fn every_swept_table_has_expiry_and_class_in_migrations() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/migration/src");
        let mut sources = String::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "rs") {
                sources.push_str(&std::fs::read_to_string(path).unwrap());
            }
        }
        for table in TABLES {
            assert!(
                sources.contains(table.name),
                "no migration for {}",
                table.name
            );
        }
        assert!(sources.contains("expires_at"));
        assert!(sources.contains("retention_class"));
    }

    #[tokio::test]
    async fn dry_run_counts_and_deletes_nothing() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_media_deletion(&db, 50, None, now - 10).await;

        let report = run_recorded(&db, Mode::DryRun, now).await;

        assert_eq!(media_ids(&db).await, vec![50]);
        let media = report
            .tables
            .iter()
            .find(|t| t.name == "media_deletions")
            .unwrap();
        assert_eq!(media.eligible, 1);
        assert_eq!(media.deleted, 0);
        let detail = count(
            &db,
            "SELECT COUNT(*) FROM job_runs WHERE job = 'retention_sweep' \
             AND detail LIKE 'mode=dry_run%eligible=%media_deletions:1%'",
        )
        .await
        .unwrap();
        assert_eq!(detail, 1);
    }

    #[tokio::test]
    async fn enforce_refused_without_recorded_dry_run() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_media_deletion(&db, 60, None, now - 10).await;

        let report = run_recorded(&db, Mode::Enforce, now).await;

        assert_eq!(media_ids(&db).await, vec![60]);
        assert!(report.alerts.iter().any(|a| a.contains("no_prior_dry_run")));
    }

    #[tokio::test]
    async fn canary_deleted_by_first_enforced_sweep_and_job_row_written() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        run_recorded(&db, Mode::DryRun, now).await;
        insert_canary(&db, now).await.unwrap();

        let report = run_recorded(&db, Mode::Enforce, now).await;

        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM job_runs WHERE job = 'retention_canary'"
            )
            .await
            .unwrap(),
            0
        );
        assert!(report.deleted_total() >= 1);
        assert!(report.alerts.is_empty());
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM job_runs WHERE job = 'retention_sweep'"
            )
            .await
            .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn row_under_legal_hold_survives_enforced_sweep() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_media_deletion(&db, 42, None, now - 10).await;
        insert_media_deletion(&db, 43, None, now - 10).await;
        actions::place(
            &db,
            NewHold {
                rule: HoldRule::Enforcement12m,
                authority: "test".into(),
                reason: "test".into(),
                owner: "test".into(),
                case_ref: None,
                subjects: vec![HoldSubject {
                    kind: "media".into(),
                    id: "42".into(),
                    user_id: None,
                }],
            },
            Utc::now(),
        )
        .await
        .unwrap();
        run_recorded(&db, Mode::DryRun, now).await;

        run_recorded(&db, Mode::Enforce, now).await;

        assert_eq!(media_ids(&db).await, vec![42]);
    }

    #[tokio::test]
    async fn row_under_live_ban_survives_and_revoked_ban_does_not_protect() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        exec(
            &db,
            "INSERT INTO user_bans (user_id, reason, created_at) \
             VALUES (7, 'test', '2026-01-01T00:00:00Z')",
        )
        .await;
        exec(
            &db,
            "INSERT INTO user_bans (user_id, reason, created_at, revoked_at) \
             VALUES (8, 'test', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
        )
        .await;
        insert_media_deletion(&db, 70, Some(7), now - 10).await;
        insert_media_deletion(&db, 80, Some(8), now - 10).await;
        run_recorded(&db, Mode::DryRun, now).await;

        run_recorded(&db, Mode::Enforce, now).await;

        assert_eq!(media_ids(&db).await, vec![70]);
    }

    #[tokio::test]
    async fn account_lifetime_rows_are_never_swept() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        exec(
            &db,
            &format!(
                "INSERT INTO media_deletions (media_id, bucket, object_keys, uploader_id, \
                 deleted_by, deleted_at, expires_at, retention_class) \
                 VALUES (90, NULL, '[]', NULL, NULL, 0, {}, 'account_lifetime')",
                now - 10
            ),
        )
        .await;
        run_recorded(&db, Mode::DryRun, now).await;

        run_recorded(&db, Mode::Enforce, now).await;

        assert_eq!(media_ids(&db).await, vec![90]);
    }
}
