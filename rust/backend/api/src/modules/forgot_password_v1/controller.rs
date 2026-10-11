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
        mail::{mail_error_kind, mail_error_to_response, send_forgot_password_email},
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

mod reset_token {
    use super::*;
    use rand::Rng;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use zeroize::Zeroize;

    const TOKEN_TTL: Duration = Duration::from_secs(600);
    const MAX_TRACKED_USERS: usize = 10_000;

    pub struct ResetGrant {
        pub user_id: i32,
        pub session_auth_secret: String,
        generation: String,
        issued_at: Instant,
    }

    #[derive(Default)]
    struct TokenMap {
        grants: HashMap<String, ResetGrant>,
        generations: HashMap<i32, (String, Instant)>,
    }

    impl TokenMap {
        fn reap_stale(&mut self, now: Instant) {
            self.grants
                .retain(|_, grant| now.saturating_duration_since(grant.issued_at) < TOKEN_TTL);
            self.generations
                .retain(|_, (_, at)| now.saturating_duration_since(*at) < TOKEN_TTL);
        }

        fn mint(
            &mut self,
            token: &str,
            user_id: i32,
            session_auth_secret: String,
            issued_at: Instant,
        ) -> Result<(), ErrorResponse> {
            self.reap_stale(issued_at);
            if !self.generations.contains_key(&user_id)
                && self.generations.len() >= MAX_TRACKED_USERS
            {
                return Err(ErrorResponse::new(ErrorCode::ServiceUnavailable)
                    .with_message("Password recovery is temporarily unavailable"));
            }
            self.grants.retain(|_, grant| grant.user_id != user_id);
            let generation = namespaced_key(token);
            self.generations
                .insert(user_id, (generation.clone(), issued_at));
            self.grants.insert(
                generation.clone(),
                ResetGrant {
                    user_id,
                    session_auth_secret,
                    generation,
                    issued_at,
                },
            );
            Ok(())
        }

        fn take(&mut self, token: &str, now: Instant) -> Option<ResetGrant> {
            self.reap_stale(now);
            self.grants.remove(&namespaced_key(token))
        }

        fn restore_after_rejection(
            &mut self,
            grant: ResetGrant,
            error: &ErrorResponse,
            now: Instant,
        ) -> bool {
            self.reap_stale(now);
            if error.code != ErrorCode::RateLimited
                || now.saturating_duration_since(grant.issued_at) >= TOKEN_TTL
                || self.grants.contains_key(&grant.generation)
                || !self
                    .generations
                    .get(&grant.user_id)
                    .is_some_and(|(generation, issued_at)| {
                        *generation == grant.generation && *issued_at == grant.issued_at
                    })
            {
                return false;
            }
            self.grants.insert(grant.generation.clone(), grant);
            true
        }
    }

    static TOKENS: OnceLock<Mutex<TokenMap>> = OnceLock::new();

