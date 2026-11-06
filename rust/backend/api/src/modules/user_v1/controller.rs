use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_macros::debug_handler;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionError, TransactionTrait};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use super::validator::*;
use crate::{
    config::settings::{web_auth, ProviderStatus},
    db::sea_models::{
        email_change_pending, email_verification, passkey_credential,
        user::{Entity as User, Model as UserModel, UserRole},
        user_oauth_identity, user_session,
    },
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::{
        abuse_limiter,
        account_methods::{can_unlink, total_sign_in_methods},
        auth::AuthSession,
        data_request::{self, RequestError},
    },
    AppState,
};

/// Providers a signed-in user may link/unlink from the account UI. Narrower than
/// `OAuthProvider`: only these three are currently offered as sign-in methods.
const SUPPORTED_IDENTITY_PROVIDERS: [&str; 3] = ["github", "discord", "google"];

fn is_supported_identity_provider(provider: &str) -> bool {
    SUPPORTED_IDENTITY_PROVIDERS.contains(&provider)
}

fn linkable_providers(configured: &[ProviderStatus], linked: &[&str]) -> Vec<String> {
    configured
        .iter()
        .filter(|status| {
            status.ready
                && is_supported_identity_provider(&status.name)
                && !linked.contains(&status.name.as_str())
        })
        .map(|status| status.name.clone())
        .collect()
}

async fn identity_snapshot(
    state: &AppState,
    user: &UserModel,
) -> Result<serde_json::Value, ErrorResponse> {
    let identities = user_oauth_identity::Entity::list_for_user(&state.sea_db, user.id).await?;
    let passkey_count = passkey_credential::Entity::count_for_user(&state.sea_db, user.id).await?;
    let has_password = user.password.is_some();
    let linked: Vec<&str> = identities
        .iter()
        .map(|identity| identity.provider.as_str())
        .collect();
    let views: Vec<serde_json::Value> = identities
        .iter()
        .map(|identity| {
            json!({
                "provider": identity.provider,
                "provider_user_id": identity.provider_user_id,
                "created_at": identity.created_at,
            })
        })
        .collect();

    Ok(json!({
        "identities": views,
        "has_password": has_password,
        "passkey_count": passkey_count,
        "can_unlink_any": can_unlink(total_sign_in_methods(
            identities.len(),
            has_password,
            passkey_count,
        )),
        "linkable_providers": linkable_providers(&web_auth().providers_status, &linked),
    }))
}

#[debug_handler]
#[instrument(skip(auth, state), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn list_identities(
    auth: AuthSession,
    state: State<AppState>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;

    let body = identity_snapshot(&state, &user).await?;
    info!(user_id = user.id, "Sign-in identities listed");
    Ok((StatusCode::OK, Json(body)))
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn unlink_identity(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1UnlinkIdentityPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.as_ref().ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;
    let provider = payload.0.provider.trim().to_lowercase();
    if !is_supported_identity_provider(&provider) {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Unsupported sign-in provider"));
    }

    crate::services::fresh_auth::consume(&auth, &state.sea_db).await?;
    let user_id = user.id;
    // Guard reads + delete in ONE transaction: separate pool checkouts let two
    // concurrent unlinks each pass the count guard and both delete, leaving the
    // account with zero sign-in methods (permanent lockout).
    let removed = state
        .sea_db
        .transaction::<_, u64, ErrorResponse>(|txn| {
            let provider = provider.clone();
            Box::pin(async move {
                // Re-read inside the txn: password presence can change concurrently.
                let Some(fresh_user) = User::find_by_id(user_id).one(txn).await? else {
                    return Err(ErrorResponse::new(ErrorCode::Unauthorized)
                        .with_message("You must be logged in to access this resource"));
                };

                let identities =
                    user_oauth_identity::Entity::list_for_user(txn, user_id).await?;
                if !identities
                    .iter()
                    .any(|identity| identity.provider == provider)
                {
                    return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                        .with_message("This sign-in method is not linked to your account"));
                }

                let passkey_count =
                    passkey_credential::Entity::count_for_user(txn, user_id).await?;
                let total = total_sign_in_methods(
                    identities.len(),
                    fresh_user.password.is_some(),
                    passkey_count,
                );
                if !can_unlink(total) {
                    warn!(user_id, provider = %provider, "Refused to unlink the last sign-in method");
                    // 409 (BIZ_004): the request conflicts with the "keep one method" invariant.
                    return Err(ErrorResponse::new(ErrorCode::DependencyExists)
                        .with_message("Add a password or another sign-in method first"));
                }

                user_oauth_identity::Entity::delete_for_user_provider(txn, user_id, &provider).await
            })
        })
        .await
        .map_err(|err| match err {
            TransactionError::Connection(err) => ErrorResponse::from(err),
            TransactionError::Transaction(err) => err,
        })?;

    if removed == 0 {
        // Lost a race with a concurrent unlink; the method is already gone.
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
            .with_message("This sign-in method is not linked to your account"));
    }

    info!(user_id = user.id, provider = %provider, "Unlinked OAuth sign-in method");
    let body = identity_snapshot(&state, user).await?;
    Ok((StatusCode::OK, Json(body)))
}

