use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_client_ip::ClientIp;
use axum_macros::debug_handler;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tracing::{error, info, instrument, warn};

use crate::{
    db::sea_models::{forgot_password, user},
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::{
        abuse_limiter::{self, FailureMode},
        auth::AuthSession,
        mail::{mail_error_kind, send_forgot_password_email, MailRouter},
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

/// The one answer to every well-formed request (A12). Known and unknown
/// addresses both get it, so the response is not an account-existence signal.
pub const REQUEST_ACCEPTED: &str =
    "If an account exists for that email, a reset code has been sent.";

/// Reset grants live in the caller's session payload, so they survive a
/// process restart and are bound to that session's cookie (A12).
pub mod reset_grant {
    use chrono::Utc;
    use rand::Rng;
    use serde::{Deserialize, Serialize};
    use subtle::ConstantTimeEq;
    use tracing::error;

    use crate::{
        db::sea_models::user,
        error::{ErrorCode, ErrorResponse},
        services::auth::AuthSession,
    };

    const SESSION_KEY: &str = "rux_forgot_password_grant";
    /// Reset window in seconds. Same ten minutes as the former in-memory TTL.
    pub const TOKEN_TTL_SECS: i64 = 600;

    #[derive(Clone, Serialize, Deserialize)]
    pub struct ResetGrant {
        token: String,
        pub user_id: i32,
        pub session_auth_secret: String,
        issued_at: i64,
    }

    fn session_error<E: std::fmt::Debug>(err: E) -> ErrorResponse {
        error!(error = ?err, "reset grant session access failed");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Unable to start password reset")
    }

    /// Stores a fresh grant in this session and returns the token the client
    /// presents on reset. Replaces any grant the session already held.
    pub async fn issue(auth: &AuthSession, account: &user::Model) -> Result<String, ErrorResponse> {
        let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
        rand::rng().fill(bytes.as_mut());
        let token = hex::encode(*bytes);
        let grant = ResetGrant {
            token: token.clone(),
            user_id: account.id,
            session_auth_secret: account.session_auth_secret.clone(),
            issued_at: Utc::now().timestamp(),
        };
        auth.session()
            .insert(SESSION_KEY, &grant)
            .await
            .map_err(session_error)?;
        auth.session().save().await.map_err(session_error)?;
        Ok(token)
    }

    /// Removes the session's grant (single use) and returns it only while the
    /// window is open.
    pub async fn take(auth: &AuthSession) -> Result<Option<ResetGrant>, ErrorResponse> {
        let grant: Option<ResetGrant> = auth
            .session()
            .remove(SESSION_KEY)
            .await
            .map_err(session_error)?;
        Ok(grant.filter(|grant| Utc::now().timestamp() - grant.issued_at < TOKEN_TTL_SECS))
    }

    /// Puts a grant back after a busy rejection, so the client can retry once.
    pub async fn restore(auth: &AuthSession, grant: ResetGrant) -> Result<(), ErrorResponse> {
        auth.session()
            .insert(SESSION_KEY, &grant)
            .await
            .map_err(session_error)
    }

    pub fn token_matches(grant: &ResetGrant, presented: &str) -> bool {
        bool::from(grant.token.as_bytes().ct_eq(presented.as_bytes()))
    }
}

/// Request-side work shared by every address. Unknown addresses do the same
/// hashing as known ones, write no row and send no mail.
pub async fn request_code(
    db: &DatabaseConnection,
    mailer: Arc<MailRouter>,
    secret_key: &[u8],
    email: &str,
) -> Result<(), ErrorResponse> {
    let normalized_email = crate::utils::email::normalize_email(email);
    let account = match user::Entity::find_by_email(db, normalized_email.clone()).await {
        Ok(account) => account,
        Err(err) => {
            error!("Database error finding user: {}", err);
            return Err(err);
        }
    };
    let code = forgot_password::Entity::generate_code();
    let code_hash = crate::utils::code_hash::hash_code(secret_key, &code);
    match account {
        Some(account) => {
            issue_for_account(db, mailer, account, &normalized_email, code, code_hash).await
        }
        None => {
            warn!("Forgot password requested for an address with no account");
            Ok(())
        }
    }
}

async fn issue_for_account(
    db: &DatabaseConnection,
    mailer: Arc<MailRouter>,
    account: user::Model,
    email: &str,
    code: String,
    code_hash: String,
) -> Result<(), ErrorResponse> {
    let user_id = account.id;
    match forgot_password::Entity::find_query(db, Some(user_id), None, None).await {
        Ok(verification) if verification.is_in_delay() => {
            // Delay still applies: no new code and no mail.
            warn!(user_id, "Forgot password in delay period");
            return Ok(());
        }
        Ok(_) => {}
        Err(err) if err.code == ErrorCode::InvalidInput => {}
        Err(err) => {
            error!(user_id, "Error checking forgot password delay: {}", err);
            return Err(err);
        }
    }

    // Store only the keyed hash; the plaintext is emailed but never persisted.
    forgot_password::Entity::regenerate_for_email(db, user_id, email, code_hash).await?;

    // ── DELIVERY: reset-code hand-off ─────────────────────────────────────────
    // This is the single place the plaintext reset code leaves the request. To
    // change how a code reaches the user (SMTP, log file, test-fixture echo…),
    // edit ONLY this block — generation, hashing, and storage above stay as-is.
    //
    // Non-production: there is usually no SMTP server in dev, so the code is
    // written to the API log (`just dev` console). Production: the send runs
    // detached so the answer does not wait on SMTP and cannot differ by
    // account; a transport error is logged, never returned. NEVER log the code
    // on the production branch.
    if matches!(crate::config::settings::is_production(), Ok(false)) {
        // No email field: PII guard in tests/security_tests.rs forbids logging
        // the recovery address here. user_id identifies the row for lookup.
        info!(
            user_id,
            reset_code = %code,
            "DEV delivery: forgot-password reset code (non-production build)"
        );
    } else {
        let email = email.to_owned();
        tokio::spawn(async move {
            if let Err(err) = send_forgot_password_email(&mailer, &email, &code).await {
                error!(
                    user_id,
                    error_kind = mail_error_kind(&err),
                    "Failed to send forgot password email"
                );
            }
        });
    }

    info!(user_id, "Recovery code issued");
    Ok(())
}

#[debug_handler]
#[instrument(skip(state, payload), fields(client_ip = %secure_ip))]
pub async fn generate(
    state: State<AppState>,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1GeneratePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Requests are not counted (A12): failures are counted on verify only, and
    // the per-route layer in router.rs bounds request volume per IP.
    request_code(
        &state.sea_db,
        state.mailer.clone(),
        &state.secret_key,
        &payload.email,
    )
    .await?;
    Ok((StatusCode::OK, Json(json!({ "message": REQUEST_ACCEPTED }))))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(client_ip = %secure_ip))]
pub async fn verify(
    state: State<AppState>,
    auth: AuthSession,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1VerifyPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let ip_key = format!("forgot_password_verify:{}", secure_ip);
    abuse_limiter::gate(
        &state.gate_store,
        &ip_key,
        ABUSE_LIMITER_CONFIG,
        FailureMode::Ip,
    )
    .await?;
    let email = crate::utils::email::normalize_email(&payload.email);
    let account_key = account_verification_key(&email);
    abuse_limiter::gate(
        &state.gate_store,
        &account_key,
        ABUSE_LIMITER_CONFIG,
        FailureMode::Credential,
    )
    .await?;

    let code_hash = crate::utils::code_hash::hash_code(&state.secret_key, &payload.code);
    let verified = forgot_password::Entity::verify_code(&state.sea_db, &email, &code_hash).await;
    let attempt = abuse_limiter::attempt_of_code(&verified);
    abuse_limiter::settle(
        &state.gate_store,
        &ip_key,
        ABUSE_LIMITER_CONFIG,
        FailureMode::Ip,
        attempt,
    )
    .await;
    abuse_limiter::settle(
        &state.gate_store,
        &account_key,
        ABUSE_LIMITER_CONFIG,
        FailureMode::Credential,
        attempt,
    )
    .await;
    let account = verified?;
    let user_id = account.id;
    let reset_token = reset_grant::issue(&auth, &account).await?;

    info!(
        user_id,
        "Forgot password code verified and consumed; reset token issued"
    );
    Ok((StatusCode::OK, Json(V1VerifyResponse { reset_token })))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(client_ip = %secure_ip))]