    fn tokens() -> &'static Mutex<TokenMap> {
        TOKENS.get_or_init(|| Mutex::new(TokenMap::default()))
    }

    fn namespaced_key(token: &str) -> String {
        format!("forgot_password:reset_token:{token}")
    }

    pub async fn mint(account: &user::Model) -> Result<String, ErrorResponse> {
        let mut bytes = zeroize::Zeroizing::new([0u8; 32]);
        rand::rng().fill(bytes.as_mut());
        let token = hex::encode(*bytes);
        bytes.zeroize();

        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "reset_token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        map.mint(
            &token,
            account.id,
            account.session_auth_secret.clone(),
            Instant::now(),
        )?;
        Ok(token)
    }

    /// Removes the grant so a concurrent reset cannot replay it.
    pub async fn take(token: &str) -> Result<Option<ResetGrant>, ErrorResponse> {
        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "reset_token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        Ok(map.take(token, Instant::now()))
    }

    /// Restores only an unexpired, unsuperseded grant rejected before password work was admitted.
    pub async fn restore_after_rejection(
        grant: ResetGrant,
        error: &ErrorResponse,
    ) -> Result<bool, ErrorResponse> {
        if error.code != ErrorCode::RateLimited {
            return Ok(false);
        }
        let mut map = tokens().lock().map_err(|e| {
            error!(error = %e, "reset_token map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
        })?;
        Ok(map.restore_after_rejection(grant, error, Instant::now()))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn busy_rejection_preserves_same_bound_grant_for_one_retry() {
            let mut map = TokenMap::default();
            let issued_at = Instant::now();
            map.mint("original", 7, "credential-binding".into(), issued_at)
                .unwrap();
            let grant = map.take("original", issued_at).unwrap();
            assert!(map.take("original", issued_at).is_none());
            assert!(map.restore_after_rejection(
                grant,
                &ErrorResponse::new(ErrorCode::RateLimited),
                issued_at,
            ));
            let retried = map.take("original", issued_at).unwrap();
            assert_eq!(retried.user_id, 7);
            assert_eq!(retried.session_auth_secret, "credential-binding");
            assert_eq!(retried.issued_at, issued_at);
            assert!(map.take("original", issued_at).is_none());
        }

        #[test]
        fn retry_does_not_extend_original_expiry() {
            let mut map = TokenMap::default();
            let issued_at = Instant::now();
            map.mint("original", 7, "binding".into(), issued_at)
                .unwrap();
            let retry_at = issued_at + TOKEN_TTL - Duration::from_secs(1);
            let grant = map.take("original", retry_at).unwrap();
            assert!(map.restore_after_rejection(
                grant,
                &ErrorResponse::new(ErrorCode::RateLimited),
                retry_at,
            ));
            assert!(map.take("original", issued_at + TOKEN_TTL).is_none());
        }

        #[test]
        fn expired_in_flight_grant_cannot_be_restored() {
            let mut map = TokenMap::default();
            let issued_at = Instant::now();
            map.mint("original", 7, "binding".into(), issued_at)
                .unwrap();
            let grant = map.take("original", issued_at).unwrap();
            assert!(!map.restore_after_rejection(
                grant,
                &ErrorResponse::new(ErrorCode::RateLimited),
                issued_at + TOKEN_TTL,
            ));
            assert!(map.take("original", issued_at + TOKEN_TTL).is_none());
        }

        #[test]
        fn superseding_grant_cannot_be_overwritten_by_old_busy_retry() {
            let mut map = TokenMap::default();
            let issued_at = Instant::now();
            map.mint("old", 7, "old-binding".into(), issued_at).unwrap();
            let old = map.take("old", issued_at).unwrap();
            map.mint("new", 7, "new-binding".into(), issued_at).unwrap();
            assert!(!map.restore_after_rejection(
                old,
                &ErrorResponse::new(ErrorCode::RateLimited),
                issued_at,
            ));
            assert!(map.take("old", issued_at).is_none());
            assert_eq!(
                map.take("new", issued_at).unwrap().session_auth_secret,
                "new-binding",
            );
        }

        #[test]
        fn consuming_newer_grant_does_not_revive_old_busy_grant() {
            let mut map = TokenMap::default();
            let issued_at = Instant::now();
            map.mint("old", 7, "old-binding".into(), issued_at).unwrap();
            let old = map.take("old", issued_at).unwrap();
            map.mint("new", 7, "new-binding".into(), issued_at).unwrap();
            let new = map.take("new", issued_at).unwrap();
            assert!(!map.restore_after_rejection(
                old,
                &ErrorResponse::new(ErrorCode::RateLimited),
                issued_at,
            ));
            assert!(map.take("old", issued_at).is_none());
            assert!(map.restore_after_rejection(
                new,
                &ErrorResponse::new(ErrorCode::RateLimited),
                issued_at,
            ));
            assert_eq!(
                map.take("new", issued_at).unwrap().session_auth_secret,
                "new-binding",
            );
        }

        #[test]
        fn non_admission_errors_leave_grant_consumed() {
            for code in [ErrorCode::InvalidInput, ErrorCode::InternalServerError] {
                let mut map = TokenMap::default();
                let issued_at = Instant::now();
                map.mint("original", 7, "binding".into(), issued_at)
                    .unwrap();
                let grant = map.take("original", issued_at).unwrap();
                assert!(!map.restore_after_rejection(grant, &ErrorResponse::new(code), issued_at,));
                assert!(map.take("original", issued_at).is_none());
            }
        }

        #[test]
        fn consumed_generation_markers_stay_bounded_and_expire() {
            let mut map = TokenMap::default();
            let issued_at = Instant::now();
            for user_id in 0..MAX_TRACKED_USERS as i32 {
                let token = user_id.to_string();
                map.mint(&token, user_id, "binding".into(), issued_at)
                    .unwrap();
                map.take(&token, issued_at).unwrap();
            }
            assert_eq!(
                map.mint("overflow", i32::MAX, "binding".into(), issued_at)
                    .unwrap_err()
                    .code,
                ErrorCode::ServiceUnavailable,
            );
            map.mint(
                "after-expiry",
                i32::MAX,
                "binding".into(),
                issued_at + TOKEN_TTL,
            )
            .unwrap();
            assert_eq!(map.generations.len(), 1);
        }
    }
}

