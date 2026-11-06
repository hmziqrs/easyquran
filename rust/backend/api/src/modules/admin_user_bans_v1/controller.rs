use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Duration, FixedOffset};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait};
use serde_json::json;
use tracing::{info, instrument};

use crate::db::sea_models::audit_log;
use crate::db::sea_models::enforcement_notice::{self, NewNotice};
use crate::db::sea_models::user::{self, new_session_auth_secret};
use crate::db::sea_models::user_ban::{self, NewUserBan, UserBanQuery};
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::auth::AuthSession;
use crate::AppState;

use super::dto::{BanCreated, CreateBanPayload, RenewBanPayload};

/// Longest ban a single decision can set (A19). Bans always expire; there is no
/// permanent ban. Renewal is capped at the same length from now.
pub const MAX_BAN_DAYS: i64 = 365;
pub const MAX_REASON_TEXT_CHARS: usize = 1_000;
pub const MAX_REVIEW_NOTE_CHARS: usize = 500;

/// The write path ships dark. Routes mount only when this env flag is true.
/// Stays off until A20, A21, A22 and A35 land (one release, see docs/plans/rules.md).
pub const WRITES_FLAG: &str = "ABUSE_BAN_WRITES_ENABLED";

pub fn writes_enabled() -> bool {
    crate::config::env::env_bool(WRITES_FLAG, false)
}

fn actor(auth: &AuthSession) -> Result<user::Model, ErrorResponse> {
    auth.user
        .clone()
        .ok_or_else(|| ErrorResponse::new(ErrorCode::Unauthorized))
}

fn invalid(message: &str) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidInput).with_message(message)
}

fn check_expiry(
    expires_at: chrono::DateTime<FixedOffset>,
    now: chrono::DateTime<FixedOffset>,
) -> Result<(), ErrorResponse> {
    if expires_at <= now {
        return Err(invalid("expiresAt must be in the future"));
    }
    if expires_at > now + Duration::days(MAX_BAN_DAYS) {
        return Err(invalid("expiresAt exceeds the maximum ban length"));
    }
    Ok(())
}

/// Rotate the account's session secret. Every existing session fails the
/// extractor's hash check on its next request; sessions rows are not deleted.
async fn rotate_session_secret<C: ConnectionTrait>(
    conn: &C,
    user_id: i32,
) -> Result<(), ErrorResponse> {
    let secret = new_session_auth_secret().map_err(|err| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message(format!("session_auth_secret rotation failed: {err}"))
    })?;
    user::ActiveModel {
        id: Set(user_id),
        session_auth_secret: Set(secret),
        ..Default::default()
    }
    .update(conn)
    .await?;
    Ok(())
}

/// Audit row for an enforcement action. A23 moves this into services/audit.rs.
async fn write_audit<C: ConnectionTrait>(
    conn: &C,
    actor_id: i32,
    action: &str,
    ban_id: i32,
    metadata: serde_json::Value,
) -> Result<(), ErrorResponse> {
    audit_log::model::ActiveModel {
        user_id: Set(Some(actor_id)),
        action: Set(action.to_string()),
        resource_type: Set("user_ban".to_string()),
        resource_id: Set(ban_id.to_string()),
        metadata: Set(Some(metadata)),
        created_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    }
    .insert(conn)
    .await?;
    Ok(())
}

#[instrument(skip(state, auth))]
pub async fn list_bans(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(query): Query<UserBanQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let _admin = actor(&auth)?;
    let page = user_ban::Entity::admin_list(&state.sea_db, query).await?;
    Ok((StatusCode::OK, Json(page)))
}

