//! Operator alerts (A26). A P0 report sends one mail and one push to the operator.
//! The daily digest (08:00 UTC) mails counts of open reports and appeals. Every P0
//! dispatch writes one `alert.dispatch` audit row with the per-channel result.
//!
//! "accepted" means the next hop took the message: a provider accepted the mail, or
//! FCM returned a message name. Neither proves the operator read it. Push and digest
//! carry no report content, reporter, or device identity.

use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, NaiveTime, Utc};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tracing::{error, info, warn};

use crate::db::sea_models::{appeal, device, report};
use crate::error::DbResult;
use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::services::job_log::{self, OUTCOME_ERROR, OUTCOME_OK};
use crate::services::mail::provider::MailProvider;
use crate::services::mail::{mail_error_kind, MailError, MailRouter, OutboundEmail, SendReceipt};
use crate::services::notification::private_push_hint;
use crate::state::AppState;

pub const ENV_ALERT_EMAIL: &str = "OPERATOR_ALERT_EMAIL";
pub const ENV_USER_ID: &str = "OPERATOR_USER_ID";
pub const DIGEST_JOB: &str = "operator_digest";

const DIGEST_TIME: NaiveTime = match NaiveTime::from_hms_opt(8, 0, 0) {
    Some(t) => t,
    None => panic!("digest time is a constant"),
};

/// Where the operator is reached. Both unset means no alert channel: dispatch still
/// runs and records `not_configured` for each channel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperatorConfig {
    pub alert_email: Option<String>,
    /// Account whose registered devices receive the push.
    pub user_id: Option<i32>,
}

