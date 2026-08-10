use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_macros::debug_handler;

use axum_client_ip::ClientIp;

use rux_auth::AuthBackend as AuthBackendTrait;
use sea_orm::{ActiveModelTrait, EntityTrait};
use serde_json::json;
use tracing::{error, info, instrument, warn};

use crate::{
    db::sea_models::{email_verification, user, user_session},
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    modules::auth_v1::validator::{
        V1LoginPayload, V1LoginTotpPayload, V1RegisterPayload, V1TwoFADisablePayload,
        V1TwoFAVerifyPayload,
    },
    services::{abuse_limiter, auth::AuthSession, mail::send_email_verification_code},
    utils::twofa,
    AppState,
};

const ABUSE_LIMITER_CONFIG: abuse_limiter::AbuseLimiterConfig = abuse_limiter::AbuseLimiterConfig {
    temp_block_attempts: 3,
    temp_block_range: 360,
    temp_block_duration: 3600,
    block_retry_limit: 5,
    block_range: 900,
    block_duration: 86400,
};

#[debug_handler(state = AppState)]
#[instrument(skip(auth), fields(user_id))]
pub async fn log_out(mut auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    if let Some(user) = &auth.user {
        tracing::Span::current().record("user_id", user.id);
        info!(user_id = user.id, "User logging out");
    }

    match auth.logout().await {
        Ok(_) => {
            info!("Logout successful");
            Ok((StatusCode::OK, Json(json!({"message": "Logged out"}))))
        }
        Err(e) => {
            error!(error = %e, "Logout failed");
            Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("An error occurred while logging out"))
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(client_ip = %secure_ip, user_id, user_role, result))]
pub async fn log_in(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ClientIp(secure_ip): ClientIp,
    headers: HeaderMap,
    payload: ValidatedJson<V1LoginPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!(client_ip = %secure_ip, "Login attempt");

    let payload = payload.0;

    // AUTH-BF-1: per-account throttle on the password step; the per-IP cap can't stop an attacker rotating IPs to grind one account. Keyed on normalized email; fail-closed.
    let login_key = payload.email.trim().to_lowercase();
    abuse_limiter::limiter(
        &state.gate_store,
        &format!("login:{login_key}"),
        ABUSE_LIMITER_CONFIG,
    )
    .await?;

    let user = auth
        .backend()
        .authenticate_password(payload.email, payload.password)
        .await;

    match user {
        Ok(Some(user)) => {
            tracing::Span::current().record("user_id", user.id);
            tracing::Span::current().record("user_role", user.role.to_string());

            // Fail closed: a ban-lookup error must not grant access to a banned user.
            match auth.backend().check_ban(&user.id).await {
                Ok(ban_status) if ban_status.is_banned() => {
                    warn!(user_id = user.id, "Banned user attempted login");
                    tracing::Span::current().record("result", "banned");
                    return Err(ErrorResponse::new(ErrorCode::AccountLocked)
                        .with_message("This account has been banned"));
                }
                Ok(_) => {}
                Err(_) => {
                    tracing::Span::current().record("result", "ban_check_error");
                    return Err(ErrorResponse::new(ErrorCode::InternalServerError)
                        .with_message("Unable to verify account status"));
                }
            }

            if user.two_fa_enabled {
                tracing::Span::current().record("result", "totp_required");
                let totp_token = login_totp_token::mint(user.id).await?;
                info!(user_id = user.id, "Login requires TOTP (2FA enrolled)");
                return Ok((
                    StatusCode::OK,
                    Json(json!({
                        "status": "totp_required",
                        "totp_token": totp_token,
                    })),
                ));
            }

            let ip = Some(secure_ip.to_string());
            let device = headers
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            match auth
                .login_with_metadata(&user, device.clone(), ip.clone())
                .await
            {
                Ok(_) => {
                    info!(
                        user_id = user.id,
                        user_role = user.role.to_string(),
                        device = ?device,
                        "Login successful"
                    );

                    let session_row = user_session::Entity::create(
                        &state.sea_db,
                        user_session::NewUserSession::new(user.id, device, ip),
                    )
                    .await
                    .ok();

                    // V-HIGH-2: record the sid_map so sessions_terminate can DEL the live tower-session record; revoked_at is audit-only, else the cookie authenticates until its 14-day expiry.
                    if (auth.session().save().await).is_ok() {
                        if let (Some(row), Some(tower_sid)) =
                            (session_row.as_ref(), auth.session().id())
                        {
                            record_session_mapping(row.id, &tower_sid.to_string());
                        }
                    }

                    tracing::Span::current().record("result", "success");
                    Ok((StatusCode::OK, Json(json!(user))))
                }
                Err(err) => {
                    error!(error = %err, user_id = user.id, "Session creation failed");
                    tracing::Span::current().record("result", "session_error");
                    Err(ErrorResponse::new(ErrorCode::InternalServerError)
                        .with_message("An error occurred while logging in")
                        .with_details(err.to_string()))
                }
            }
        }
        Ok(None) => {
            warn!(client_ip = %secure_ip, "Invalid credentials");
            tracing::Span::current().record("result", "invalid_credentials");
            Err(ErrorResponse::new(ErrorCode::InvalidCredentials))
        }
        Err(err) => {
            error!(error = ?err, client_ip = %secure_ip, "Authentication error");
            tracing::Span::current().record("result", "auth_error");
            Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Authentication error"))
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn login_totp(
    State(state): State<AppState>,
    mut auth: AuthSession,
    payload: ValidatedJson<V1LoginTotpPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let payload = payload.0;

    let user_id = match login_totp_token::take(&payload.totp_token).await? {
        Some(id) => id,
        None => {
            warn!("login/totp: pending token missing/expired/replayed");
            return Err(ErrorResponse::new(ErrorCode::Unauthorized)
                .with_message("Invalid or expired login session"));
        }
    };
    tracing::Span::current().record("user_id", user_id);

    let user = match auth.backend().get_user(&user_id).await {
        Ok(Some(u)) => u,
        _ => {
            warn!(user_id, "login/totp: user not found after valid token");
            return Err(ErrorResponse::new(ErrorCode::Unauthorized)
                .with_message("Invalid or expired login session"));
        }
    };

    // Fail-closed ban re-check between the two steps.
    match auth.backend().check_ban(&user.id).await {
        Ok(ban_status) if ban_status.is_banned() => {
            warn!(user_id = user.id, "login/totp: banned user");
            return Err(ErrorResponse::new(ErrorCode::AccountLocked)
                .with_message("This account has been banned"));
        }
        Ok(_) => {}
        Err(_) => {
            return Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Unable to verify account status"));
        }
    }

    // Fail-closed per-account TOTP brute-force throttle.
    let key_prefix = format!("totp:{}", user.id);
    abuse_limiter::limiter(&state.gate_store, &key_prefix, ABUSE_LIMITER_CONFIG).await?;

    // CRYP-2FA-002: secret is encrypted at rest; a decrypt failure must fail closed (reject), never verify against the opaque envelope bytes.
    let secret = match user.two_fa_secret_plain() {
        Ok(Some(s)) => s,
        Ok(None) => {
            warn!(
                user_id = user.id,
                "login/totp: 2FA enabled but no secret stored"
            );
            return Err(ErrorResponse::new(ErrorCode::Unauthorized)
                .with_message("Invalid or expired login session"));
        }
        Err(e) => {
            warn!(user_id = user.id, error = %e, "login/totp: TOTP secret unreadable");
            return Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Unable to verify second factor"));
        }
    };

    let matched = twofa::verify_totp_code_now(&secret, &payload.code);
    let matched = match matched {
        Some(m) => m,
        None => {
            warn!(user_id = user.id, "login/totp: invalid TOTP code");
            return Err(
                ErrorResponse::new(ErrorCode::Unauthorized).with_message("Invalid 2FA code")
            );
        }
    };

    // Replay gate: keep both — is_fresh_counter is the fast-path reject and advance_totp_counter_if_higher (atomic conditional UPDATE) is the authoritative gate closing the TOCTOU race across login/verify/disable.
    if !twofa::is_fresh_counter(matched, user.two_fa_last_totp_counter) {
        warn!(
            user_id = user.id,
            matched_counter = matched,
            "login/totp: TOTP code replayed (counter already used)"
        );
        return Err(ErrorResponse::new(ErrorCode::Unauthorized).with_message("Invalid 2FA code"));
    }
    let advanced =
        user::Entity::advance_totp_counter_if_higher(&state.sea_db, user.id, matched).await?;
    if !advanced {
        warn!(
            user_id = user.id,
            matched_counter = matched,
            "login/totp: TOTP lost the watermark race (concurrent advance); rejecting as replay"
        );
        return Err(ErrorResponse::new(ErrorCode::Unauthorized).with_message("Invalid 2FA code"));
    }

    let ip: Option<String> = None;
    let device: Option<String> = None;
    match auth
        .login_with_metadata(&user, device.clone(), ip.clone())
        .await
    {
        Ok(_) => {
            info!(
                user_id = user.id,
                "login/totp: TOTP verified, full session issued"
            );
            tracing::Span::current().record("result", "success");

            let session_row = user_session::Entity::create(
                &state.sea_db,
                user_session::NewUserSession::new(user.id, device, ip),
            )
            .await
            .ok();

            // V-HIGH-2: record sid_map (same as password-login path).
            if (auth.session().save().await).is_ok() {
                if let (Some(row), Some(tower_sid)) = (session_row.as_ref(), auth.session().id()) {
                    record_session_mapping(row.id, &tower_sid.to_string());
                }
            }

            Ok((StatusCode::OK, Json(json!(user))))
        }
        Err(err) => {
            error!(error = %err, user_id = user.id, "login/totp: session creation failed");
            tracing::Span::current().record("result", "session_error");
            Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("An error occurred while logging in")
                .with_details(err.to_string()))
        }
    }
}

#[debug_handler]
#[instrument(skip(state, payload), fields(user_id, result))]
pub async fn register(
    State(state): State<AppState>,
    payload: ValidatedJson<V1RegisterPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let payload = payload.0;

    info!("User registration attempt");

    let email = payload.email.clone();

    // Store only the keyed hash; the plaintext code is emailed, never persisted.
    let code = email_verification::Entity::generate_code();
    let code_hash = crate::utils::code_hash::hash_code(&state.secret_key, &code);

    match user::Entity::create(&state.sea_db, payload.into_new_user(), code_hash).await {
        Ok(user) => {
            info!(user_id = user.id, email = %user.email, "User registered successfully");
            tracing::Span::current().record("user_id", user.id);
            tracing::Span::current().record("result", "success");

            let app_state = state.clone();
            let user_id = user.id;
            let email_for_task = email.clone();
            let code_for_task = code.clone();
            tokio::spawn(async move {
                if let Err(err) =
                    send_email_verification_code(&app_state.mailer, &email_for_task, &code_for_task)
                        .await
                {
                    tracing::error!(
                        user_id,
                        email = %email_for_task,
                        "Failed to send verification email: {}",
                        err
                    );
                }
            });

            Ok((StatusCode::CREATED, Json(json!(user))))
        }
        Err(err) => {
            warn!(error = ?err, "Registration failed");
            tracing::Span::current().record("result", "failure");
            // Do not echo the raw SeaORM error: a unique-violation would leak that the email is already registered (enumeration oracle).
            Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Registration could not be completed at this time"))
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id))]
pub async fn twofa_setup(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    tracing::Span::current().record("user_id", user.id);

    info!(user_id = user.id, "2FA setup initiated");

    let secret_b32 = twofa::generate_secret_base32(20).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to generate 2FA secret")
    })?;
    let otpauth_url = twofa::build_otpauth_url(
        &user.email,
        "Ruxlog",
        &secret_b32,
        twofa::DEFAULT_TOTP_DIGITS,
    );

    let backup_codes = twofa::generate_backup_codes(10).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to generate backup codes")
    })?;
    let backup_hashes = {
        let codes_for_hash = backup_codes.clone();
        tokio::task::spawn_blocking(move || twofa::hash_backup_codes(&codes_for_hash))
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError)
                    .with_message(format!("Backup code hashing failed: {e}"))
            })?
    };
    let backup_hashes_json = serde_json::json!(backup_hashes);

    let existing = user::Entity::find_by_id_with_404(&state.sea_db, user.id).await?;
    let mut active: user::ActiveModel = existing.into();
    active.two_fa_enabled = sea_orm::Set(false);
    active.two_fa_secret = sea_orm::Set(Some(secret_b32.clone()));
    active.two_fa_backup_codes = sea_orm::Set(Some(backup_hashes_json));
    active.updated_at = sea_orm::Set(chrono::Utc::now().fixed_offset());
    active.update(&state.sea_db).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "secret": secret_b32,
            "otpauth_url": otpauth_url,
            "backup_codes": backup_codes,
        })),
    ))
}

