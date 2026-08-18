use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_client_ip::ClientIp;
use axum_macros::debug_handler;

use serde_json::json;
use tracing::{error, info, instrument, warn};

use crate::{
    db::sea_models::{forgot_password, user},
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::{
        abuse_limiter,
        mail::{mail_error_kind, mail_error_to_response, send_forgot_password_email, MailError},
    },
    AppState,
};

use super::validator::{V1GeneratePayload, V1ResetPayload, V1VerifyPayload, V1VerifyResponse};

const ABUSE_LIMITER_CONFIG: abuse_limiter::AbuseLimiterConfig = abuse_limiter::AbuseLimiterConfig {
    temp_block_attempts: 3,
    temp_block_range: 360,
    temp_block_duration: 3600,
    block_retry_limit: 5,
    block_range: 900,
    block_duration: 86400,
};

/// Single-use reset tokens minted by `verify`, consumed by `reset`; backed by an in-memory TTL map (a restart drops outstanding tokens).
mod reset_token {
    use super::*;
    use rand::Rng;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use zeroize::Zeroize;

    const TTL_SECS: u64 = 600;

    type TokenMap = HashMap<String, (i32, Instant)>;

    static TOKENS: OnceLock<Mutex<TokenMap>> = OnceLock::new();

    fn tokens() -> &'static Mutex<TokenMap> {
        TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn reap_stale(map: &mut TokenMap) {
        map.retain(|_, (_, at)| at.elapsed().as_secs() < TTL_SECS);
    }

    fn namespaced_key(token: &str) -> String {
        format!("forgot_password:reset_token:{token}")
    }

    pub async fn mint(user_id: i32) -> Result<String, ErrorResponse> {
        let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
        rand::rng().fill(bytes.as_mut());
        let token = hex::encode(*bytes);
        bytes.zeroize();

        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "reset_token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        reap_stale(&mut map);
        map.insert(namespaced_key(&token), (user_id, Instant::now()));
        Ok(token)
    }

    /// Removal on take is required for single-use semantics — never a read (`get`), or a replayed `reset` could reuse the token.
    pub async fn take(token: &str) -> Result<Option<i32>, ErrorResponse> {
        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "reset_token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        reap_stale(&mut map);
        Ok(map.remove(&namespaced_key(token)).map(|(id, _)| id))
    }
}

/// Shared by every `generate` exit (unknown email, in-delay, mail throttled, sent) so the response leaks no account existence (SC-006); do not diverge them.
pub(crate) fn uniform_success_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "message": "If an account exists for that email, a password reset link has been sent.",
        })),
    )
}

/// Fixed input for the dummy Argon2 hash on the unknown-email branch; constant length keeps per-request CPU cost fixed (not dead — see equalize_unknown_email_work).
const DUMMY_HASH_PASSWORD: &str = "timing-equalization-dummy";

/// Closes the timing oracle between `generate`'s branches: the unknown-email path runs this Argon2id hash to match the known-email path's CPU cost (result discarded by caller).
fn equalize_unknown_email_work() -> String {
    password_auth::generate_hash(DUMMY_HASH_PASSWORD)
}