#[debug_handler(state = AppState)]
#[instrument(skip(auth), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn get_profile(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => {
            info!(user_id = user.id, "Profile retrieved");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        None => {
            warn!("Profile request with no authenticated user");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("No user with this ID exists"))
        }
    }
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn update_profile(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1UpdateProfilePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.as_ref().ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;

    let delivery = EmailChangeDelivery {
        secret_key: &state.secret_key,
        mailer: state.mailer.clone(),
    };
    match update_profile_with_auth(&state.sea_db, &auth, payload.0, delivery).await {
        Ok(Some(outcome)) => {
            info!(user_id = outcome.user.id, "Profile updated");
            let mut body = json!(outcome.user);
            body["email_change_pending"] = json!(outcome.email_change_pending);
            Ok((StatusCode::OK, Json(body)))
        }
        Ok(None) => {
            warn!(user_id = user.id, "User not found during update");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("User could not be found or updated"))
        }
        Err(err) => {
            error!(user_id = user.id, "Failed to update profile: {}", err);
            Err(err)
        }
    }
}

/// What a profile update did. `email_change_pending` means the new address
/// is parked and waits for a code; `user.email` still holds the old one.
pub struct ProfileUpdate {
    pub user: UserModel,
    pub email_change_pending: bool,
}

/// Where a parked email change sends its code, and the key that makes the code
/// hash. Built from `AppState` by the handler.
pub struct EmailChangeDelivery<'a> {
    pub secret_key: &'a [u8],
    pub mailer: std::sync::Arc<crate::services::mail::MailRouter>,
}

pub async fn update_profile_with_auth(
    db: &sea_orm::DatabaseConnection,
    auth: &AuthSession,
    payload: V1UpdateProfilePayload,
    delivery: EmailChangeDelivery<'_>,
) -> Result<Option<ProfileUpdate>, ErrorResponse> {
    let owner = auth.user_required()?;
    let requested_email = payload
        .email
        .as_ref()
        .map(|email| crate::utils::email::normalize_email(email))
        .filter(|email| *email != owner.email);
    if requested_email.is_some() {
        crate::services::fresh_auth::consume(auth, db).await?;
    }
    // The address never switches in the profile write. It parks until the
    // code on the new mailbox is confirmed (A12).
    let mut update = payload.into_update_user();
    update.email = None;
    let Some(user) = User::update(db, owner.id, update).await? else {
        return Ok(None);
    };
    let email_change_pending = match requested_email {
        Some(new_email) => {
            request_email_change(db, &delivery, owner.id, &new_email).await?;
            true
        }
        None => false,
    };
    Ok(Some(ProfileUpdate {
        user,
        email_change_pending,
    }))
}