#[debug_handler]
pub async fn twofa_verify(
    State(state): State<AppState>,
    mut auth: AuthSession,
    payload: ValidatedJson<V1TwoFAVerifyPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    let payload = payload.0;

    // Fail-closed per-account TOTP brute-force throttle.
    let key_prefix = format!("totp:{}", user.id);
    abuse_limiter::limiter(&state.gate_store, &key_prefix, ABUSE_LIMITER_CONFIG).await?;

    let existing = user::Entity::find_by_id_with_404(&state.sea_db, user.id).await?;
    // CRYP-2FA-002: decrypt the at-rest secret via the accessor; fail closed on error.
    let secret = match existing.two_fa_secret_plain() {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("2FA not initialized"))
        }
        Err(e) => {
            warn!(user_id = existing.id, error = %e, "2fa/verify: TOTP secret unreadable");
            return Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Unable to verify second factor"));
        }
    };

    // V-MED-6: keep both checks — is_fresh_counter (fast-path reject) and advance_totp_counter_if_higher (authoritative atomic gate closing the TOCTOU race); removing either reopens replay.
    let totp_counter = twofa::verify_totp_code_now(&secret, &payload.code);

    if let Some(matched) = totp_counter {
        if twofa::is_fresh_counter(matched, existing.two_fa_last_totp_counter) {
            let advanced =
                user::Entity::advance_totp_counter_if_higher(&state.sea_db, user.id, matched)
                    .await?;
            if !advanced {
                warn!(
                    user_id = user.id,
                    matched_counter = matched,
                    "TOTP code lost the watermark race (concurrent advance); rejecting as replay"
                );
                return Err(
                    ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid 2FA code")
                );
            }
            // Counter already persisted by the atomic gate above; keep Unchanged — a Set here could clobber a concurrent advance and reopen replay.
            let mut active: user::ActiveModel = existing.into();
            active.two_fa_enabled = sea_orm::Set(true);
            active.two_fa_last_totp_counter = sea_orm::Unchanged(Some(matched));
            active.updated_at = sea_orm::Set(chrono::Utc::now().fixed_offset());
            let updated = active.update(&state.sea_db).await?;
            rotate_session_after_trust_change(&mut auth).await;
            return Ok((StatusCode::OK, Json(json!(updated))));
        } else {
            warn!(
                user_id = user.id,
                matched_counter = matched,
                "TOTP code replayed (counter already used); rejecting"
            );
            return Err(
                ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid 2FA code")
            );
        }
    }

    if let Some(backup_code) = payload.backup_code {
        if let Some(stored) = &existing.two_fa_backup_codes {
            let stored_vec: Vec<String> =
                serde_json::from_value(stored.clone()).unwrap_or_else(|_| vec![]);
            let consume_result = {
                let stored_clone = stored_vec;
                let code_clone = backup_code.clone();
                tokio::task::spawn_blocking(move || {
                    twofa::consume_backup_code(&stored_clone, &code_clone)
                })
                .await
                .map_err(|e| {
                    ErrorResponse::new(ErrorCode::InternalServerError)
                        .with_message(format!("Backup code verification failed: {e}"))
                })?
            };
            if let Some(updated_hashes) = consume_result {
                let mut active: user::ActiveModel = existing.into();
                active.two_fa_enabled = sea_orm::Set(true);
                active.two_fa_backup_codes = sea_orm::Set(Some(serde_json::json!(updated_hashes)));
                active.updated_at = sea_orm::Set(chrono::Utc::now().fixed_offset());
                let updated = active.update(&state.sea_db).await?;
                rotate_session_after_trust_change(&mut auth).await;
                return Ok((StatusCode::OK, Json(json!(updated))));
            }
        }
    }

    Err(ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid 2FA code"))
}