impl OperatorConfig {
    pub fn from_env() -> Self {
        let alert_email = std::env::var(ENV_ALERT_EMAIL)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let user_id = std::env::var(ENV_USER_ID)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok());
        Self {
            alert_email,
            user_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailStatus {
    Accepted,
    NotConfigured,
    Suppressed,
    NotAccepted,
    Failed(&'static str),
}

impl MailStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::NotConfigured => "not_configured",
            Self::Suppressed => "suppressed",
            Self::NotAccepted => "not_accepted",
            Self::Failed(_) => "failed",
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Failed(kind) => json!({ "status": self.label(), "failure": kind }),
            _ => json!({ "status": self.label() }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushStatus {
    NotConfigured,
    NoDevice,
    LoadFailed,
    /// FCM answered per device. `accepted` counts FCM message names returned.
    Sent {
        accepted: u32,
        failed: u32,
    },
}

impl PushStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::NoDevice => "no_device",
            Self::LoadFailed => "load_failed",
            Self::Sent { accepted, .. } if *accepted > 0 => "accepted_by_fcm",
            Self::Sent { .. } => "not_accepted",
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Sent { accepted, failed } => json!({
                "status": self.label(),
                "accepted": accepted,
                "failed": failed,
            }),
            _ => json!({ "status": self.label() }),
        }
    }
}

/// Maps a mail send result to a closed status. Only a provider receipt with
/// `delivered` or `queued` counts as accepted.
pub fn mail_status(result: &Result<SendReceipt, MailError>) -> MailStatus {
    match result {
        Ok(receipt) if receipt.accepted_by_provider() => MailStatus::Accepted,
        Ok(receipt) if receipt.suppressed => MailStatus::Suppressed,
        Ok(_) => MailStatus::NotAccepted,
        Err(MailError::Suppressed) => MailStatus::Suppressed,
        Err(err) => MailStatus::Failed(mail_error_kind(err)),
    }
}

async fn send_mail(
    mailer: &MailRouter,
    cfg: &OperatorConfig,
    subject: &str,
    text: &str,
) -> MailStatus {
    let Some(to) = cfg.alert_email.clone() else {
        return MailStatus::NotConfigured;
    };
    // Non-transactional on purpose: a suppressed operator address must stop the alert.
    let result = mailer
        .send(OutboundEmail {
            to,
            subject: subject.to_string(),
            html: Some(format!("<p>{text}</p>")),
            text: Some(text.to_string()),
            template: None,
            list_unsubscribe: None,
        })
        .await;
    mail_status(&result)
}

async fn push_operator(
    db: &DatabaseConnection,
    fcm: Option<&rux_fcm::FcmClient>,
    cfg: &OperatorConfig,
) -> PushStatus {
    let (Some(client), Some(user_id)) = (fcm, cfg.user_id) else {
        return PushStatus::NotConfigured;
    };
    let devices = match device::Entity::list_for_user(db, user_id).await {
        Ok(d) => d,
        Err(err) => {
            warn!(error = %err, "Operator device lookup failed; push not sent");
            return PushStatus::LoadFailed;
        }
    };
    if devices.is_empty() {
        return PushStatus::NoDevice;
    }
    let mut accepted = 0u32;
    let mut failed = 0u32;
    for dev in devices {
        match client.send(private_push_hint(&dev.token)).await {
            Ok(_) => accepted += 1,
            Err(rux_fcm::FcmError::Unregistered) => {
                failed += 1;
                if let Err(err) = device::Entity::prune_owned_by_id(db, dev.id, user_id).await {
                    warn!(error = %err, device_id = dev.id, "Failed to prune unregistered operator device");
                }
            }
            Err(err) => {
                failed += 1;
                warn!(error = %err, device_id = dev.id, "Operator push failed");
            }
        }
    }
    PushStatus::Sent { accepted, failed }
}

/// Sends the P0 alert on both channels and writes one `alert.dispatch` audit row.
/// Returns the channel statuses. A failed audit write is logged at error level.
pub async fn dispatch_p0(
    db: &DatabaseConnection,
    mailer: &MailRouter,
    fcm: Option<&rux_fcm::FcmClient>,
    cfg: &OperatorConfig,
    report_id: i64,
) -> (MailStatus, PushStatus) {
    let text = format!(
        "P0 report {report_id} is waiting. Open the operator tools. No report content is included."
    );
    let mail = send_mail(mailer, cfg, "EasyQuran: P0 report waiting", &text).await;
    let push = push_operator(db, fcm, cfg).await;

    let row = audit::record(
        db,
        AuditEvent {
            actor_id: None,
            action: AuditAction::AlertDispatch,
            resource_id: report_id.to_string(),
            metadata: json!({
                "priority": "p0",
                "mail": mail.to_json(),
                "push": push.to_json(),
            }),
        },
    )
    .await;
    if let Err(err) = row {
        error!(error = %err, report_id, "P0 alert dispatch result not audited");
    }
    info!(
        report_id,
        mail = mail.label(),
        push = push.label(),
        "P0 alert dispatched"
    );
    (mail, push)
}

/// Fire-and-forget from the intake handler. The report row is already committed.
pub fn spawn_p0(state: &AppState, report_id: i64) {
    let state = state.clone();
    let cfg = OperatorConfig::from_env();
    tokio::spawn(async move {
        dispatch_p0(
            &state.sea_db,
            &state.mailer,
            state.fcm.as_deref(),
            &cfg,
            report_id,
        )
        .await;
    });
}

/// Ship-gate alert from the retention sweeper (A32). The text carries alert codes
/// and table names only, never row content.
pub async fn send_retention_alert(state: &AppState, alerts: &[String]) {
    let cfg = OperatorConfig::from_env();
    let text = format!(
        "Retention sweeper alert: {}. No row content is included.",
        alerts.join("; ")
    );
    let mail = send_mail(
        &state.mailer,
        &cfg,
        "EasyQuran: retention sweeper alert",
        &text,
    )
    .await;
    info!(
        mail = mail.label(),
        count = alerts.len(),
        "retention alert dispatched"
    );
}

/// Data-request clock (A38): open requests past the one-month deadline. Counts only.
pub async fn send_overdue_alert(state: &AppState, overdue: i64, oldest_days: i64) -> MailStatus {
    let cfg = OperatorConfig::from_env();
    let text = format!(
        "Data requests past the one-month deadline: {overdue}. Oldest is {oldest_days} days past it. \
         Answer through the erasure runbook in deploy/legal-debt.md. No subject ids are included."
    );
    let mail = send_mail(
        &state.mailer,
        &cfg,
        "EasyQuran: data request overdue",
        &text,
    )
    .await;
    info!(
        mail = mail.label(),
        overdue, "data request overdue alert dispatched"
    );
    mail
}

/// Operator text for a known-hash match (A37). Names no image, uploader or hash.
pub const HASH_MATCH_TEXT: &str = "A known-hash match blocked an upload. Open the operator tools. \
     No image, uploader or hash is included.";

/// Known-hash match alert (A37). Fires for hash matches only: a moderator refusal does not
/// page the operator. Fire-and-forget from the upload handler; the refusal is already decided.
pub fn spawn_hash_match(state: &AppState) {
    let state = state.clone();
    tokio::spawn(async move {
        dispatch_hash_match(&state).await;
    });
}

/// Mails the hash-match alert and writes one `alert.dispatch` audit row with the mail status.
pub async fn dispatch_hash_match(state: &AppState) -> MailStatus {
    let cfg = OperatorConfig::from_env();
    let mail = send_mail(
        &state.mailer,
        &cfg,
        "EasyQuran: known-hash match blocked",
        HASH_MATCH_TEXT,
    )
    .await;
    let row = audit::record(
        &state.sea_db,
        AuditEvent {
            actor_id: None,
            action: AuditAction::AlertDispatch,
            resource_id: "media.hash_match".to_string(),
            metadata: json!({ "priority": "hash_match", "mail": mail.to_json() }),
        },
    )
    .await;
    if let Err(err) = row {
        error!(error = %err, "hash-match alert result not audited");
    }
    info!(mail = mail.label(), "hash-match alert dispatched");
    mail
}

/// Counts only. No ids, no content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digest {
    pub open_p0: u64,
    pub open_p1: u64,
    pub open_appeals: u64,
}