/// Parks a change to `new_email` and sends its code there. Non-production logs
/// the code (dev has no SMTP); production sends detached, so a transport error
/// is logged and does not reveal anything to the requester.
async fn request_email_change(
    db: &sea_orm::DatabaseConnection,
    delivery: &EmailChangeDelivery<'_>,
    user_id: i32,
    new_email: &str,
) -> Result<(), ErrorResponse> {
    let identity_hash = crate::utils::email::verification_identity_hash(new_email);
    crate::services::registration_admission::ensure_identity_capacity(
        db,
        &identity_hash,
        Some(user_id),
    )
    .await?;
    let code = email_verification::Entity::generate_code();
    let code_hash = crate::utils::code_hash::hash_code(delivery.secret_key, &code);
    email_change_pending::Entity::park(db, user_id, new_email, Some(identity_hash), code_hash)
        .await?;

    // ── DELIVERY: email-change code hand-off ──────────────────────────────────
    // Same contract as the other DELIVERY blocks: the plaintext code leaves here
    // and nowhere else. NEVER log the code on the production branch.
    if matches!(crate::config::settings::is_production(), Ok(false)) {
        info!(
            user_id,
            verification_code = %code,
            "DEV delivery: email-change code (non-production build)"
        );
    } else {
        let mailer: Arc<crate::services::mail::MailRouter> = delivery.mailer.clone();
        let new_email = new_email.to_owned();
        tokio::spawn(async move {
            if let Err(err) =
                crate::services::mail::send_email_verification_code(&mailer, &new_email, &code)
                    .await
            {
                error!(
                    user_id,
                    error_kind = crate::services::mail::mail_error_kind(&err),
                    "Failed to send email-change code"
                );
            }
        });
    }
    Ok(())
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn confirm_email_change(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1ConfirmEmailChangePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user_id = auth.user_required()?.id;
    let key_prefix = format!("email_change:{}", user_id);
    abuse_limiter::gate(
        &state.gate_store,
        &key_prefix,
        ABUSE_LIMITER_CONFIG,
        abuse_limiter::FailureMode::Credential,
    )
    .await?;
    let code_hash = crate::utils::code_hash::hash_code(&state.secret_key, &payload.0.code);
    let confirmed = User::confirm_email_change(&state.sea_db, user_id, &code_hash).await;
    abuse_limiter::settle(
        &state.gate_store,
        &key_prefix,
        ABUSE_LIMITER_CONFIG,
        abuse_limiter::FailureMode::Credential,
        abuse_limiter::attempt_of_code(&confirmed),
    )
    .await;
    confirmed?;
    // The session secret rotated, so this session ends with the next request.
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Email address updated. Sign in again with the new address.",
        })),
    ))
}

const ABUSE_LIMITER_CONFIG: abuse_limiter::AbuseLimiterConfig = abuse_limiter::AbuseLimiterConfig {
    temp_block_attempts: 3,
    temp_block_range: 360,
    temp_block_duration: 3600,
    block_retry_limit: 5,
    block_range: 900,
    block_duration: 86400,
};

#[debug_handler]
#[instrument(skip(auth, state, payload))]
pub async fn admin_create(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1AdminCreateUserPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: without this a lower-rank admin could create a higher-rank user.
    let caller_level = auth.user.as_ref().map(|u| u.role.to_i32()).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let requested = UserRole::from_str(&payload.0.role)
        .map_err(|_| ErrorResponse::new(ErrorCode::InvalidInput).with_message("Invalid role"))?;
    if requested.to_i32() > caller_level {
        warn!(
            caller_level,
            requested = %payload.0.role,
            "Admin attempted to create a user above their own role"
        );
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot create a user with a role higher than your own"));
    }

    let payload = payload.0.into_new_user();
    let user = User::admin_create(&state.sea_db, &state.storage.config.public_url, payload).await?;
    info!(user_id = user.id, "Admin created user");
    Ok((StatusCode::CREATED, Json(json!(user))))
}

#[debug_handler]
#[instrument(skip(auth, state), fields(user_id))]
pub async fn admin_delete(
    auth: AuthSession,
    state: State<AppState>,
    Path(user_id): Path<i32>,
    payload: ValidatedJson<V1AdminDeleteUserPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: block self-deletion and deletion of equal/higher-rank users (privilege escalation / self-lockout).
    let caller = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let caller_level = caller.role.to_i32();
    if caller.id == user_id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot delete your own account via the admin path"));
    }
    let Some(target) = User::get_by_id(&state.sea_db, user_id).await? else {
        warn!(user_id, "Admin tried to delete non-existent user");
        return Err(
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("User does not exist")
        );
    };
    if target.role.to_i32() >= caller_level {
        warn!(
            caller_level,
            target_level = target.role.to_i32(),
            "Admin attempted to delete an equal/higher-role user"
        );
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot delete a user at or above your own role"));
    }
    // Typed confirmation: the caller retypes the target's email (case-insensitive).
    let typed = crate::utils::email::normalize_email(&payload.0.confirm_email);
    if typed != crate::utils::email::normalize_email(&target.email) {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Confirmation does not match the user's email"));
    }
    // Pre-state snapshot, written in the delete transaction. The cascade destroys
    // bans, sessions and reports, so the snapshot is their only record. Session
    // rows carry raw IP and UA, so only their count is kept.
    let snapshot = user_delete_snapshot(&state.sea_db, &target).await?;
    let deleted = match User::admin_delete(&state.sea_db, user_id, caller.id, snapshot).await {
        Ok(affected) => affected,
        Err(err) => {
            error!(user_id, "Failed to delete user: {}", err);
            return Err(err);
        }
    };
    if deleted == 0 {
        warn!(user_id, "Admin tried to delete non-existent user");
        return Err(
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("User does not exist")
        );
    }
    info!(user_id, "Admin deleted user");
    Ok((
        StatusCode::OK,
        Json(json!({ "message": "User deleted successfully" })),
    ))
}