#[debug_handler]
pub async fn twofa_disable(
    State(state): State<AppState>,
    mut auth: AuthSession,
    payload: ValidatedJson<V1TwoFADisablePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    let payload = payload.0;

    // Fail-closed per-account TOTP brute-force throttle.
    let key_prefix = format!("totp:{}", user.id);
    abuse_limiter::limiter(&state.gate_store, &key_prefix, ABUSE_LIMITER_CONFIG).await?;

    let existing = user::Entity::find_by_id_with_404(&state.sea_db, user.id).await?;

    // V-MED-6: advance_totp_counter_if_higher is the authoritative replay gate; totp_authorized records success so the final UPDATE leaves the counter Unchanged (no racy re-write); backup-code path skips it.
    let mut totp_authorized = false;

    if existing.two_fa_enabled {
        if let Some(code) = payload.code.clone() {
            // CRYP-2FA-002: decrypt failure → empty secret (falls through to backup code); never grant disable without a valid second factor.
            let secret = match existing.two_fa_secret_plain() {
                Ok(Some(s)) => s,
                Ok(None) => String::new(),
                Err(e) => {
                    warn!(user_id = existing.id, error = %e, "2fa/disable: TOTP secret unreadable");
                    String::new()
                }
            };
            let totp_matched = if secret.is_empty() {
                None
            } else {
                twofa::verify_totp_code_now(&secret, &code)
            };
            let totp_fresh = match totp_matched {
                Some(matched) => {
                    twofa::is_fresh_counter(matched, existing.two_fa_last_totp_counter)
                }
                None => false,
            };

            if totp_fresh {
                let matched = totp_matched.expect("totp_matched is Some when totp_fresh");
                let advanced =
                    user::Entity::advance_totp_counter_if_higher(&state.sea_db, user.id, matched)
                        .await?;
                if !advanced {
                    warn!(
                        user_id = user.id,
                        matched_counter = matched,
                        "TOTP disable lost the watermark race (concurrent advance); rejecting as replay"
                    );
                    return Err(ErrorResponse::new(ErrorCode::InvalidToken)
                        .with_message("Invalid 2FA or backup code"));
                }
                totp_authorized = true;
            }

            let mut backup_ok = false;
            if !totp_authorized {
                if let Some(stored) = &existing.two_fa_backup_codes {
                    let stored_vec: Vec<String> =
                        serde_json::from_value(stored.clone()).unwrap_or_else(|_| vec![]);
                    backup_ok = {
                        let stored_clone = stored_vec;
                        let code_clone = code.clone();
                        tokio::task::spawn_blocking(move || {
                            twofa::consume_backup_code(&stored_clone, &code_clone).is_some()
                        })
                        .await
                        .map_err(|e| {
                            ErrorResponse::new(ErrorCode::InternalServerError)
                                .with_message(format!("Backup code verification failed: {e}"))
                        })?
                    };
                }
            }

            if !totp_authorized && !backup_ok {
                return Err(ErrorResponse::new(ErrorCode::InvalidToken)
                    .with_message("Invalid 2FA or backup code"));
            }
        } else {
            return Err(ErrorResponse::new(ErrorCode::MissingRequiredField)
                .with_message("code is required"));
        }
    }

    let last_counter = existing.two_fa_last_totp_counter;
    let mut active: user::ActiveModel = existing.into();
    active.two_fa_enabled = sea_orm::Set(false);
    active.two_fa_secret = sea_orm::Set(None);
    active.two_fa_backup_codes = sea_orm::Set(None);
    active.two_fa_last_totp_counter = sea_orm::Unchanged(last_counter);
    active.updated_at = sea_orm::Set(chrono::Utc::now().fixed_offset());
    let updated = active.update(&state.sea_db).await?;

    rotate_session_after_trust_change(&mut auth).await;

    Ok((StatusCode::OK, Json(json!(updated))))
}

