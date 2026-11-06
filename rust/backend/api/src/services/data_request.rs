//! Data-request procedure (A38). A subject's erasure request is opened with a one-month
//! response clock (`due_at` = `received_at` + 30 days). It is answered by `erase`, which
//! runs the A33 erase in the same transaction, writes the receipt, scrubs the `users` row
//! when no legal hold applies, and closes the request. A background clock alerts the
//! operator when a request is past its deadline.
//!
//! The claim is logical only: rows leave the live database. Backups keep them until the
//! backup cap (A33). What a live legal hold keeps stays, and the receipt lists it.
//!
//! Self-service deletion (F7) calls `open` with origin `self`, then `erase` with the
//! subject's own id as actor. Nothing here assumes an admin caller.
//!
//! Counts and ids only reach logs, `job_runs` and alerts. Mail text carries no subject ids.

use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, TransactionTrait,
};
use serde::Serialize;
use serde_json::json;
use tracing::{info, warn};

use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::services::job_log::{self, OUTCOME_ERROR, OUTCOME_OK};
use crate::services::{operator_alert, retention};
use crate::state::AppState;

pub const ORIGIN_ADMIN: &str = "admin";
pub const ORIGIN_SELF: &str = "self";
const SUBJECT_USER: &str = "user";
/// One-month response clock (GDPR Art 12(3)).
pub const RESPONSE_SECS: i64 = 30 * 86_400;
pub const CLOCK_JOB: &str = "data_request_clock";
pub const CLOCK_INTERVAL_SECS: u64 = 6 * 3_600;
const ERASED_NAME: &str = "Erased user";
const QUEUE_LIMIT: u64 = 200;

#[derive(Debug)]
pub enum RequestError {
    Db(DbErr),
    AlreadyOpen,
    NotFound,
    Closed,
}