#[instrument(skip(state, auth, payload), fields(user_id = payload.user_id))]
pub async fn create_ban(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(payload): Json<CreateBanPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor(&auth)?;
    let now = chrono::Utc::now().fixed_offset();
    check_expiry(payload.expires_at, now)?;
    if payload
        .reason_text
        .as_ref()
        .is_some_and(|text| text.chars().count() > MAX_REASON_TEXT_CHARS)
    {
        return Err(invalid("reasonText is too long"));
    }
    if payload.user_id == admin.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("an admin cannot ban their own account"));
    }

    let txn = state.sea_db.begin().await?;
    let target = user::Entity::find_by_id_with_404(&txn, payload.user_id).await?;
    if target.role.to_i32() >= admin.role.to_i32() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("cannot ban an account with an equal or higher role"));
    }

    // Revoke expired bans first: the one-active-ban index counts unrevoked rows.
    user_ban::Entity::revoke_expired_for_user(&txn, target.id, now).await?;
    if let Some(existing) = user_ban::Entity::get_active_ban(&txn, target.id).await? {
        return Err(ErrorResponse::new(ErrorCode::DuplicateEntry)
            .with_message("account already has an active ban")
            .with_context(json!({ "existingBanId": existing.id })));
    }

    let reason_code = payload.reason_code.as_str();
    let corroborator = payload.corroborator.as_str();
    let ban = user_ban::Entity::create(
        &txn,
        NewUserBan::new(target.id)
            .with_reason(reason_code)
            .with_banned_by(admin.id)
            .with_expiry(payload.expires_at),
    )
    .await?;
    rotate_session_secret(&txn, target.id).await?;
    let notice = enforcement_notice::Entity::insert_notice(
        &txn,
        NewNotice {
            user_id: target.id,
            ban_id: ban.id,
            reason_code: reason_code.to_string(),
            reason_text: payload.reason_text.clone(),
            corroborator: corroborator.to_string(),
            decided_by: admin.id,
            decided_at: now.timestamp(),
            effective_at: now.timestamp(),
            expires_at: payload.expires_at.timestamp(),
        },
    )
    .await?;
    write_audit(
        &txn,
        admin.id,
        "ban.create",
        ban.id,
        json!({
            "userId": target.id,
            "reasonCode": reason_code,
            "corroborator": corroborator,
            "expiresAt": payload.expires_at.to_rfc3339(),
        }),
    )
    .await?;
    txn.commit().await?;

    info!(ban_id = ban.id, user_id = target.id, "account ban created");
    Ok((
        StatusCode::CREATED,
        Json(BanCreated {
            ban_id: ban.id,
            notice_id: notice.id,
            user_id: target.id,
            expires_at: payload.expires_at.timestamp(),
        }),
    ))
}

#[instrument(skip(state, auth))]
pub async fn revoke_ban(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(ban_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor(&auth)?;
    let txn = state.sea_db.begin().await?;
    let ban = user_ban::Entity::find_by_id(ban_id)
        .one(&txn)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    if ban.revoked_at.is_some() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("ban is already revoked"));
    }
    user_ban::Entity::revoke(&txn, ban_id, Some(admin.id)).await?;
    write_audit(
        &txn,
        admin.id,
        "ban.revoke",
        ban_id,
        json!({ "userId": ban.user_id }),
    )
    .await?;
    txn.commit().await?;
    info!(ban_id, "account ban revoked");
    Ok((
        StatusCode::OK,
        Json(json!({ "banId": ban_id, "revoked": true })),
    ))
}

/// Extend an active ban. Needs a recorded self-review; the longest ban still
/// auto-expires, so this only moves the expiry (capped at MAX_BAN_DAYS from now).
#[instrument(skip(state, auth, payload))]
pub async fn renew_ban(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(ban_id): Path<i32>,
    Json(payload): Json<RenewBanPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor(&auth)?;
    if !payload.single_operator_control {
        return Err(invalid("singleOperatorControl must be true"));
    }
    let note_len = payload.self_review_note.trim().chars().count();
    if note_len == 0 || note_len > MAX_REVIEW_NOTE_CHARS {
        return Err(invalid("selfReviewNote must be 1 to 500 characters"));
    }
    let now = chrono::Utc::now().fixed_offset();
    let txn = state.sea_db.begin().await?;
    let ban = user_ban::Entity::find_by_id(ban_id)
        .one(&txn)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    if !ban.is_active() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("only an active ban can be renewed"));
    }
    let previous = ban.expires_at;
    if let Some(prev) = previous {
        if payload.expires_at <= prev {
            return Err(invalid("renewal must extend the current expiry"));
        }
    }
    check_expiry(payload.expires_at, now)?;
    let user_id = ban.user_id;
    user_ban::Entity::set_expiry(&txn, ban, payload.expires_at).await?;
    write_audit(
        &txn,
        admin.id,
        "ban.renew",
        ban_id,
        json!({
            "userId": user_id,
            "previousExpiresAt": previous.map(|p| p.to_rfc3339()),
            "expiresAt": payload.expires_at.to_rfc3339(),
            "singleOperatorControl": true,
            "selfReviewNote": payload.self_review_note.trim(),
        }),
    )
    .await?;
    txn.commit().await?;
    info!(ban_id, "account ban renewed after self-review");
    Ok((
        StatusCode::OK,
        Json(json!({ "banId": ban_id, "expiresAt": payload.expires_at.timestamp() })),
    ))
}

/// Notice delivery is A21. Until it lands this route refuses and sends nothing,
/// so no notice can read as delivered.
#[instrument(skip(state, auth))]
pub async fn resend_notice(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(ban_id): Path<i32>,
) -> Result<StatusCode, ErrorResponse> {
    let _admin = actor(&auth)?;
    let exists = user_ban::Entity::find_by_id(ban_id)
        .one(&state.sea_db)
        .await?
        .is_some();
    if !exists {
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound));
    }
    Err(ErrorResponse::new(ErrorCode::ServiceUnavailable)
        .with_message("notice delivery is not enabled yet (A21)"))
}