#[debug_handler]
pub async fn sessions_list(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    let page = 1;

    match user_session::Entity::list_by_user(&state.sea_db, user.id, Some(page)).await {
        Ok(result) => Ok((
            StatusCode::OK,
            Json(json!({
                "data": result.data,
                "total": result.total,
                "page": page,
            })),
        )),
        Err(err) => Err(err),
    }
}

#[debug_handler]
pub async fn sessions_terminate(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user_id = auth.user.as_ref().map(|u| u.id).unwrap_or(0);

    // V-LOW-IDOR: ownership check must precede any write (earlier revoke-first mutated the victim row before the 401); resolve, reject on mismatch, then revoke.
    let _existing = match user_session::Entity::find_by_id(id)
        .one(&state.sea_db)
        .await
    {
        Ok(Some(model)) if model.user_id == user_id => model,
        Ok(Some(_)) => return Err(ErrorResponse::new(ErrorCode::Unauthorized)),
        Ok(None) => return Err(ErrorResponse::new(ErrorCode::RecordNotFound)),
        Err(err) => return Err(err.into()),
    };
    user_session::Entity::revoke(&state.sea_db, id).await?;

    {
        // V-HIGH-2: revoke only stamps revoked_at (audit-only); the live tower-session record must also be deleted or the cookie authenticates until its 14-day expiry.
        if let Some(tower_sid) = lookup_session_mapping(id) {
            auth.backend().terminate(&tower_sid).await;
        } else {
            warn!(
                session_id = id,
                "No tower-session mapping for session; live record could not be DEL'd — cookie valid until 14-day expiry (revoked_at is audit-only)"
            );
        }
        Ok((
            StatusCode::OK,
            Json(json!({ "message": "Session terminated" })),
        ))
    }
}