/// Evidence snapshot of a user about to be deleted. No password hash, no session
/// secret, no device identity, no session IP or UA.
async fn user_delete_snapshot(
    db: &sea_orm::DatabaseConnection,
    target: &UserModel,
) -> Result<serde_json::Value, ErrorResponse> {
    use crate::db::sea_models::{report, user_ban, user_session};
    use sea_orm::{ColumnTrait, PaginatorTrait, QueryFilter};

    let bans = user_ban::Entity::find()
        .filter(user_ban::Column::UserId.eq(target.id))
        .all(db)
        .await?;
    let session_count = user_session::Entity::find()
        .filter(user_session::Column::UserId.eq(target.id))
        .count(db)
        .await?;
    let report_ids: Vec<i64> = report::Entity::find()
        .filter(report::Column::ReporterUserId.eq(target.id))
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.id)
        .collect();

    Ok(json!({
        "user": {
            "id": target.id,
            "name": target.name,
            "email": target.email,
            "role": target.role.to_i32(),
            "is_verified": target.is_verified,
            "created_at": target.created_at,
        },
        "bans": bans,
        "session_count": session_count,
        "report_ids": report_ids,
    }))
}

/// Maps the data-request service errors to the admin API's error codes.
fn data_request_error(err: RequestError) -> ErrorResponse {
    match err {
        RequestError::Db(e) => e.into(),
        RequestError::AlreadyOpen => ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("This user already has an open data request"),
        RequestError::NotFound => ErrorResponse::new(ErrorCode::RecordNotFound)
            .with_message("Data request does not exist"),
        RequestError::Closed => ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Data request is already answered"),
        // Nothing for the request is committed; it stays open and can be answered again.
        RequestError::Storage(e) => e,
    }
}

/// Opens an erasure request for a user and starts its one-month clock (A38).
#[debug_handler]
#[instrument(skip(auth, state, payload))]
pub async fn admin_open_data_request(
    auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1AdminOpenDataRequestPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let caller = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let Some(target) = User::get_by_id(&state.sea_db, payload.0.user_id).await? else {
        return Err(
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("User does not exist")
        );
    };
    let now = crate::services::job_log::epoch_now();
    let id = data_request::open(
        &state.sea_db,
        Some(caller.id),
        data_request::ORIGIN_ADMIN,
        target.id,
        now,
    )
    .await
    .map_err(data_request_error)?;
    info!(request_id = id, "Admin opened data request");
    Ok((
        StatusCode::CREATED,
        Json(json!({ "request_id": id, "due_at": now + data_request::RESPONSE_SECS })),
    ))
}

/// Open data requests, earliest deadline first.
#[debug_handler]
#[instrument(skip(auth, state))]
pub async fn admin_list_data_requests(
    auth: AuthSession,
    state: State<AppState>,
) -> Result<impl IntoResponse, ErrorResponse> {
    if auth.user.is_none() {
        return Err(ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated"));
    }
    let rows = data_request::open_queue(&state.sea_db).await?;
    Ok((StatusCode::OK, Json(json!(rows))))
}

/// Answers an open request by erasing its subject (A38). Same guards as admin delete:
/// no self-erase, no equal or higher role, typed email confirmation. Returns the receipt.
#[debug_handler]
#[instrument(skip(auth, state, payload))]
pub async fn admin_erase_data_request(
    auth: AuthSession,
    state: State<AppState>,
    Path(request_id): Path<i64>,
    payload: ValidatedJson<V1AdminDeleteUserPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let caller = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let Some(req) = data_request::get(&state.sea_db, request_id).await? else {
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
            .with_message("Data request does not exist"));
    };
    if req.closed_at.is_some() {
        return Err(data_request_error(RequestError::Closed));
    }
    let Some(user_id) = req.user_id() else {
        return Err(data_request_error(RequestError::NotFound));
    };
    let Some(target) = User::get_by_id(&state.sea_db, user_id).await? else {
        return Err(
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("User does not exist")
        );
    };
    if caller.id == target.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot erase your own account via the admin path"));
    }
    if target.role.to_i32() >= caller.role.to_i32() {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot erase a user at or above your own role"));
    }
    let typed = crate::utils::email::normalize_email(&payload.0.confirm_email);
    if typed != crate::utils::email::normalize_email(&target.email) {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Confirmation does not match the user's email"));
    }
    let now = crate::services::job_log::epoch_now();
    let out = data_request::erase(&state, Some(caller.id), request_id, now)
        .await
        .map_err(data_request_error)?;
    info!(request_id, "Admin answered data request");
    Ok((
        StatusCode::OK,
        Json(json!({
            "receipt_id": out.receipt_id,
            "deleted": out.deleted,
            "retained_tables": out.retained_tables,
            "user_row_erased": out.user_row_erased,
            "media_deleted": out.media_deleted,
            "posts_anonymized": out.posts_anonymized,
        })),
    ))
}