pub async fn digest_counts(db: &DatabaseConnection) -> DbResult<Digest> {
    Ok(Digest {
        open_p0: report::Entity::count_open(db, "p0").await?,
        open_p1: report::Entity::count_open(db, "p1").await?,
        open_appeals: appeal::Entity::open_count(db).await?,
    })
}

pub fn digest_text(d: &Digest) -> String {
    format!(
        "Daily digest. Open P0 reports: {}. Open P1 reports: {}. Open appeals: {}.",
        d.open_p0, d.open_p1, d.open_appeals
    )
}

/// Next 08:00 UTC strictly after `now`.
pub fn next_digest_at(now: DateTime<Utc>) -> DateTime<Utc> {
    let today = now.date_naive().and_time(DIGEST_TIME).and_utc();
    if today > now {
        today
    } else {
        today + ChronoDuration::days(1)
    }
}

/// Counts the open rows, mails the digest, and writes one `job_runs` row with the
/// counts and the mail status.
pub async fn send_digest(state: &AppState) {
    let started = job_log::epoch_now();
    let cfg = OperatorConfig::from_env();
    let (outcome, detail) = match digest_counts(&state.sea_db).await {
        Ok(d) => {
            let mail = send_mail(
                &state.mailer,
                &cfg,
                "EasyQuran daily digest",
                &digest_text(&d),
            )
            .await;
            (
                OUTCOME_OK,
                Some(format!(
                    "open_p0={} open_p1={} open_appeals={} mail={}",
                    d.open_p0,
                    d.open_p1,
                    d.open_appeals,
                    mail.label()
                )),
            )
        }
        Err(err) => {
            error!(error = ?err, "Operator digest count failed");
            (OUTCOME_ERROR, None)
        }
    };
    let finished = job_log::epoch_now();
    job_log::record(
        &state.sea_db,
        DIGEST_JOB,
        started,
        finished,
        outcome,
        detail.as_deref(),
    )
    .await;
}

/// Long-running task: sleeps until each 08:00 UTC, then sends the digest.
pub async fn run_digest(state: AppState) {
    loop {
        let now = Utc::now();
        let wait = (next_digest_at(now) - now)
            .to_std()
            .unwrap_or(Duration::from_secs(60));
        tokio::time::sleep(wait).await;
        send_digest(&state).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn next_digest_before_and_after_0800() {
        let before = Utc.with_ymd_and_hms(2026, 11, 6, 7, 59, 59).unwrap();
        assert_eq!(
            next_digest_at(before),
            Utc.with_ymd_and_hms(2026, 11, 6, 8, 0, 0).unwrap()
        );
        let at = Utc.with_ymd_and_hms(2026, 11, 6, 8, 0, 0).unwrap();
        assert_eq!(
            next_digest_at(at),
            Utc.with_ymd_and_hms(2026, 11, 7, 8, 0, 0).unwrap()
        );
    }

    #[test]
    fn hash_match_text_names_no_upload_or_uploader() {
        assert!(HASH_MATCH_TEXT.contains("No image, uploader or hash is included."));
        assert!(!HASH_MATCH_TEXT.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn mail_status_maps_receipts() {
        assert_eq!(
            mail_status(&Ok(SendReceipt {
                delivered: 1,
                ..SendReceipt::default()
            })),
            MailStatus::Accepted
        );
        assert_eq!(
            mail_status(&Ok(SendReceipt::default())),
            MailStatus::NotAccepted
        );
        assert_eq!(
            mail_status(&Ok(SendReceipt {
                suppressed: true,
                ..SendReceipt::default()
            })),
            MailStatus::Suppressed
        );
        assert_eq!(
            mail_status(&Err(MailError::Config("x".into()))),
            MailStatus::Failed("config")
        );
    }
}