/// 2026-08-26 product decision: accurate feedback over the old SC-006
/// anti-enumeration mask. Unknown emails answer with an explicit
/// RecordNotFound; successful sends confirm delivery. The abuse limiter above
/// remains the brute-force brake on the now-honest oracle.
#[debug_handler]
#[instrument(skip(state, payload), fields(client_ip = %secure_ip))]
pub async fn generate(
    state: State<AppState>,
    ClientIp(secure_ip): ClientIp,
    payload: ValidatedJson<V1GeneratePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
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
    let normalized_email = crate::utils::email::normalize_email(&payload.email);
    abuse_limiter::limiter(
        &state.gate_store,
        &format!("forgot_password_request:account:{normalized_email}"),
        ABUSE_LIMITER_CONFIG,
    )
    .await?;
    let user = match user::Entity::find_by_email(pool, normalized_email.clone()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Forgot password requested for non-existent email");
            return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("No account exists for that email"));
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
                // Delay stays enforced (no new code, no email) and the user is told why.
                warn!(user_id, "Forgot password in delay period");
                return Err(ErrorResponse::new(ErrorCode::TooManyAttempts).with_message(
                    "A reset code was already sent. Please wait a minute before requesting another.",
                ));
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
    if let Err(err) =
        forgot_password::Entity::regenerate_for_email(pool, user_id, &normalized_email, code_hash)
            .await
    {
        error!(user_id, "Failed to store forgot-password code: {}", err);
        return Err(err);
    }

    // ── DELIVERY: reset-code hand-off ─────────────────────────────────────────
    // This is the single place the plaintext reset code leaves the request. To
    // change how a code reaches the user (SMTP, log file, test-fixture echo…),
    // edit ONLY this block — generation, hashing, and storage above stay as-is.
    //
    // Non-production: there is usually no SMTP server in dev, so the code is
    // written to the API log (`just dev` console) and delivery is treated as
    // done. Production: email it and fail the request on a transport error.
    // NEVER log the code on the production branch.
    if matches!(crate::config::settings::is_production(), Ok(false)) {
        // No email field: PII guard in tests/security_tests.rs forbids logging
        // the recovery address here. user_id identifies the row for lookup.
        info!(
            user_id,
            reset_code = %code,
            "DEV delivery: forgot-password reset code (non-production build)"
        );
    } else if let Err(err) = send_forgot_password_email(&state.mailer, &payload.email, &code).await
    {
        error!(
            user_id,
            error_kind = mail_error_kind(&err),
            "Failed to send forgot password email"
        );
        return Err(mail_error_to_response(&err));
    }

    info!(user_id, "Recovery code issued");
    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "A password reset code has been sent to your email.",
        })),
    ))
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
    let email = crate::utils::email::normalize_email(&payload.email);
    limit_account_verification(&state.gate_store, &email).await?;

    let code_hash = crate::utils::code_hash::hash_code(&state.secret_key, &payload.code);
    let account = forgot_password::Entity::verify_code(&state.sea_db, &email, &code_hash).await?;
    let user_id = account.id;
    let reset_token = reset_token::mint(&account).await?;

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

    let grant = match reset_token::take(&payload.reset_token).await? {
        Some(grant) => grant,
        None => {
            warn!("Reset attempted with an unknown or already-used reset token");
            return Err(ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("Reset token is invalid or has expired"));
        }
    };

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
            reset_token::restore_after_rejection(grant, &err).await?;
            error!(user_id, "Failed to reset password: {}", err);
            Err(err)
        }
    }
}

async fn limit_account_verification(
    store: &rux_request_gate::InMemoryStore,
    email: &str,
) -> Result<(), ErrorResponse> {
    let email = crate::utils::email::normalize_email(email);
    abuse_limiter::limiter(
        store,
        &format!("forgot_password_verify:account:{email}"),
        ABUSE_LIMITER_CONFIG,
    )
    .await
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn recovery_verification_budget_follows_account_across_clients() {
        let store = rux_request_gate::InMemoryStore::default();
        for _ in 0..2 {
            super::limit_account_verification(&store, "Owner@Example.com")
                .await
                .unwrap();
        }
        assert_eq!(
            super::limit_account_verification(&store, "owner@example.com")
                .await
                .unwrap_err()
                .code,
            crate::error::ErrorCode::TooManyAttempts,
        );
        super::limit_account_verification(&store, "another@example.com")
            .await
            .unwrap();
    }

    fn handler_src() -> &'static str {
        let src = include_str!("controller.rs");
        let (code, _) = src
            .rsplit_once("#[cfg(test)]")
            .expect("tests module present");
        code
    }

    #[test]
    fn unknown_email_answers_explicit_record_not_found() {
        let code = handler_src();
        assert!(
            code.contains("ErrorCode::RecordNotFound"),
            "the unknown-email branch must answer RecordNotFound (explicit contract, 2026-08-26)"
        );
        assert!(
            code.contains("No account exists for that email"),
            "the not-found message must stay user-readable and stable — the web flow matches on status 404"
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

    #[test]
    fn success_envelope_confirms_the_send() {
        let code = handler_src();
        assert!(
            code.contains("A password reset code has been sent to your email"),
            "success must be affirmative now that existence is explicit — no 'if an account exists' conditional"
        );
        assert!(
            !code.contains("if an account exists"),
            "the old uniform conditional copy must not come back"
        );
    }
}
