//! Retention sweeper (A32). Deletes rows past their retention on the swept tables.
//!
//! Dark by default: `RETENTION_SWEEP_MODE` unset or unknown means off, and the task
//! never starts. `dry_run` counts eligible rows and deletes nothing. `enforce`
//! deletes, but only after a `dry_run` run has been recorded in `job_runs`.
//!
//! Two kinds of rule per table:
//! - `ExpiresAt`: the row's own `expires_at` epoch column (new tables and the
//!   tables that already carried it).
//! - `AgeDays`: age of the row's own timestamp column. Existing tables get no new
//!   column; the rule reads what is already there.
//!
//! Every delete keeps rows under a live legal hold (A25) on the row's subject,
//! and rows whose owning user has a live ban (`user_bans.revoked_at IS NULL`).
//! Rows with `retention_class = 'account_lifetime'` are never swept. Batches are at
//! most 500 rows, at most 20 per table per run, with a 50 ms pause between
//! batches. A failing table is logged, reported and skipped.
//!
//! Sync tombstones (`tombstone_domain`): before a table's rows are deleted, the
//! domain's `floor_rev` rises to the highest deleted `rev` and, for bookmarks, the
//! `floor_ms` rises to the 180-day cutoff. Both happen in one transaction that
//! commits before the first delete. A client below the floor does a full pull, and
//! a stale write cannot recreate a deleted row.
//!
//! A canary row is inserted at boot with an expiry in the past. An enforced run
//! must delete it, or the run raises an alert. A run that deletes nothing while
//! eligible rows exist also raises an alert. Alerts go to the operator through
//! `operator_alert::send_retention_alert`.
//!
//! Counts only reach logs, `job_runs.detail`, alerts and audit metadata. Row
//! contents never do.

use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, TransactionTrait,
};
use serde_json::json;
use tracing::{error, info, warn};

use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::services::job_log::{self, OUTCOME_ERROR, OUTCOME_OK};
use crate::services::operator_alert;
use crate::state::AppState;

pub const MODE_ENV: &str = "RETENTION_SWEEP_MODE";
pub const JOB: &str = "retention_sweep";
pub const CANARY_JOB: &str = "retention_canary";
pub const INTERVAL_SECS: u64 = 6 * 3_600;
pub const JITTER_MAX_SECS: u64 = 300;
pub const BATCH_SIZE: i64 = 500;
pub const MAX_BATCHES_PER_TABLE: usize = 20;
/// Sync tombstone retention (contracts/sync.md: clean rows and tombstones, 180 days).
pub const TOMBSTONE_DAYS: i64 = 180;
const BATCH_PAUSE: Duration = Duration::from_millis(50);
const ACCOUNT_LIFETIME: &str = "account_lifetime";
const BOOKMARKS_DOMAIN: &str = "bookmarks";

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