/// Record the account's self-declared age (A34), for accounts created without a date of
/// birth (OAuth, pre-A34 email). The first answer stands: a second call is refused. The
/// date is parsed for the minor check and never stored, logged or echoed. An under-18
/// answer sets the flag and removes device state (see `User::declare_age`).
#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn declare_age(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1DeclareAgePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let owner = auth.user_required()?;
    let today = chrono::Utc::now().date_naive();
    let dob =
        crate::modules::auth_v1::validator::parse_date_of_birth(&payload.0.date_of_birth, today)
            .map_err(|_| {
                ErrorResponse::new(ErrorCode::InvalidInput)
                    .with_message("Enter a valid date of birth")
            })?;
    let is_minor = crate::modules::auth_v1::validator::is_minor_on(dob, today);
    match User::declare_age(&state.sea_db, owner.id, is_minor).await? {
        Some(user) => {
            info!(user_id = user.id, is_minor, "Age declared");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        None => Err(ErrorResponse::new(ErrorCode::DuplicateEntry)
            .with_message("Your age is already recorded for this account")),
    }
}

/// Self-service account deletion (F7). Fresh auth first, then the typed email, then every
/// session of the caller ends, then the A38 self path erases the account: uploaded media and
/// avatar leave storage, authored posts keep their text without the author link. Immediate:
/// no grace period. A held item stays and is listed in the receipt.
#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn self_delete_account(
    mut auth: AuthSession,
    state: State<AppState>,
    payload: ValidatedJson<V1DeleteAccountPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.clone().ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("You must be logged in to access this resource")
    })?;
    crate::services::fresh_auth::consume(&auth, &state.sea_db).await?;
    let typed = crate::utils::email::normalize_email(&payload.0.confirm_email);
    if typed != crate::utils::email::normalize_email(&user.email) {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Confirmation does not match your email"));
    }
    // End every session first, so no cookie survives the erase. Sessions are not evidence,
    // so this runs even when a legal hold keeps the rows.
    let sessions = user_session::Entity::find()
        .filter(user_session::Column::UserId.eq(user.id))
        .all(&state.sea_db)
        .await?;
    for session in sessions {
        crate::modules::auth_v1::controller::terminate_session(&state, &auth, session.id).await?;
    }
    let now = crate::services::job_log::epoch_now();
    let out = data_request::erase_self(&state, user.id, now)
        .await
        .map_err(data_request_error)?;
    if let Err(err) = auth.logout().await {
        warn!(error = ?err, "self delete: logout of current session failed");
    }
    if let Err(err) = auth.session().flush().await {
        warn!(error = ?err, "self delete: failed to flush current session");
    }
    info!("Self-service account deletion answered");
    Ok((
        StatusCode::OK,
        crate::middlewares::auth_marker::clear_headers(state.settings.http.cookie_secure),
        Json(json!({
            "receipt_id": out.receipt_id,
            "deleted": out.deleted,
            "retained_tables": out.retained_tables,
            "user_row_erased": out.user_row_erased,
            "media_deleted": out.media_deleted,
            "posts_anonymized": out.posts_anonymized,
        })),
    ))
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id))]
pub async fn admin_update(
    auth: AuthSession,
    state: State<AppState>,
    Path(user_id): Path<i32>,
    payload: ValidatedJson<V1AdminUpdateUserPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: requested role can't exceed the caller's, and the target must be strictly lower-rank.
    let caller_level = auth.user.as_ref().map(|u| u.role.to_i32()).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;

    if let Some(role_str) = payload.0.role.as_deref() {
        let requested = UserRole::from_str(role_str).map_err(|_| {
            ErrorResponse::new(ErrorCode::InvalidInput).with_message("Invalid role")
        })?;
        if requested.to_i32() > caller_level {
            warn!(
                caller_level,
                requested = role_str,
                "Admin attempted to assign a role above their own"
            );
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("You cannot assign a role higher than your own"));
        }
    }

    if let Some(target) = User::get_by_id(&state.sea_db, user_id).await? {
        if target.role.to_i32() >= caller_level {
            warn!(
                caller_level,
                target_level = target.role.to_i32(),
                "Admin attempted to modify an equal/higher-role user"
            );
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("You cannot modify a user at or above your own role"));
        }
    }

    let payload = payload.0.into_update_user();
    match User::admin_update(
        &state.sea_db,
        &state.storage.config.public_url,
        user_id,
        payload,
    )
    .await
    {
        Ok(Some(user)) => {
            info!(user_id, "Admin updated user");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        Ok(None) => {
            warn!(user_id, "Admin tried to update non-existent user");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("No user with this ID exists"))
        }
        Err(err) => {
            error!(user_id, "Admin failed to update user: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(auth, state, payload), fields(user_id))]
pub async fn admin_change_password(
    auth: AuthSession,
    state: State<AppState>,
    Path(user_id): Path<i32>,
    payload: ValidatedJson<AdminChangePassword>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Route guard only checks ROLE_ADMIN: resetting a superior's password would be an account takeover.
    let caller_level = auth.user.as_ref().map(|u| u.role.to_i32()).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    if let Some(target) = User::get_by_id(&state.sea_db, user_id).await? {
        if target.role.to_i32() >= caller_level {
            warn!(
                caller_level,
                target_level = target.role.to_i32(),
                "Admin attempted to reset password of an equal/higher-role user"
            );
            return Err(
                ErrorResponse::new(ErrorCode::OperationNotAllowed).with_message(
                    "You cannot reset the password of a user at or above your own role",
                ),
            );
        }
    }
    User::change_password(&state.sea_db, user_id, payload.0.password).await?;
    info!(user_id, "Admin changed user password");
    Ok((
        StatusCode::OK,
        Json(json!({ "message": "Password changed successfully" })),
    ))
}

#[debug_handler]
#[instrument(skip(state, payload))]
pub async fn admin_list(
    state: State<AppState>,
    payload: ValidatedJson<V1AdminUserQueryParams>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let query = payload.0.into_user_query();
    let page = query.page.unwrap_or(1);

    let result = User::admin_list(&state.sea_db, &state.storage.config.public_url, query).await?;
    let users = result.data;
    let total = result.total;
    info!(total, page, "Admin listed users");
    Ok((
        StatusCode::OK,
        Json(json!({
            "data": users,
            "total": total,
            "per_page": User::PER_PAGE,
            "page": page,
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state), fields(user_id))]
pub async fn admin_view(
    state: State<AppState>,
    Path(user_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    match User::find_by_id_with_relations(&state.sea_db, &state.storage.config.public_url, user_id)
        .await
    {
        Ok(user) => {
            info!(user_id, "Admin viewed user");
            Ok((StatusCode::OK, Json(json!(user))))
        }
        Err(err) => {
            error!(user_id, "Admin failed to view user: {}", err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_identity_providers_are_the_three_linkable_ones() {
        assert!(is_supported_identity_provider("github"));
        assert!(is_supported_identity_provider("discord"));
        assert!(is_supported_identity_provider("google"));
        assert!(!is_supported_identity_provider("facebook"));
        assert!(!is_supported_identity_provider("apple"));
        assert!(!is_supported_identity_provider(""));
    }

    #[test]
    fn linkable_providers_are_ready_supported_and_unlinked() {
        let configured = vec![
            ProviderStatus {
                name: "github".to_string(),
                ready: true,
            },
            ProviderStatus {
                name: "discord".to_string(),
                ready: false,
            },
            ProviderStatus {
                name: "google".to_string(),
                ready: true,
            },
            ProviderStatus {
                name: "facebook".to_string(),
                ready: true,
            },
            ProviderStatus {
                name: "apple".to_string(),
                ready: true,
            },
        ];

        let linkable = linkable_providers(&configured, &["github"]);
        assert_eq!(
            linkable,
            vec!["google"],
            "only ready, supported, not-yet-linked providers are offered"
        );
    }
}