impl From<DbErr> for RequestError {
    fn from(err: DbErr) -> Self {
        Self::Db(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestRow {
    pub id: i64,
    pub subject_id: String,
    pub due_at: i64,
    pub closed_at: Option<i64>,
}

impl RequestRow {
    /// The subject's user id. None when the stored subject id is not an integer.
    pub fn user_id(&self) -> Option<i32> {
        self.subject_id.parse().ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueRow {
    pub id: i64,
    pub subject_id: String,
    pub received_at: i64,
    pub due_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EraseOutcome {
    pub receipt_id: i64,
    pub deleted: i64,
    pub retained_tables: usize,
    pub user_row_erased: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overdue {
    pub count: i64,
    pub oldest_due_at: Option<i64>,
}

async fn scalar_i64<C: ConnectionTrait>(
    conn: &C,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<Option<i64>, DbErr> {
    let Some(row) = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql.to_string(),
            values,
        ))
        .await?
    else {
        return Ok(None);
    };
    row.try_get_by_index::<Option<i64>>(0)
}

async fn load<C: ConnectionTrait>(conn: &C, id: i64) -> Result<Option<RequestRow>, DbErr> {
    let Some(row) = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT id, subject_id, due_at, closed_at FROM data_requests WHERE id = ?",
            [id.into()],
        ))
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(RequestRow {
        id: row.try_get_by_index::<i64>(0)?,
        subject_id: row.try_get_by_index::<String>(1)?,
        due_at: row.try_get_by_index::<i64>(2)?,
        closed_at: row.try_get_by_index::<Option<i64>>(3)?,
    }))
}

/// One request by id, open or closed.
pub async fn get(db: &DatabaseConnection, id: i64) -> Result<Option<RequestRow>, DbErr> {
    load(db, id).await
}

/// Open a request for one user and start its clock. One open request per subject.
pub async fn open(
    db: &DatabaseConnection,
    actor: Option<i32>,
    origin: &str,
    user_id: i32,
    now: i64,
) -> Result<i64, RequestError> {
    let txn = db.begin().await?;
    let id = open_in(&txn, actor, origin, user_id, now).await?;
    txn.commit().await?;
    Ok(id)
}

/// Self-service erasure (F7): open and answer in one transaction, through the same
/// `open_in` and `erase_in` the operator path uses. An open request already on file for
/// the subject is answered as is, so its clock is never restarted.
pub async fn erase_self(
    db: &DatabaseConnection,
    user_id: i32,
    now: i64,
) -> Result<EraseOutcome, RequestError> {
    let txn = db.begin().await?;
    let existing = scalar_i64(
        &txn,
        "SELECT id FROM data_requests WHERE subject_kind = ? AND subject_id = ? \
         AND closed_at IS NULL ORDER BY id LIMIT 1",
        vec![SUBJECT_USER.into(), user_id.to_string().into()],
    )
    .await?;
    let id = match existing {
        Some(id) => id,
        None => open_in(&txn, Some(user_id), ORIGIN_SELF, user_id, now).await?,
    };
    let out = erase_in(&txn, Some(user_id), id, now).await?;
    txn.commit().await?;
    Ok(out)
}

async fn open_in<C: ConnectionTrait>(
    txn: &C,
    actor: Option<i32>,
    origin: &str,
    user_id: i32,
    now: i64,
) -> Result<i64, RequestError> {
    let subject = user_id.to_string();
    let existing = scalar_i64(
        txn,
        "SELECT COUNT(*) FROM data_requests WHERE subject_kind = ? AND subject_id = ? AND closed_at IS NULL",
        vec![SUBJECT_USER.into(), subject.clone().into()],
    )
    .await?
    .unwrap_or(0);
    if existing > 0 {
        return Err(RequestError::AlreadyOpen);
    }
    let due_at = now + RESPONSE_SECS;
    let res = txn
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO data_requests \
             (subject_kind, subject_id, origin, received_at, due_at, closed_at, receipt_id, \
              opened_by, expires_at, retention_class) \
             VALUES (?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?)",
            [
                SUBJECT_USER.into(),
                subject.into(),
                origin.into(),
                now.into(),
                due_at.into(),
                actor.into(),
                (now + retention::ERASURE_RETENTION_DAYS * 86_400).into(),
                retention::ERASURE_RETENTION_CLASS.into(),
            ],
        ))
        .await?;
    let id = res.last_insert_id() as i64;
    audit::record(
        txn,
        AuditEvent {
            actor_id: actor,
            action: AuditAction::DataRequestOpen,
            resource_id: id.to_string(),
            metadata: json!({ "origin": origin, "due_at": due_at }),
        },
    )
    .await?;
    Ok(id)
}

/// Overwrite the `users` row with tombstone values: no name, no mailbox, no password,
/// no 2FA or OAuth link, no mailbox hash. The id stays, because evidence rows key on it.
/// A fresh session secret invalidates any session that survived the sessions delete.
async fn scrub_user_row<C: ConnectionTrait>(conn: &C, user_id: i32) -> Result<(), DbErr> {
    let email = format!("erased-{user_id}@erased.invalid");
    let secret = uuid::Uuid::new_v4().to_string();
    conn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE users SET name = ?, email = ?, password = NULL, avatar_id = NULL, \
         is_verified = 0, two_fa_enabled = 0, two_fa_secret = NULL, two_fa_backup_codes = NULL, \
         two_fa_last_totp_counter = NULL, google_id = NULL, oauth_provider = NULL, \
         session_auth_secret = ?, verification_identity_hash = NULL WHERE id = ?",
        [
            ERASED_NAME.into(),
            email.into(),
            secret.into(),
            user_id.into(),
        ],
    ))
    .await?;
    Ok(())
}

/// Answer an open request by erasing its subject. One transaction: A33 erase (rows,
/// receipt, audit), `users` scrub unless a live user hold applies, request closed with
/// its receipt, one `data_request.close` audit row. `actor` is the admin, or the subject
/// on the self-service path.
pub async fn erase(
    db: &DatabaseConnection,
    actor: Option<i32>,
    request_id: i64,
    now: i64,
) -> Result<EraseOutcome, RequestError> {
    let txn = db.begin().await?;
    let out = erase_in(&txn, actor, request_id, now).await?;
    txn.commit().await?;
    Ok(out)
}