pub async fn reset(
    state: State<AppState>,
    auth: AuthSession,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1ResetPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // Intentionally shares the verify bucket — do not rename to a reset-specific key.
    let key_prefix = format!("forgot_password_verify:{}", secure_ip);
    abuse_limiter::gate(
        &state.gate_store,
        &key_prefix,
        ABUSE_LIMITER_CONFIG,
        FailureMode::Ip,
    )
    .await?;

    if payload.password != payload.confirm_password {
        warn!("Password mismatch");
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Password and confirm password do not match"));
    }

    let invalid = || {
        ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("Reset token is invalid or has expired")
    };
    let Some(grant) = reset_grant::take(&auth).await? else {
        warn!("Reset attempted without an open reset grant in this session");
        return Err(invalid());
    };
    if !reset_grant::token_matches(&grant, &payload.reset_token) {
        warn!("Reset token did not match the session grant");
        return Err(invalid());
    }

    let user_id = grant.user_id;
    match forgot_password::Entity::reset(
        &state.sea_db,
        user_id,
        payload.password.clone(),
        &grant.session_auth_secret,
    )
    .await
    {
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
            if err.code == ErrorCode::RateLimited {
                reset_grant::restore(&auth, grant).await?;
            }
            error!(user_id, "Failed to reset password: {}", err);
            Err(err)
        }
    }
}