#[debug_handler]
#[instrument(skip(state, payload), fields(client_ip = %secure_ip))]
pub async fn generate(
    state: State<AppState>,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1GeneratePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Run before the existence check so email-probing is throttled too.
    let ip = secure_ip.to_string();
    let key_prefix = format!("forgot_password:{}", ip);
    match abuse_limiter::limiter(&state.gate_store, &key_prefix, ABUSE_LIMITER_CONFIG).await {
        Ok(_) => (),
        Err(err) => {
            warn!("Abuse limiter blocked forgot password request");
            return Err(err);
        }
    }

    let pool = &state.sea_db;
    // Unknown email: return the same response as a known one (SC-006); never 404.
    let user = match user::Entity::find_by_email(pool, payload.email.clone()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Forgot password requested for non-existent email; returning uniform response");
            let _ = tokio::task::spawn_blocking(equalize_unknown_email_work)
                .await
                .map_err(|e| {
                    error!("Dummy equalization hash task panicked: {e}");
                    ErrorResponse::new(ErrorCode::InternalServerError)
                })?;
            return Ok(uniform_success_response());
        }
        Err(err) => {
            error!("Database error finding user: {}", err);
            return Err(err);
        }
    };
    let user_id = user.id;

    match forgot_password::Entity::find_query(pool, Some(user_id), None, None).await {
        Ok(verification) => {
            if verification.is_in_delay() {
                // Delay stays enforced (no new code, no email) but the response must stay uniform — a distinct status here would confirm the account exists.
                warn!(user_id, "Forgot password in delay period");
                return Ok(uniform_success_response());
            }
        }
        Err(err) => {
            if err.code != ErrorCode::InvalidInput {
                error!(user_id, "Error checking forgot password delay: {}", err);
                return Err(err);
            }
        }
    }

    // Store only the keyed hash; the plaintext is emailed but never persisted.
    let code = forgot_password::Entity::generate_code();
    let code_hash = crate::utils::code_hash::hash_code(&state.secret_key, &code);
    if let Err(err) = forgot_password::Entity::regenerate(pool, user_id, code_hash).await {
        error!(user_id, "Failed to store forgot-password code: {}", err);
        return Err(err);
    }
    if let Err(err) = send_forgot_password_email(&state.mailer, &payload.email, &code).await {
        // The per-recipient transactional cap must not turn into an account-existence oracle — throttling returns the uniform envelope too.
        if matches!(err, MailError::Throttled { .. }) {
            warn!(
                user_id,
                error_kind = mail_error_kind(&err),
                "Forgot password email throttled; returning uniform response"
            );
            return Ok(uniform_success_response());
        }
        error!(
            user_id,
            error_kind = mail_error_kind(&err),
            "Failed to send forgot password email"
        );
        return Err(mail_error_to_response(&err));
    }

    info!(user_id, "Recovery email sent");
    Ok(uniform_success_response())
}

#[debug_handler]
#[instrument(skip(state, payload), fields(client_ip = %secure_ip))]
pub async fn verify(
    state: State<AppState>,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1VerifyPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let key_prefix = format!("forgot_password_verify:{}", secure_ip);
    abuse_limiter::limiter(&state.gate_store, &key_prefix, ABUSE_LIMITER_CONFIG).await?;

    let code_hash = crate::utils::code_hash::hash_code(&state.secret_key, &payload.code);
    let result = forgot_password::Entity::find_query(
        &state.sea_db,
        None,
        Some(&payload.email),
        Some(&code_hash),
    )
    .await;

    let verification = match result {
        Ok(verification) => {
            if verification.is_expired() {
                warn!("Forgot password code expired");
                return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                    .with_message("The verification code has expired"));
            }
            verification
        }
        Err(err) => {
            warn!("Invalid forgot password code");
            return Err(err);
        }
    };
    let user_id = verification.user_id;

    if let Err(err) = forgot_password::Entity::consume_code(&state.sea_db, user_id).await {
        error!(user_id, "Failed to consume forgot-password code: {}", err);
        return Err(err);
    }

    let reset_token = reset_token::mint(user_id).await?;

    info!(
        user_id,
        "Forgot password code verified and consumed; reset token issued"
    );
    Ok((StatusCode::OK, Json(V1VerifyResponse { reset_token })))
}