#[derive(Clone, Copy, Debug)]
pub enum Expiry {
    /// `expires_at` epoch column, with the account-lifetime skip.
    ExpiresAt,
    /// Age in days of a julian-day expression over the row's own timestamp.
    /// `expr` is a fixed SQL expression over alias `t`, never input.
    AgeDays { expr: &'static str, days: i64 },
}

/// A legal-hold subject kind and the SQL condition that ties a row to a subject.
/// The condition sees `s` (`legal_hold_subjects`) and `t` (the swept row). An empty
/// `kind` matches any subject kind, and the condition must then name it.
pub struct Hold {
    pub kind: &'static str,
    pub match_sql: &'static str,
}

/// One swept table. `user_col` names the owning user: the live-ban guard and a
/// `user` legal-hold guard both key on it.
pub struct Table {
    pub name: &'static str,
    pub expiry: Expiry,
    pub extra: Option<&'static str>,
    pub user_col: Option<&'static str>,
    pub hold: Option<Hold>,
    pub tombstone_domain: Option<&'static str>,
}

const TOMBSTONE_AGE: Expiry = Expiry::AgeDays {
    expr: "julianday(t.deleted_at)",
    days: TOMBSTONE_DAYS,
};

/// Swept tables. `t` aliases the row. Expiry and class columns are named by the
/// migration that creates each table; see the source-scan test.
pub const TABLES: &[Table] = &[
    // Operational tables with no subject.
    Table {
        name: "rate_limit_events",
        expiry: Expiry::ExpiresAt,
        extra: None,
        user_col: None,
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "job_runs",
        expiry: Expiry::ExpiresAt,
        extra: None,
        user_col: None,
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "newsletter_subscribers",
        expiry: Expiry::ExpiresAt,
        extra: None,
        user_col: None,
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "media_deletions",
        expiry: Expiry::ExpiresAt,
        extra: None,
        user_col: Some("uploader_id"),
        hold: Some(Hold {
            kind: "media",
            match_sql: "s.subject_id = CAST(t.media_id AS TEXT)",
        }),
        tombstone_domain: None,
    },
    Table {
        name: "audit_logs",
        expiry: Expiry::ExpiresAt,
        extra: None,
        user_col: Some("user_id"),
        hold: Some(Hold {
            kind: "user",
            match_sql: "s.subject_id = t.resource_id AND t.resource_type = 'user'",
        }),
        tombstone_domain: None,
    },
    // Account data, 90 days.
    Table {
        name: "user_sessions",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.last_seen)",
            days: 90,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "devices",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.last_seen_at)",
            days: 90,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "user_devices",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.last_seen_at)",
            days: 90,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "post_views",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.created_at)",
            days: 90,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
    // Sync tombstones, 180 days. Floor advances first (see `advance_floors`).
    Table {
        name: "reading_sessions",
        expiry: TOMBSTONE_AGE,
        extra: Some("t.deleted_at IS NOT NULL"),
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: Some("sessions"),
    },
    Table {
        name: "bookmarks",
        expiry: TOMBSTONE_AGE,
        extra: Some("t.deleted_at IS NOT NULL"),
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: Some(BOOKMARKS_DOMAIN),
    },
    Table {
        name: "bookmark_folders",
        expiry: TOMBSTONE_AGE,
        extra: Some("t.deleted_at IS NOT NULL"),
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: Some(BOOKMARKS_DOMAIN),
    },
    // Moderation and evidence, 12 months.
    Table {
        name: "user_bans",
        expiry: Expiry::AgeDays {
            expr: "MAX(julianday(t.created_at), IFNULL(julianday(t.revoked_at), 0), IFNULL(julianday(t.expires_at), 0))",
            days: 365,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "enforcement_notices",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.created_at, 'unixepoch')",
            days: 365,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
    Table {
        name: "case_evidence",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.captured_at, 'unixepoch')",
            days: 365,
        },
        extra: None,
        user_col: None,
        hold: Some(Hold {
            kind: "",
            match_sql: "s.subject_kind = t.kind AND s.subject_id = t.subject_id",
        }),
        tombstone_domain: None,
    },
    Table {
        name: "reports",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.created_at)",
            days: 365,
        },
        extra: None,
        user_col: None,
        hold: Some(Hold {
            kind: "",
            match_sql: "s.subject_kind = t.target_kind AND s.subject_id = t.target_id",
        }),
        tombstone_domain: None,
    },
    Table {
        name: "post_comments",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.updated_at)",
            days: 365,
        },
        extra: Some("t.hidden = 1"),
        user_col: Some("user_id"),
        hold: Some(Hold {
            kind: "comment",
            match_sql: "s.subject_id = CAST(t.id AS TEXT)",
        }),
        tombstone_domain: None,
    },
    Table {
        name: "comment_flags",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.created_at)",
            days: 365,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: Some(Hold {
            kind: "comment",
            match_sql: "s.subject_id = CAST(t.comment_id AS TEXT)",
        }),
        tombstone_domain: None,
    },
    Table {
        name: "notifications",
        expiry: Expiry::AgeDays {
            expr: "julianday(t.created_at)",
            days: 365,
        },
        extra: None,
        user_col: Some("user_id"),
        hold: None,
        tombstone_domain: None,
    },
];

/// Legal-hold guard. `kind` empty means no kind filter.
fn hold_clause(kind: &str, match_sql: &str, now: i64) -> String {
    let kind_sql = if kind.is_empty() {
        String::new()
    } else {
        format!("s.subject_kind = '{kind}' AND ")
    };
    format!(
        " AND NOT EXISTS (SELECT 1 FROM legal_holds h \
         JOIN legal_hold_subjects s ON s.hold_id = h.id \
         WHERE h.released_at IS NULL AND (h.auto_release = 0 OR h.expires_at > {now}) \
         AND {kind_sql}({match_sql}))"
    )
}

