use chrono::Utc;
use rand::Rng;
use rux_auth::AuthBackend as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use subtle::ConstantTimeEq;

use crate::db::sea_models::user;
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::auth::AuthSession;
use crate::services::session_challenges::{self, fingerprint};

const PENDING_KEY: &str = "rux_auth_pending_totp";
const BOOTSTRAP_KEY: &str = "rux_auth_pending_totp_bootstrap";
const TTL_SECS: i64 = 300;

/// Second-factor step after a verified first factor (A13). Lives in the session payload:
/// one pending login per session, bound to that session's cookie, secrets fingerprinted.
#[derive(Clone, Serialize, Deserialize)]
pub struct PendingGrant {
    pub user_id: i32,
    token: String,
    /// Session id at issue. A rotation keeps the payload but not the id, so the grant dies.
    session_id: String,
    issued_at: i64,
    credential: String,
    factor: String,
}

fn session_key(session: &tower_sessions::Session) -> String {
    session.id().map(|id| id.to_string()).unwrap_or_default()
}

impl PendingGrant {
    fn is_fresh(&self) -> bool {
        Utc::now().timestamp() - self.issued_at < TTL_SECS
    }

    fn bound_to(&self, session: &tower_sessions::Session) -> bool {
        let current = session_key(session);
        !current.is_empty() && self.session_id == current
    }

    pub fn matches_user(&self, user: &user::Model) -> bool {
        self.user_id == user.id
            && user.two_fa_enabled
            && bool::from(
                self.credential
                    .as_bytes()
                    .ct_eq(fingerprint(&user.session_auth_secret).as_bytes()),
            )
            && user.two_fa_secret.as_deref().is_some_and(|secret| {
                bool::from(self.factor.as_bytes().ct_eq(fingerprint(secret).as_bytes()))
            })
    }

    fn token_matches(&self, token: &str) -> bool {
        bool::from(self.token.as_bytes().ct_eq(token.as_bytes()))
    }
}

fn internal_error() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InternalServerError)
        .with_message("Unable to prepare second factor")
}

pub async fn issue(auth: &AuthSession, user: &user::Model) -> Result<String, ErrorResponse> {
    if auth.user.is_some() || !user.two_fa_enabled {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed));
    }
    let factor_secret = user.two_fa_secret.as_deref().ok_or_else(internal_error)?;
    if auth.session().id().is_none() {
        auth.session()
            .insert(BOOTSTRAP_KEY, true)
            .await
            .map_err(|_| internal_error())?;
        auth.session().save().await.map_err(|_| internal_error())?;
    }
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    let token = hex::encode(bytes);
    let grant = PendingGrant {
        user_id: user.id,
        token: token.clone(),
        session_id: session_key(auth.session()),
        issued_at: Utc::now().timestamp(),
        credential: fingerprint(&user.session_auth_secret),
        factor: fingerprint(factor_secret),
    };
    {
        let _guard = session_challenges::lock().await;
        // Replaces any pending login this session already holds.
        auth.session()
            .insert(PENDING_KEY, &grant)
            .await
            .map_err(|_| internal_error())?;
        auth.session().save().await.map_err(|_| internal_error())?;
    }
    auth.session()
        .remove::<bool>(BOOTSTRAP_KEY)
        .await
        .map_err(|_| internal_error())?;
    Ok(token)
}

/// Removes the pending login and returns it only when the token matches and the window is open.
pub async fn take(auth: &AuthSession, token: &str) -> Result<Option<PendingGrant>, ErrorResponse> {
    let _guard = session_challenges::lock().await;
    let Some(grant) =
        session_challenges::read_payload::<PendingGrant>(auth.session(), PENDING_KEY).await
    else {
        return Ok(None);
    };
    if !grant.token_matches(token) || !grant.is_fresh() || !grant.bound_to(auth.session()) {
        return Ok(None);
    }
    auth.session()
        .remove::<PendingGrant>(PENDING_KEY)
        .await
        .map_err(|_| internal_error())?;
    auth.session().save().await.map_err(|_| internal_error())?;
    Ok(Some(grant))
}

pub async fn response(auth: &AuthSession) -> Result<Option<Value>, ErrorResponse> {
    if auth.user.is_some() {
        return Ok(None);
    }
    let Some(grant) =
        session_challenges::read_payload::<PendingGrant>(auth.session(), PENDING_KEY).await
    else {
        return Ok(None);
    };
    if grant.is_fresh() && grant.bound_to(auth.session()) {
        let user = auth
            .backend()
            .get_user(&grant.user_id)
            .await
            .map_err(|_| internal_error())?;
        if user.as_ref().is_some_and(|user| grant.matches_user(user)) {
            return Ok(Some(
                json!({ "status": "totp_required", "totp_token": grant.token }),
            ));
        }
    }
    {
        let _guard = session_challenges::lock().await;
        auth.session()
            .remove::<PendingGrant>(PENDING_KEY)
            .await
            .map_err(|_| internal_error())?;
        auth.session().save().await.map_err(|_| internal_error())?;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_grant_expires_at_ttl_boundary() {
        let grant = PendingGrant {
            user_id: 1,
            token: "t".into(),
            session_id: "sid".into(),
            issued_at: Utc::now().timestamp() - TTL_SECS,
            credential: String::new(),
            factor: String::new(),
        };
        assert!(!grant.is_fresh());
    }
}