pub(crate) use crate::services::auth::{lookup_session_mapping, record_session_mapping};

/// F#16: rotate the session id at trust transitions (2FA on/off) so the per-session CSRF token rebinds; a rotation failure is logged non-fatal (trust change already succeeded) — surfacing a 500 would mislead the client.
async fn rotate_session_after_trust_change(auth: &mut AuthSession) {
    if let Err(err) = auth.session().cycle_id().await {
        warn!(error = %err, "F#16: failed to re-rotate session id at trust transition; CSRF token NOT rebound");
        return;
    }
    if let Err(err) = auth.session().save().await {
        warn!(error = %err, "F#16: failed to persist rotated session id at trust transition");
    }
}

mod login_totp_token {
    use super::*;
    use rand::Rng;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use zeroize::Zeroize;

    const TTL_SECS: u64 = 300;

    #[doc(hidden)]
    #[allow(dead_code)] // referenced via type_name::<AssertWired> only (compile-time wiring check)
    pub struct AssertWired;

    type TokenMap = HashMap<String, (i32, Instant)>;

    static TOKENS: OnceLock<Mutex<TokenMap>> = OnceLock::new();

    fn tokens() -> &'static Mutex<TokenMap> {
        TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn reap_stale(map: &mut TokenMap) {
        map.retain(|_, (_, at)| at.elapsed().as_secs() < TTL_SECS);
    }