/// The WHERE clause for one table: due, not account-lifetime, and every guard.
fn where_clause(table: &Table, now: i64) -> String {
    let mut clause = match table.expiry {
        Expiry::ExpiresAt => {
            format!("t.expires_at < {now} AND t.retention_class <> '{ACCOUNT_LIFETIME}'")
        }
        Expiry::AgeDays { expr, days } => {
            format!("({expr}) < julianday({now}, 'unixepoch', '-{days} days')")
        }
    };
    if let Some(extra) = table.extra {
        clause.push_str(&format!(" AND ({extra})"));
    }
    if let Some(col) = table.user_col {
        clause.push_str(&format!(
            " AND NOT EXISTS (SELECT 1 FROM user_bans b \
             WHERE b.user_id = t.{col} AND b.revoked_at IS NULL)"
        ));
        clause.push_str(&hold_clause(
            "user",
            &format!("s.subject_id = CAST(t.{col} AS TEXT)"),
            now,
        ));
    }
    if let Some(hold) = &table.hold {
        clause.push_str(&hold_clause(hold.kind, hold.match_sql, now));
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

/// Raise the domain floors over the rows about to be deleted. Runs in its own
/// transaction and commits before any delete. Floors only rise.
async fn advance_floors(
    db: &DatabaseConnection,
    table: &Table,
    domain: &str,
    now: i64,
) -> Result<(), DbErr> {
    let name = table.name;
    let clause = where_clause(table, now);
    let exists = format!(
        "EXISTS (SELECT 1 FROM {name} AS t WHERE t.user_id = sync_counters.user_id AND {clause})"
    );
    let txn = db.begin().await?;
    txn.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!(
            "UPDATE sync_counters SET floor_rev = MAX(floor_rev, \
             (SELECT MAX(t.rev) FROM {name} AS t WHERE t.user_id = sync_counters.user_id AND {clause})) \
             WHERE domain = '{domain}' AND {exists}"
        ),
    ))
    .await?;
    if domain == BOOKMARKS_DOMAIN {
        let cutoff_ms = (now - TOMBSTONE_DAYS * 86_400) * 1_000;
        txn.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "UPDATE sync_counters SET floor_ms = MAX(floor_ms, {cutoff_ms}) \
                 WHERE domain = '{domain}' AND {exists}"
            ),
        ))
        .await?;
    }
    txn.commit().await
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
    if delete && eligible > 0 {
        if let Some(domain) = table.tombstone_domain {
            advance_floors(db, table, domain, now).await?;
        }
        let sql = format!(
            "DELETE FROM {name} WHERE rowid IN \
             (SELECT t.rowid FROM {name} AS t WHERE {clause} LIMIT {BATCH_SIZE})"
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
                let alert = format!("table_failed table={}", table.name);
                error!(target: "retention_alert", %alert, "retention alert");
                report.alerts.push(alert);
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
        let report = run_recorded(&state.sea_db, mode, job_log::epoch_now()).await;
        if !report.alerts.is_empty() {
            operator_alert::send_retention_alert(&state, &report.alerts).await;
        }
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
    fn every_swept_table_has_a_migration() {
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

    #[test]
    fn every_delete_clause_keeps_the_ban_and_hold_guards() {
        for table in TABLES {
            let clause = where_clause(table, 1_700_000_000);
            if table.user_col.is_some() {
                assert!(clause.contains("b.revoked_at IS NULL"), "{}", table.name);
            }
            if table.user_col.is_some() || table.hold.is_some() {
                assert!(clause.contains("legal_hold_subjects"), "{}", table.name);
            }
        }
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

    #[tokio::test]
    async fn aged_revoked_ban_deleted_and_live_ban_kept() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        exec(
            &db,
            "INSERT INTO user_bans (user_id, reason, created_at, revoked_at) \
             VALUES (21, 'old', '2019-01-01T00:00:00Z', '2019-02-01T00:00:00Z')",
        )
        .await;
        exec(
            &db,
            "INSERT INTO user_bans (user_id, reason, created_at) \
             VALUES (22, 'live', '2019-01-01T00:00:00Z')",
        )
        .await;
        run_recorded(&db, Mode::DryRun, now).await;

        run_recorded(&db, Mode::Enforce, now).await;

        let left = count(
            &db,
            "SELECT COUNT(*) FROM user_bans WHERE user_id IN (21, 22)",
        )
        .await
        .unwrap();
        assert_eq!(left, 1);
    }

    #[tokio::test]
    async fn tombstone_floor_rises_before_aged_tombstone_is_deleted() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        exec(
            &db,
            "INSERT INTO sync_counters (user_id, domain, rev) VALUES (31, 'bookmarks', 12)",
        )
        .await;
        exec(
            &db,
            "INSERT INTO bookmark_folders (user_id, id, name, created_at, updated_at, deleted_at, rev) \
             VALUES (31, 'gone', 'x', '2019-01-01T00:00:00Z', '2019-01-01T00:00:00Z', \
             '2019-01-02T00:00:00Z', 7)",
        )
        .await;
        run_recorded(&db, Mode::DryRun, now).await;

        run_recorded(&db, Mode::Enforce, now).await;

        let folders = count(
            &db,
            "SELECT COUNT(*) FROM bookmark_folders WHERE user_id = 31",
        )
        .await
        .unwrap();
        assert_eq!(folders, 0);
        let floor = count(
            &db,
            "SELECT floor_rev FROM sync_counters WHERE user_id = 31 AND domain = 'bookmarks'",
        )
        .await
        .unwrap();
        assert_eq!(floor, 7);
    }

    #[tokio::test]
    async fn notification_under_user_hold_survives() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        exec(
            &db,
            "INSERT INTO notifications (user_id, kind, title, body, created_at) \
             VALUES (41, 'k', 't', 'b', '2019-01-01 00:00:00')",
        )
        .await;
        actions::place(
            &db,
            NewHold {
                rule: HoldRule::Enforcement12m,
                authority: "test".into(),
                reason: "test".into(),
                owner: "test".into(),
                case_ref: None,
                subjects: vec![HoldSubject {
                    kind: "user".into(),
                    id: "41".into(),
                    user_id: Some(41),
                }],
            },
            Utc::now(),
        )
        .await
        .unwrap();
        run_recorded(&db, Mode::DryRun, now).await;

        run_recorded(&db, Mode::Enforce, now).await;

        let kept = count(&db, "SELECT COUNT(*) FROM notifications WHERE user_id = 41")
            .await
            .unwrap();
        assert_eq!(kept, 1);
    }
}