async fn erase_in<C: ConnectionTrait>(
    txn: &C,
    actor: Option<i32>,
    request_id: i64,
    now: i64,
) -> Result<EraseOutcome, RequestError> {
    let Some(req) = load(txn, request_id).await? else {
        return Err(RequestError::NotFound);
    };
    if req.closed_at.is_some() {
        return Err(RequestError::Closed);
    }
    let Some(user_id) = req.user_id() else {
        return Err(RequestError::NotFound);
    };
    let report = retention::erase_subject_in(txn, actor, user_id, now).await?;
    let held = retention::user_hold_expiry(txn, &user_id.to_string(), now).await? != 0;
    if !held {
        scrub_user_row(txn, user_id).await?;
    }
    txn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE data_requests SET closed_at = ?, receipt_id = ? WHERE id = ? AND closed_at IS NULL",
        [now.into(), report.receipt_id.into(), request_id.into()],
    ))
    .await?;
    audit::record(
        txn,
        AuditEvent {
            actor_id: actor,
            action: AuditAction::DataRequestClose,
            resource_id: request_id.to_string(),
            metadata: json!({
                "receipt_id": report.receipt_id,
                "deleted": report.deleted,
                "retained_tables": report.retained_tables,
                "user_row_erased": !held,
            }),
        },
    )
    .await?;
    info!(
        request_id,
        receipt_id = report.receipt_id,
        "data request answered"
    );
    Ok(EraseOutcome {
        receipt_id: report.receipt_id,
        deleted: report.deleted,
        retained_tables: report.retained_tables,
        user_row_erased: !held,
    })
}

/// Open requests, earliest deadline first. Ids and times only.
pub async fn open_queue(db: &DatabaseConnection) -> Result<Vec<QueueRow>, DbErr> {
    use sea_orm::QueryResult;
    let rows: Vec<QueryResult> = db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT id, subject_id, received_at, due_at FROM data_requests \
             WHERE closed_at IS NULL ORDER BY due_at, id LIMIT ?",
            [(QUEUE_LIMIT as i64).into()],
        ))
        .await?;
    rows.iter()
        .map(|row| {
            Ok(QueueRow {
                id: row.try_get_by_index::<i64>(0)?,
                subject_id: row.try_get_by_index::<String>(1)?,
                received_at: row.try_get_by_index::<i64>(2)?,
                due_at: row.try_get_by_index::<i64>(3)?,
            })
        })
        .collect()
}

/// Open requests past their deadline: count and earliest deadline.
pub async fn overdue(db: &DatabaseConnection, now: i64) -> Result<Overdue, DbErr> {
    let count = scalar_i64(
        db,
        "SELECT COUNT(*) FROM data_requests WHERE closed_at IS NULL AND due_at < ?",
        vec![now.into()],
    )
    .await?
    .unwrap_or(0);
    let oldest_due_at = scalar_i64(
        db,
        "SELECT MIN(due_at) FROM data_requests WHERE closed_at IS NULL AND due_at < ?",
        vec![now.into()],
    )
    .await?;
    Ok(Overdue {
        count,
        oldest_due_at,
    })
}