    fn namespaced_key(token: &str) -> String {
        format!("auth:login_totp:{token}")
    }

    pub async fn mint(user_id: i32) -> Result<String, ErrorResponse> {
        let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
        rand::rng().fill(bytes.as_mut());
        let token = hex::encode(*bytes);
        bytes.zeroize();

        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "login_totp token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        reap_stale(&mut map);
        map.insert(namespaced_key(&token), (user_id, Instant::now()));
        Ok(token)
    }

    pub async fn take(token: &str) -> Result<Option<i32>, ErrorResponse> {
        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "login_totp token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        reap_stale(&mut map);
        Ok(map.remove(&namespaced_key(token)).map(|(id, _)| id))
    }
}

#[cfg(test)]
mod tests {
    use super::login_totp_token;
    use crate::services::auth::session_mapping_key;

    #[test]
    fn session_mapping_key_is_stable_and_namespaced() {
        assert_eq!(session_mapping_key(1), "rux:sid_map:1");
        assert_eq!(session_mapping_key(42), "rux:sid_map:42");
        assert_eq!(
            session_mapping_key(7),
            format!("rux:sid_map:{}", 7),
            "key must be the pg id under the rux namespace"
        );
    }

    #[test]
    fn login_totp_redis_key_prefix_is_stable() {
        for token in ["deadbeef", "abc123", "z"] {
            let expected = format!("auth:login_totp:{token}");
            assert!(
                expected.starts_with("auth:login_totp:"),
                "pending-TOTP key must live under the auth:login_totp: namespace"
            );
        }
        let _ = std::any::type_name::<login_totp_token::AssertWired>;
    }
}
