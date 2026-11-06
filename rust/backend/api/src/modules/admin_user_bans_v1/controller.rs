use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Duration, FixedOffset};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set, TransactionTrait};
use serde_json::json;
use tracing::{info, instrument, warn};

use crate::db::sea_models::enforcement_notice::{self, DeliveryState, NewNotice};
use crate::db::sea_models::user::{self, new_session_auth_secret};
use crate::db::sea_models::user_ban::{self, NewUserBan, UserBanQuery};
use crate::error::{ErrorCode, ErrorResponse};
use crate::modules::admin_bans_v1;
use crate::modules::appeal_v1;
use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::services::auth::AuthSession;
use crate::AppState;

use super::dto::{BanCreated, CreateBanPayload, RenewBanPayload};
use super::notice as delivery;

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

#[instrument(skip(state, auth))]
pub async fn list_bans(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(query): Query<UserBanQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let _admin = actor(&auth)?;
    // Appeal decision-clock alert (A35). Runs on each admin list load: no scheduler yet.
    if let Err(err) =
        appeal_v1::controller::check_appeal_clock(&state.sea_db, chrono::Utc::now().timestamp())
            .await
    {
        warn!(error = ?err, "appeal clock check failed");
    }
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
    if payload.shared_device_checked != Some(true) {
        return Err(invalid(
            "sharedDeviceChecked must be true: run the shared-device step in deploy/moderation-sop.md",
        ));
    }
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
            // Refused above unless true, so the stored decision records the check.
            shared_device_checked: true,
            decided_by: admin.id,
            decided_at: now.timestamp(),
            effective_at: now.timestamp(),
            expires_at: payload.expires_at.timestamp(),
        },
    )
    .await?;
    audit::record(
        &txn,
        AuditEvent {
            actor_id: Some(admin.id),
            action: AuditAction::BanCreate,
            resource_id: ban.id.to_string(),
            metadata: json!({
                "userId": target.id,
                "reasonCode": reason_code,
                "corroborator": corroborator,
                "expiresAt": payload.expires_at.to_rfc3339(),
            }),
        },
    )
    .await?;
    txn.commit().await?;

    // Lift the account's login/TOTP/reauth throttle after the ban commits, so the
    // banned response (not a 429) is what the account sees next. Runs after commit:
    // the pool has one connection and the transaction holds it until here.
    if let Err(err) = admin_bans_v1::controller::clear_identity_limits_for(
        &state.sea_db,
        &state.gate_store,
        target.id,
    )
    .await
    {
        warn!(user_id = target.id, error = ?err, "ban created; identity limiter clear failed");
    }

    // Notice goes out after the ban commits: the row already exists (state NULL)
    // and the send result is recorded on it. A delivery error leaves the state NULL,
    // which reads as unknown, never as sent.
    let delivery_state = match deliver_notice(&state, &notice, Some(&target)).await {
        Ok(delivery) => Some(delivery.as_str().to_string()),
        Err(err) => {
            warn!(notice_id = notice.id, error = ?err, "ban created; notice delivery record failed");
            None
        }
    };

    info!(ban_id = ban.id, user_id = target.id, "account ban created");
    Ok((
        StatusCode::CREATED,
        Json(BanCreated {
            ban_id: ban.id,
            notice_id: notice.id,
            user_id: target.id,
            expires_at: payload.expires_at.timestamp(),
            delivery_state,
        }),
    ))
}

/// Sends a notice for the account, or records `no_contact_details` when the
/// account is gone. A send issues a fresh appeal link first (A35); the stored
/// hash is replaced, so an earlier link stops working.
async fn deliver_notice(
    state: &AppState,
    notice: &enforcement_notice::Model,
    target: Option<&user::Model>,
) -> Result<DeliveryState, ErrorResponse> {
    let contact = target.map(|user| (user.email.as_str(), user.name.as_str()));
    let appeal_url = match contact {
        Some(_) => Some(issue_appeal_link(state, notice).await?),
        None => None,
    };
    let delivery = delivery::deliver(
        &state.sea_db,
        &state.mailer,
        &state.settings.site.name,
        notice,
        contact,
        appeal_url.as_deref(),
    )
    .await?;
    Ok(delivery)
}

async fn issue_appeal_link(
    state: &AppState,
    notice: &enforcement_notice::Model,
) -> Result<String, ErrorResponse> {
    let token = appeal_v1::controller::new_token();
    let hash = appeal_v1::controller::token_hash(&state.secret_key, &token);
    enforcement_notice::Entity::set_appeal_token_hash(&state.sea_db, notice.id, &hash).await?;
    Ok(appeal_v1::controller::appeal_url(
        &state.settings.site.url,
        &token,
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
    audit::record(
        &txn,
        AuditEvent {
            actor_id: Some(admin.id),
            action: AuditAction::BanRevoke,
            resource_id: ban_id.to_string(),
            metadata: json!({ "userId": ban.user_id }),
        },
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
    audit::record(
        &txn,
        AuditEvent {
            actor_id: Some(admin.id),
            action: AuditAction::BanRenew,
            resource_id: ban_id.to_string(),
            metadata: json!({
                "userId": user_id,
                "previousExpiresAt": previous.map(|p| p.to_rfc3339()),
                "expiresAt": payload.expires_at.to_rfc3339(),
                "singleOperatorControl": true,
                "selfReviewNote": payload.self_review_note.trim(),
            }),
        },
    )
    .await?;
    txn.commit().await?;
    info!(ban_id, "account ban renewed after self-review");
    Ok((
        StatusCode::OK,
        Json(json!({ "banId": ban_id, "expiresAt": payload.expires_at.timestamp() })),
    ))
}

/// Re-sends the latest notice for a ban. The stored state is overwritten with the
/// new attempt's result and the attempt is audited.
#[instrument(skip(state, auth))]
pub async fn resend_notice(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(ban_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor(&auth)?;
    let ban = user_ban::Entity::find_by_id(ban_id)
        .one(&state.sea_db)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    let notice = enforcement_notice::Entity::find_latest_for_ban(&state.sea_db, ban_id)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    let target = user::Entity::find_by_id(ban.user_id)
        .one(&state.sea_db)
        .await?;
    let delivery = deliver_notice(&state, &notice, target.as_ref()).await?;
    write_audit(
        &state.sea_db,
        admin.id,
        "ban.notice_resend",
        ban_id,
        json!({
            "noticeId": notice.id,
            "userId": ban.user_id,
            "deliveryState": delivery.as_str(),
        }),
    )
    .await?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "banId": ban_id,
            "noticeId": notice.id,
            "deliveryState": delivery.as_str(),
        })),
    ))
}
