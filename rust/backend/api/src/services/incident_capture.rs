//! Case evidence capture (A25). Copies a comment and the rows around it into
//! `case_evidence` before any removal, or on demand from an intake report
//! (`just incident capture <report-id>`). Each object is one append-only row.
//!
//! Snapshots are explicit field lists. Device identity never enters a snapshot:
//! no `device`, `device_id` or `user_agent` column is read here.

use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait,
};
use serde_json::{json, Value};

use crate::db::sea_models::{
    audit_log,
    case_evidence::{self, NewEvidence},
    comment_flag, media, post_comment, post_view, report, user, user_session,
};
use crate::error::{DbResult, ErrorCode, ErrorResponse};

use super::audit::{self, AuditAction, AuditEvent};

/// Most recent post views copied per capture.
pub const VIEWS_LIMIT: u64 = 500;

pub struct CaptureOutcome {
    pub case_ref: String,
    pub rows: usize,
}

/// Case ref for a comment removed while it has open flags.
pub fn removal_case_ref(comment_id: i32) -> String {
    format!("comment-{comment_id}")
}

/// Case ref for an intake report.
pub fn report_case_ref(report_id: i64) -> String {
    format!("report-{report_id}")
}

/// Copy one comment and its surroundings into `case_ref`. Caller owns the
/// transaction, so the evidence commits or rolls back with the action it precedes.
/// Objects: comment, comment_flags, user, sessions, views, media, audit.
pub async fn capture_comment<C: ConnectionTrait>(
    conn: &C,
    case_ref: &str,
    comment_id: i32,
    now: DateTime<Utc>,
) -> DbResult<usize> {
    let comment = post_comment::Entity::find_by_id(comment_id)
        .one(conn)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    let flags = comment_flag::Entity::find()
        .filter(comment_flag::Column::CommentId.eq(comment_id))
        .order_by_asc(comment_flag::Column::Id)
        .all(conn)
        .await?;
    let author = user::Entity::find_by_id(comment.user_id).one(conn).await?;
    let sessions = user_session::Entity::find()
        .filter(user_session::Column::UserId.eq(comment.user_id))
        .order_by_asc(user_session::Column::Id)
        .all(conn)
        .await?;
    let views = post_view::Entity::find()
        .filter(post_view::Column::PostId.eq(comment.post_id))
        .filter(post_view::Column::UserId.eq(comment.user_id))
        .order_by_desc(post_view::Column::CreatedAt)
        .limit(VIEWS_LIMIT)
        .all(conn)
        .await?;
    let avatar = match author.as_ref().and_then(|u| u.avatar_id) {
        // Trait call: the media module's inherent `find_by_id` takes a concrete DbConn.
        Some(id) => {
            <media::model::Entity as EntityTrait>::find_by_id(id)
                .one(conn)
                .await?
        }
        None => None,
    };
    let audit_rows = audit_log::Entity::find()
        .filter(audit_log::Column::ResourceType.eq("comment"))
        .filter(audit_log::Column::ResourceId.eq(comment_id.to_string()))
        .order_by_asc(audit_log::Column::Id)
        .all(conn)
        .await?;

    let subject = comment_id.to_string();
    let row = |kind: &'static str, snapshot: Value| NewEvidence {
        case_ref: case_ref.to_string(),
        kind,
        subject_id: subject.clone(),
        snapshot,
    };
    let rows = vec![
        row(
            "comment",
            json!({
                "id": comment.id,
                "post_id": comment.post_id,
                "user_id": comment.user_id,
                "content": comment.content,
                "hidden": comment.hidden,
                "likes_count": comment.likes_count,
                "flags_count": comment.flags_count,
                "created_at": comment.created_at,
                "updated_at": comment.updated_at,
            }),
        ),
        row(
            "comment_flags",
            Value::Array(
                flags
                    .iter()
                    .map(|f| {
                        json!({
                            "id": f.id,
                            "user_id": f.user_id,
                            "reason": f.reason,
                            "created_at": f.created_at,
                        })
                    })
                    .collect(),
            ),
        ),
        row("user", author.as_ref().map_or(Value::Null, user_snapshot)),
        row(
            "sessions",
            Value::Array(
                sessions
                    .iter()
                    .map(|s| {
                        json!({
                            "id": s.id,
                            "ip_address": s.ip_address,
                            "last_seen": s.last_seen,
                            "revoked_at": s.revoked_at,
                        })
                    })
                    .collect(),
            ),
        ),
        row(
            "views",
            Value::Array(
                views
                    .iter()
                    .map(|v| {
                        json!({
                            "id": v.id,
                            "post_id": v.post_id,
                            "ip_address": v.ip_address,
                            "created_at": v.created_at,
                        })
                    })
                    .collect(),
            ),
        ),
        row(
            "media",
            json!({ "avatar": avatar.as_ref().map_or(Value::Null, media_snapshot) }),
        ),
        row(
            "audit",
            Value::Array(
                audit_rows
                    .iter()
                    .map(|a| {
                        json!({
                            "id": a.id,
                            "user_id": a.user_id,
                            "action": a.action,
                            "resource_type": a.resource_type,
                            "resource_id": a.resource_id,
                            "metadata": a.metadata,
                            "created_at": a.created_at,
                        })
                    })
                    .collect(),
            ),
        ),
    ];
    case_evidence::Entity::append(conn, rows, now).await
}

fn user_snapshot(u: &user::Model) -> Value {
    json!({
        "id": u.id,
        "name": u.name,
        "email": u.email,
        "role": u.role.to_i32(),
        "is_verified": u.is_verified,
        "two_fa_enabled": u.two_fa_enabled,
        "oauth_provider": u.oauth_provider,
        "avatar_id": u.avatar_id,
        "created_at": u.created_at,
    })
}

fn media_snapshot(m: &media::Model) -> Value {
    json!({
        "id": m.id,
        "object_key": m.object_key,
        "mime_type": m.mime_type,
        "width": m.width,
        "height": m.height,
        "size": m.size,
        "created_at": m.created_at,
    })
}

/// Capture the comment behind an intake report, plus one `incident.capture` audit
/// row, in one transaction. Only comment reports are captured in A25.
pub async fn capture_report(
    db: &DatabaseConnection,
    report_id: i64,
    now: DateTime<Utc>,
) -> DbResult<CaptureOutcome> {
    let txn = db.begin().await?;
    let row = report::Entity::find_by_id(report_id)
        .one(&txn)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    if row.target_kind != report::ReportTargetKind::Comment.as_str() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Only comment reports can be captured"));
    }
    let comment_id: i32 = row
        .target_id
        .parse()
        .map_err(|_| ErrorResponse::new(ErrorCode::InvalidValue))?;
    let case_ref = report_case_ref(report_id);
    let rows = capture_comment(&txn, &case_ref, comment_id, now).await?;
    audit::record(
        &txn,
        AuditEvent {
            actor_id: None,
            action: AuditAction::IncidentCapture,
            resource_id: case_ref.clone(),
            metadata: json!({ "report_id": report_id, "comment_id": comment_id, "rows": rows }),
        },
    )
    .await?;
    txn.commit().await?;
    Ok(CaptureOutcome { case_ref, rows })
}
