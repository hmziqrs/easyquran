use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_client_ip::ClientIp;
use axum_macros::debug_handler;
use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, TransactionTrait};
use serde_json::{json, Value};
use tracing::{info, instrument};

use crate::{
    db::sea_models::{
        post, post_comment,
        report::{Entity as ReportEntity, NewReport, ReportReason, ReportTargetKind},
    },
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::{
        abuse_limiter::{limiter, AbuseLimiterConfig},
        audit::{self, AuditAction, AuditEvent},
        auth::AuthSession,
        operator_alert,
    },
    AppState,
};

use super::validator::V1SubmitReportPayload;

/// 5 reports per hour and 20 per day per IP. Same limiter as the newsletter
/// subscribe bucket: a temp block after the hourly count, a long block after the daily.
/// The gate refuses the call that reaches `temp_block_attempts`, so the hourly
/// threshold is 6 to let five through.
pub fn report_limit_config() -> AbuseLimiterConfig {
    AbuseLimiterConfig {
        temp_block_attempts: 6,
        temp_block_range: 60 * 60,
        temp_block_duration: 60 * 60,
        block_retry_limit: 20,
        block_range: 24 * 60 * 60,
        block_duration: 24 * 60 * 60,
    }
}

fn invalid_target() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidValue).with_message("Target id is not valid")
}

/// Target ids are positive integers for both post and comment rows.
fn parse_target_id(raw: &str) -> Result<i32, ErrorResponse> {
    let id: i32 = raw.trim().parse().map_err(|_| invalid_target())?;
    if id <= 0 {
        return Err(invalid_target());
    }
    Ok(id)
}

/// P0 capture (A24): copy the reported item into the report row at intake, so
/// the content survives a later removal. `None` when the target no longer exists.
async fn capture_snapshot(
    db: &DatabaseConnection,
    kind: ReportTargetKind,
    id: i32,
) -> Result<Option<Value>, ErrorResponse> {
    let captured_at = Utc::now().to_rfc3339();
    match kind {
        ReportTargetKind::Comment => {
            let Some(c) = post_comment::Entity::find_by_id(id).one(db).await? else {
                return Ok(None);
            };
            Ok(Some(json!({
                "kind": "comment",
                "id": c.id,
                "post_id": c.post_id,
                "user_id": c.user_id,
                "content": c.content,
                "hidden": c.hidden,
                "created_at": c.created_at.to_rfc3339(),
                "captured_at": captured_at,
            })))
        }
        ReportTargetKind::Post => {
            let Some(p) = post::Entity::find_by_id(id).one(db).await? else {
                return Ok(None);
            };
            Ok(Some(json!({
                "kind": "post",
                "id": p.id,
                "title": p.title,
                "slug": p.slug,
                "author_id": p.author_id,
                "content": p.content,
                "captured_at": captured_at,
            })))
        }
    }
}

fn clean_details(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn clean_contact(raw: Option<&str>) -> Option<String> {
    clean_details(raw).map(|s| s.to_lowercase())
}

#[debug_handler]
#[instrument(skip(state, auth, client_ip, payload), fields(reason = payload.reason.as_str()))]
pub async fn submit(
    State(state): State<AppState>,
    auth: AuthSession,
    ClientIp(client_ip): ClientIp,
    payload: ValidatedJson<V1SubmitReportPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    limiter(
        &state.gate_store,
        &format!("report:submit:ip:{client_ip}"),
        report_limit_config(),
    )
    .await?;

    let target_id = parse_target_id(&payload.target_id)?;
    let reason: ReportReason = payload.reason;
    let kind: ReportTargetKind = payload.target_kind;

    let snapshot = if reason.is_p0() {
        capture_snapshot(&state.sea_db, kind, target_id).await?
    } else {
        None
    };

    let reporter = if auth.is_authenticated() {
        Some(auth.user_required()?.id)
    } else {
        None
    };

    // Row and audit row commit together. The audit metadata carries only the
    // closed fields: free-text details and the contact address stay in the row.
    let txn = state.sea_db.begin().await?;
    let report = ReportEntity::insert_report(
        &txn,
        NewReport {
            reporter_user_id: reporter,
            target_kind: kind,
            target_id: target_id.to_string(),
            reason,
            details: clean_details(payload.details.as_deref()),
            contact_email: clean_contact(payload.contact_email.as_deref()),
            snapshot,
        },
    )
    .await?;
    audit::record(
        &txn,
        AuditEvent {
            actor_id: reporter,
            action: AuditAction::ReportSubmit,
            resource_id: report.id.to_string(),
            metadata: json!({
                "reason": reason.as_str(),
                "target_kind": kind.as_str(),
                "priority": reason.priority(),
            }),
        },
    )
    .await?;
    txn.commit().await?;

    info!(
        report_id = report.id,
        priority = reason.priority(),
        "Report received"
    );

    if reason.is_p0() {
        operator_alert::spawn_p0(&state, report.id);
    }

    Ok((
        StatusCode::CREATED,
        Json(json!({ "message": "Report received" })),
    ))
}