fn account_verification_key(email: &str) -> String {
    format!(
        "forgot_password_verify:account:{}",
        crate::utils::email::normalize_email(email)
    )
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn recovery_verification_budget_follows_account_across_clients() {
        use rux_request_gate::{FailureMode, GateDecision, RateLimitStore};
        let store = rux_request_gate::InMemoryStore::default();
        // Failures only: three wrong codes for one account, however it is cased.
        for email in [
            "Owner@Example.com",
            "owner@example.com",
            "OWNER@example.com",
        ] {
            store
                .abuse_record(
                    &super::account_verification_key(email),
                    super::ABUSE_LIMITER_CONFIG,
                    FailureMode::Credential,
                )
                .await
                .unwrap();
        }
        let same = store
            .abuse_peek(
                &super::account_verification_key("owner@example.com"),
                super::ABUSE_LIMITER_CONFIG,
                FailureMode::Credential,
            )
            .await
            .unwrap();
        assert!(matches!(same, GateDecision::Delay { .. }));
        let other = store
            .abuse_peek(
                &super::account_verification_key("another@example.com"),
                super::ABUSE_LIMITER_CONFIG,
                FailureMode::Credential,
            )
            .await
            .unwrap();
        assert_eq!(other, GateDecision::Pass);
    }

    fn handler_src() -> &'static str {
        let src = include_str!("controller.rs");
        let (code, _) = src
            .rsplit_once("#[cfg(test)]")
            .expect("tests module present");
        code
    }

    #[test]
    fn request_answer_is_one_shared_message_with_no_existence_branch() {
        let code = handler_src();
        assert!(
            !code.contains("ErrorCode::RecordNotFound"),
            "the request path must not answer RecordNotFound for unknown addresses (A12)"
        );
        assert!(
            !code.contains("No account exists"),
            "no account-existence copy may come back (A12)"
        );
        assert!(
            code.contains("REQUEST_ACCEPTED"),
            "every request answers with the shared message"
        );
    }

    #[test]
    fn request_path_is_not_counted_by_the_failure_limiter() {
        let code = handler_src();
        let generate = code
            .split("pub async fn generate(")
            .nth(1)
            .and_then(|rest| rest.split("pub async fn verify(").next())
            .expect("generate present");
        assert!(
            !generate.contains("abuse_limiter::limiter("),
            "request must not append to the limiter; only verify counts failures (A12)"
        );
    }

    #[test]
    fn dev_reset_code_log_is_gated_off_in_production() {
        let code = handler_src();
        assert!(
            code.contains("is_production()"),
            "the dev-only reset-code log must be gated on !is_production() so production NEVER logs the plaintext code"
        );
        let delivery_block = code
            .split("DELIVERY: reset-code hand-off")
            .nth(1)
            .expect("delivery block marker present");
        assert!(
            delivery_block.contains("is_production()"),
            "the is_production() gate must sit inside the delivery block, not elsewhere in the file"
        );
    }
}
