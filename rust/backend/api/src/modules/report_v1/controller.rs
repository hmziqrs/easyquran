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

/// Row ids are positive integers for post and comment rows.
fn parse_row_id(raw: &str) -> Result<i32, ErrorResponse> {
    let id: i32 = raw.trim().parse().map_err(|_| invalid_target())?;
    if id <= 0 {
        return Err(invalid_target());
    }
    Ok(id)
}

/// Translation source ids look like `ar.jalalayn`: a 2-3 letter language, a dot,
/// then a short name. Membership in the baked translation list stays a web concern.
fn valid_source_id(raw: &str) -> bool {
    let Some((lang, name)) = raw.split_once('.') else {
        return false;
    };
    (2..=3).contains(&lang.len())
        && lang.bytes().all(|b| b.is_ascii_lowercase())
        && (1..=40).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Translation target `<source_id>:<surah>:<ayah>`. Rebuilt from the parsed parts,
/// so the stored id is canonical (no `+`, no leading zeros, no stray spaces).
fn parse_translation_target(raw: &str) -> Result<String, ErrorResponse> {
    let parts: Vec<&str> = raw.trim().split(':').collect();
    let [source, surah, ayah] = parts.as_slice() else {
        return Err(invalid_target());
    };
    if !valid_source_id(source) {
        return Err(invalid_target());
    }
    let surah: u16 = surah.parse().map_err(|_| invalid_target())?;
    let ayah: u16 = ayah.parse().map_err(|_| invalid_target())?;
    if !(1..=114).contains(&surah) || !(1..=286).contains(&ayah) {
        return Err(invalid_target());
    }
    Ok(format!("{source}:{surah}:{ayah}"))
}

/// Canonical stored target id for the kind.
fn parse_target(kind: ReportTargetKind, raw: &str) -> Result<String, ErrorResponse> {
    match kind {
        ReportTargetKind::Translation => parse_translation_target(raw),
        ReportTargetKind::Post | ReportTargetKind::Comment => Ok(parse_row_id(raw)?.to_string()),
    }
}

/// P0 capture (A24): copy the reported item into the report row at intake, so
/// the content survives a later removal. `None` when the target no longer exists,
/// and always `None` for translation: the verse lives in an immutable database.
async fn capture_snapshot(
    db: &DatabaseConnection,
    kind: ReportTargetKind,
    target_id: &str,
) -> Result<Option<Value>, ErrorResponse> {
    let captured_at = Utc::now().to_rfc3339();
    match kind {
        ReportTargetKind::Translation => Ok(None),
        ReportTargetKind::Comment => {
            let id = parse_row_id(target_id)?;
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
            let id = parse_row_id(target_id)?;
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

    let reason: ReportReason = payload.reason;
    let kind: ReportTargetKind = payload.target_kind;
    let target_id = parse_target(kind, &payload.target_id)?;

    let snapshot = if reason.is_p0() {
        capture_snapshot(&state.sea_db, kind, &target_id).await?
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
            target_id,
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