#[debug_handler]
#[instrument(skip(state, payload), fields(client_ip = %secure_ip))]
pub async fn reset(
    state: State<AppState>,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1ResetPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Intentionally shares the verify bucket — do not rename to a reset-specific key.
    let key_prefix = format!("forgot_password_verify:{}", secure_ip);
    abuse_limiter::limiter(&state.gate_store, &key_prefix, ABUSE_LIMITER_CONFIG).await?;

    if payload.password != payload.confirm_password {
        warn!("Password mismatch");
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Password and confirm password do not match"));
    }

    // Password change requires the single-use reset_token; never add a code+email fallback — that would let an email-interceptor skip /verify.
    let user_id = match reset_token::take(&payload.reset_token).await? {
        Some(id) => id,
        None => {
            warn!("Reset attempted with an unknown or already-used reset token");
            return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("Reset token is invalid or has expired"));
        }
    };

    match forgot_password::Entity::reset(&state.sea_db, user_id, payload.password.clone()).await {
        Ok(_) => {
            info!(user_id, "Password reset in database");
            Ok((
                StatusCode::OK,
                Json(json!({
                    "message": "Password reset successfully",
                })),
            ))
        }
        Err(err) => {
            error!(user_id, "Failed to reset password: {}", err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_exits_stay_on_the_uniform_envelope() {
        // Guard only the handler source — this test's own text must not satisfy the asserts.
        let src = include_str!("controller.rs");
        let (code, _) = src
            .split_once("#[cfg(test)]")
            .expect("tests module present");
        assert!(
            !code.contains("ErrorCode::TooManyAttempts"),
            "generate must not answer with a direct 429 — the in-delay branch and every other exit share the uniform envelope; IP-scoped throttling stays in the abuse limiter"
        );
        assert!(
            code.contains("MailError::Throttled"),
            "a Throttled send must fall back to the uniform envelope or the transactional mail cap becomes an account-existence oracle"
        );
    }

    #[test]
    fn uniform_success_response_is_stable_and_non_leaking() {
        let (status_known, body_known) = uniform_success_response();
        let (status_unknown, body_unknown) = uniform_success_response();

        assert_eq!(status_known, status_unknown);
        assert_eq!(status_known, StatusCode::OK);

        let known = serde_json::to_value(&*body_known).unwrap();
        let unknown = serde_json::to_value(&*body_unknown).unwrap();
        assert_eq!(known, unknown);

        let msg = known["message"].as_str().unwrap().to_lowercase();
        assert!(
            msg.contains("if an account exists"),
            "uniform message must be conditional, got: {msg}"
        );
        assert!(
            !msg.contains("doesn't exist") && !msg.contains("does not exist"),
            "uniform message must not leak non-existence, got: {msg}"
        );
        assert!(
            !msg.contains("sent successfully"),
            "uniform message must not confirm a send, got: {msg}"
        );
    }

    #[test]
    fn uniform_response_differs_from_old_record_not_found_leak() {
        let (status, body) = uniform_success_response();
        let leak =
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Email doesn't exist");

        assert_ne!(status, StatusCode::NOT_FOUND);
        let success = serde_json::to_value(&*body).unwrap();
        let leak_body = serde_json::to_value(&leak).unwrap_or(serde_json::Value::Null);
        assert_ne!(success, leak_body);
    }

    #[test]
    fn equalize_unknown_email_work_runs_real_argon2() {
        let hash = equalize_unknown_email_work();

        assert!(
            hash.starts_with("$argon2"),
            "equalize_unknown_email_work must produce an Argon2 PHC string, got: {hash}"
        );
        assert!(!hash.is_empty());
    }

    #[test]
    fn dummy_hash_uses_constant_cost() {
        let a = equalize_unknown_email_work();
        let b = equalize_unknown_email_work();
        assert!(
            a.starts_with("$argon2id$") && b.starts_with("$argon2id$"),
            "dummy hash must be Argon2id PHC strings"
        );
        let params_a = a
            .split('$')
            .nth(3)
            .expect("PHC string has a params segment");
        let params_b = b
            .split('$')
            .nth(3)
            .expect("PHC string has a params segment");
        assert_eq!(
            params_a, params_b,
            "Argon2 cost params (m/t/p) must be constant so per-request CPU cost is fixed"
        );
        assert_ne!(
            a, b,
            "two Argon2id hashes must differ due to the random salt"
        );
    }

    #[test]
    fn uniform_response_unaffected_by_equalization_work() {
        let _ = equalize_unknown_email_work(); // result dropped, as in handler
        let (status, body) = uniform_success_response();

        assert_eq!(status, StatusCode::OK);
        let v = serde_json::to_value(&*body).unwrap();
        let body_str = v.to_string();
        assert!(
            !body_str.contains("$argon2"),
            "equalization hash must not leak into the uniform response body"
        );
        assert_eq!(v.as_object().unwrap().len(), 1);
        assert!(v["message"]
            .as_str()
            .unwrap()
            .contains("If an account exists"));
    }
}