/// Long-running task: every `CLOCK_INTERVAL_SECS`, counts overdue requests. Any overdue
/// request mails the operator (`operator_alert`) and writes one `job_runs` row.
pub async fn run_clock(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(CLOCK_INTERVAL_SECS));
    loop {
        interval.tick().await;
        let started = job_log::epoch_now();
        let now = started;
        let (outcome, detail) = match overdue(&state.sea_db, now).await {
            Ok(o) if o.count == 0 => (OUTCOME_OK, "overdue=0".to_string()),
            Ok(o) => {
                let oldest_days = o.oldest_due_at.map_or(0, |due| (now - due) / 86_400);
                let mail = operator_alert::send_overdue_alert(&state, o.count, oldest_days).await;
                warn!(
                    overdue = o.count,
                    "data requests past their response deadline"
                );
                (
                    OUTCOME_ERROR,
                    format!(
                        "overdue={} oldest_days={oldest_days} mail={}",
                        o.count,
                        mail.label()
                    ),
                )
            }
            Err(err) => {
                warn!(error = %err, "data request clock failed (non-fatal)");
                (OUTCOME_ERROR, "error".to_string())
            }
        };
        job_log::record(
            &state.sea_db,
            CLOCK_JOB,
            started,
            job_log::epoch_now(),
            outcome,
            Some(&detail),
        )
        .await;
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

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        scalar_i64(db, sql, vec![]).await.unwrap().unwrap_or(0)
    }

    async fn insert_user(db: &DatabaseConnection, id: i32) {
        let t = Utc::now().to_rfc3339();
        exec(
            db,
            &format!(
                "INSERT INTO users (id, name, email, password, is_verified, role, two_fa_enabled, \
                 session_auth_secret, created_at, updated_at) VALUES \
                 ({id}, 'Real Name', 'u{id}@example.com', 'hash', 1, 'user', 0, 's', '{t}', '{t}')"
            ),
        )
        .await;
    }

    async fn insert_notification(db: &DatabaseConnection, user: i32) {
        exec(
            db,
            &format!(
                "INSERT INTO notifications (user_id, kind, title, body, created_at) \
                 VALUES ({user}, 'k', 't', 'b', '2019-01-01 00:00:00')"
            ),
        )
        .await;
    }

    #[tokio::test]
    async fn second_open_request_for_same_subject_is_refused() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        open(&db, Some(1), ORIGIN_ADMIN, 70, now).await.unwrap();
        let again = open(&db, Some(1), ORIGIN_ADMIN, 70, now).await;
        assert!(matches!(again, Err(RequestError::AlreadyOpen)));
    }

    #[tokio::test]
    async fn erase_deletes_rest_scrubs_user_row_closes_request_and_writes_receipt() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_user(&db, 71).await;
        insert_notification(&db, 71).await;
        let id = open(&db, Some(1), ORIGIN_ADMIN, 71, now).await.unwrap();

        let out = erase(&db, Some(1), id, now).await.unwrap();

        assert!(out.user_row_erased);
        assert!(out.deleted >= 1);
        let left = count(&db, "SELECT COUNT(*) FROM notifications WHERE user_id = 71").await;
        assert_eq!(left, 0);
        let scrubbed = count(
            &db,
            "SELECT COUNT(*) FROM users WHERE id = 71 AND name = 'Erased user' \
             AND email = 'erased-71@erased.invalid' AND password IS NULL",
        )
        .await;
        assert_eq!(scrubbed, 1);
        let closed = count(
            &db,
            &format!("SELECT COUNT(*) FROM data_requests WHERE id = {id} AND closed_at = {now} AND receipt_id = {}", out.receipt_id),
        )
        .await;
        assert_eq!(closed, 1);
        let receipts = count(
            &db,
            "SELECT COUNT(*) FROM erasure_receipts WHERE subject_id = '71' AND retention_class = 'erasure_24m'",
        )
        .await;
        assert_eq!(receipts, 1);
        let again = erase(&db, Some(1), id, now).await;
        assert!(matches!(again, Err(RequestError::Closed)));
    }

    #[tokio::test]
    async fn self_erase_opens_answers_and_clears_rows_sessions_and_device_tokens() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_user(&db, 77).await;
        insert_notification(&db, 77).await;
        exec(
            &db,
            "INSERT INTO user_sessions (user_id, last_seen) VALUES (77, '2019-01-01 00:00:00')",
        )
        .await;
        exec(
            &db,
            "INSERT INTO user_devices (user_id, id, platform, client, form_factor, last_seen_at) \
             VALUES (77, 'dev-77', 'web', 'web', 'desktop', '2019-01-01 00:00:00')",
        )
        .await;

        let out = erase_self(&db, 77, now).await.unwrap();

        assert!(out.user_row_erased);
        let open = count(
            &db,
            "SELECT COUNT(*) FROM data_requests WHERE subject_id = '77' AND closed_at IS NULL",
        )
        .await;
        assert_eq!(open, 0, "request opened and answered in one step");
        let origin = count(
            &db,
            "SELECT COUNT(*) FROM data_requests WHERE subject_id = '77' AND origin = 'self'",
        )
        .await;
        assert_eq!(origin, 1);
        let left = count(&db, "SELECT COUNT(*) FROM notifications WHERE user_id = 77").await;
        assert_eq!(left, 0);
        let sessions = count(&db, "SELECT COUNT(*) FROM user_sessions WHERE user_id = 77").await;
        assert_eq!(sessions, 0, "sessions gone");
        let devices = count(&db, "SELECT COUNT(*) FROM user_devices WHERE user_id = 77").await;
        assert_eq!(devices, 0, "device rows and their push tokens gone");
        let scrubbed = count(
            &db,
            "SELECT COUNT(*) FROM users WHERE id = 77 AND name = 'Erased user' AND password IS NULL",
        )
        .await;
        assert_eq!(scrubbed, 1);
    }

    #[tokio::test]
    async fn self_erase_answers_an_open_operator_request_without_restarting_clock() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_user(&db, 78).await;
        let admin_id = open(&db, Some(1), ORIGIN_ADMIN, 78, now - 100)
            .await
            .unwrap();

        let out = erase_self(&db, 78, now).await.unwrap();

        assert!(out.user_row_erased);
        let closed = count(
            &db,
            &format!("SELECT COUNT(*) FROM data_requests WHERE id = {admin_id} AND origin = 'admin' AND closed_at IS NOT NULL"),
        )
        .await;
        assert_eq!(closed, 1);
        let total = count(
            &db,
            "SELECT COUNT(*) FROM data_requests WHERE subject_id = '78'",
        )
        .await;
        assert_eq!(total, 1, "no second request row");
    }

    #[tokio::test]
    async fn legal_hold_keeps_subject_rows_and_user_row_and_evidence() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        insert_user(&db, 72).await;
        insert_notification(&db, 72).await;
        // Evidence row keyed to the subject. Audit rows are never erased.
        audit::record(
            &db,
            AuditEvent {
                actor_id: Some(72),
                action: AuditAction::UserDelete,
                resource_id: "72".into(),
                metadata: json!({}),
            },
        )
        .await
        .unwrap();
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
                    id: "72".into(),
                    user_id: Some(72),
                }],
            },
            Utc::now(),
        )
        .await
        .unwrap();
        let id = open(&db, Some(1), ORIGIN_ADMIN, 72, now).await.unwrap();

        let out = erase(&db, Some(1), id, now).await.unwrap();

        assert!(!out.user_row_erased);
        let kept = count(&db, "SELECT COUNT(*) FROM notifications WHERE user_id = 72").await;
        assert_eq!(kept, 1, "held subject rows stay");
        let identity = count(
            &db,
            "SELECT COUNT(*) FROM users WHERE id = 72 AND name = 'Real Name' AND password = 'hash'",
        )
        .await;
        assert_eq!(identity, 1, "held user row stays");
        let audited = count(&db, "SELECT COUNT(*) FROM audit_logs WHERE user_id = 72").await;
        assert!(audited >= 1, "audit evidence stays");
        let listed = count(
            &db,
            "SELECT COUNT(*) FROM erasure_receipts WHERE subject_id = '72' AND retained LIKE '%\"table\":\"users\"%'",
        )
        .await;
        assert_eq!(listed, 1, "receipt lists the held user row");
    }

    #[tokio::test]
    async fn overdue_counts_only_open_requests_past_deadline() {
        let db = test_db().await;
        let now = job_log::epoch_now();
        // Received a month and ten seconds ago: deadline already passed.
        let late = now - RESPONSE_SECS - 10;
        open(&db, None, ORIGIN_SELF, 74, late).await.unwrap();
        open(&db, None, ORIGIN_SELF, 75, now).await.unwrap();

        let before = overdue(&db, now).await.unwrap();
        assert_eq!(before.count, 1);
        assert_eq!(before.oldest_due_at, Some(late + RESPONSE_SECS));

        let late_id = count(&db, "SELECT id FROM data_requests WHERE subject_id = '74'").await;
        erase(&db, None, late_id, now).await.unwrap();
        let after = overdue(&db, now).await.unwrap();
        assert_eq!(
            after,
            Overdue {
                count: 0,
                oldest_due_at: None
            }
        );
    }
}
