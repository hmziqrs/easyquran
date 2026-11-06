//! Audit writer (A23). Every destructive or enforcement action writes exactly one
//! `audit_logs` row through [`record`]. Call sites pass an [`AuditAction`], never a
//! raw action string, so the source-scan guard in `tests/audit_writer.rs` can
//! check that every action has a call site.
//!
//! Rows are evidence: no FK to users, 12-month retention class, `expires_at` set
//! at write time. Metadata must never carry device identity (device id, key or
//! continuity hint); call sites pass only what the action needs.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbErr, Set};
use serde_json::Value;

use crate::db::sea_models::audit_log;

pub const RETENTION_CLASS: &str = "audit_12m";
const RETENTION_SECS: i64 = 365 * 24 * 60 * 60;

/// Every audited action. Adding a variant needs a call site, or the guard fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAction {
    RouteBlock,
    RouteUnblock,
    RouteDelete,
    CommentHide,
    CommentUnhide,
    CommentDelete,
    CommentFlagsClear,
    BanCreate,
    BanRevoke,
    BanRenew,
    BanNoticeResend,
    UserDelete,
    CategoryDelete,
    MediaDelete,
    NewsletterSend,
    SuppressionAdd,
    SuppressionDelete,
    SuppressionReview,
    MailGuardPause,
    SeedRun,
    DeviceRemove,
    ReportSubmit,
    IncidentCapture,
    LegalHoldPlace,
    LegalHoldRenew,
    AlertDispatch,
}

impl AuditAction {
    pub const ALL: &[AuditAction] = &[
        Self::RouteBlock,
        Self::RouteUnblock,
        Self::RouteDelete,
        Self::CommentHide,
        Self::CommentUnhide,
        Self::CommentDelete,
        Self::CommentFlagsClear,
        Self::BanCreate,
        Self::BanRevoke,
        Self::BanRenew,
        Self::BanNoticeResend,
        Self::UserDelete,
        Self::CategoryDelete,
        Self::MediaDelete,
        Self::NewsletterSend,
        Self::SuppressionAdd,
        Self::SuppressionDelete,
        Self::SuppressionReview,
        Self::MailGuardPause,
        Self::SeedRun,
        Self::DeviceRemove,
        Self::ReportSubmit,
        Self::IncidentCapture,
        Self::LegalHoldPlace,
        Self::LegalHoldRenew,
        Self::AlertDispatch,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::RouteBlock => "route.block",
            Self::RouteUnblock => "route.unblock",
            Self::RouteDelete => "route.delete",
            Self::CommentHide => "comment.hide",
            Self::CommentUnhide => "comment.unhide",
            Self::CommentDelete => "comment.delete",
            Self::CommentFlagsClear => "comment.flags_clear",
            Self::BanCreate => "ban.create",
            Self::BanRevoke => "ban.revoke",
            Self::BanRenew => "ban.renew",
            Self::BanNoticeResend => "ban.notice_resend",
            Self::UserDelete => "user.delete",
            Self::CategoryDelete => "category.delete",
            Self::MediaDelete => "media.delete",
            Self::NewsletterSend => "newsletter.send",
            Self::SuppressionAdd => "suppression.add",
            Self::SuppressionDelete => "suppression.delete",
            Self::SuppressionReview => "suppression.review",
            Self::MailGuardPause => "mail.guard_pause",
            Self::SeedRun => "seed.run",
            Self::DeviceRemove => "device.remove",
            Self::ReportSubmit => "report.submit",
            Self::IncidentCapture => "incident.capture",
            Self::LegalHoldPlace => "legal_hold.place",
            Self::LegalHoldRenew => "legal_hold.renew",
            Self::AlertDispatch => "alert.dispatch",
        }
    }

    pub fn resource_type(self) -> &'static str {
        match self {
            Self::RouteBlock | Self::RouteUnblock | Self::RouteDelete => "route",
            Self::CommentHide
            | Self::CommentUnhide
            | Self::CommentDelete
            | Self::CommentFlagsClear => "comment",
            Self::BanCreate | Self::BanRevoke | Self::BanRenew | Self::BanNoticeResend => {
                "user_ban"
            }
            Self::UserDelete => "user",
            Self::CategoryDelete => "category",
            Self::MediaDelete => "media",
            Self::NewsletterSend => "newsletter",
            Self::SuppressionAdd | Self::SuppressionDelete | Self::SuppressionReview => {
                "mail_suppression"
            }
            Self::MailGuardPause => "mail_guard",
            Self::SeedRun => "seed_run",
            Self::DeviceRemove => "device",
            Self::ReportSubmit => "report",
            Self::IncidentCapture => "case",
            Self::LegalHoldPlace | Self::LegalHoldRenew => "legal_hold",
            Self::AlertDispatch => "report",
        }
    }
}

/// One audited action. `actor_id` is `None` for system or CLI/TUI actions.
pub struct AuditEvent {
    pub actor_id: Option<i32>,
    pub action: AuditAction,
    pub resource_id: String,
    pub metadata: Value,
}

/// Insert the audit row. Pass the transaction connection when the action runs in
/// one, so the action and its row commit or roll back together.
pub async fn record<C: ConnectionTrait>(conn: &C, event: AuditEvent) -> Result<(), DbErr> {
    let now = Utc::now();
    audit_log::model::ActiveModel {
        user_id: Set(event.actor_id),
        action: Set(event.action.as_str().to_string()),
        resource_type: Set(event.action.resource_type().to_string()),
        resource_id: Set(event.resource_id),
        metadata: Set(Some(event.metadata)),
        ip_address: Set(None),
        user_agent: Set(None),
        created_at: Set(now.fixed_offset()),
        expires_at: Set(now.timestamp() + RETENTION_SECS),
        retention_class: Set(RETENTION_CLASS.to_string()),
        ..Default::default()
    }
    .insert(conn)
    .await?;
    Ok(())
}
