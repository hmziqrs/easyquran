use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::db::sea_models::user;
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::auth::AuthSession;
use crate::services::session_challenges::{self, fingerprint, Quota};

const SESSION_KEY: &str = "rux_auth_fresh_grant";
const SETUP_KEY: &str = "rux_auth_factor_setup";
const LINK_KEY: &str = "rux_auth_link_grants";
const TTL_SECS: i64 = 300;
const LINK_QUOTA: Quota = Quota {
    per_session: 8,
    ttl_secs: TTL_SECS,
};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Purpose {
    SecurityAction,
    FactorSetup,
    OAuthLink,
}

/// Grant held in the caller's session payload (A13). Secret material is stored as
/// fingerprints, never as the password hash or TOTP seed. The session cookie is the binding.
#[derive(Clone, Serialize, Deserialize)]
struct Grant {
    purpose: Purpose,
    user_id: i32,
    issued_at: i64,
    /// Session id at issue. A rotation keeps the payload but not the id, so the grant dies.
    session_id: String,
    credential: String,
    password: Option<String>,
    factor: Option<String>,
    factor_enabled: bool,
}

impl Grant {
    fn new(purpose: Purpose, user: &user::Model, session_id: String) -> Self {
        Self {
            purpose,
            user_id: user.id,
            issued_at: Utc::now().timestamp(),
            session_id,
            credential: fingerprint(&user.session_auth_secret),
            password: user.password.as_deref().map(fingerprint),
            factor: user.two_fa_secret.as_deref().map(fingerprint),
            factor_enabled: user.two_fa_enabled,
        }
    }

    fn is_fresh(&self) -> bool {
        Utc::now().timestamp() - self.issued_at < TTL_SECS
    }

    fn matches(&self, user: &user::Model) -> bool {
        self.user_id == user.id
            && same_secret(&self.credential, &fingerprint(&user.session_auth_secret))
            && same_optional_secret(
                self.password.as_deref(),
                user.password.as_deref().map(fingerprint).as_deref(),
            )
            && same_optional_secret(
                self.factor.as_deref(),
                user.two_fa_secret.as_deref().map(fingerprint).as_deref(),
            )
            && self.factor_enabled == user.two_fa_enabled
    }
}

fn same_secret(left: &str, right: &str) -> bool {
    bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

fn same_optional_secret(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_secret(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn internal_error() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InternalServerError)
        .with_message("Unable to confirm authentication")
}

pub fn required() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::OperationNotAllowed)
        .with_message("Fresh authentication required")
        .with_context(json!({ "reason": "fresh_auth_required" }))
}

/// Issues one five-minute security-action grant after complete authentication.
pub async fn issue(auth: &AuthSession, user: &user::Model) -> Result<(), ErrorResponse> {
    issue_for(auth, user, Purpose::SecurityAction, SESSION_KEY).await
}

pub async fn authorize_link(auth: &AuthSession, state_secret: &str) -> Result<(), ErrorResponse> {
    let user = auth.user_required()?;
    let sid = auth.session().id().ok_or_else(required)?.to_string();
    let grant = Grant::new(Purpose::OAuthLink, user, sid);
    let body = serde_json::to_string(&grant).map_err(|_| internal_error())?;
    session_challenges::insert(auth.session(), LINK_KEY, LINK_QUOTA, state_secret, body).await
}

pub async fn consume_link(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
    state_secret: &str,
) -> Result<user::Model, ErrorResponse> {
    let owner = auth.user_required()?;
    let sid = auth.session().id().ok_or_else(required)?.to_string();
    let user = user::Entity::find_by_id_with_404(db, owner.id).await?;
    if !same_secret(&owner.session_auth_secret, &user.session_auth_secret) {
        return Err(required());
    }
    let body = session_challenges::take(auth.session(), LINK_KEY, LINK_QUOTA, state_secret)
        .await?
        .ok_or_else(required)?;
    let grant: Grant = serde_json::from_str(&body).map_err(|_| required())?;
    if grant.purpose != Purpose::OAuthLink
        || !grant.is_fresh()
        || grant.session_id != sid
        || !grant.matches(&user)
    {
        return Err(required());
    }
    Ok(user)
}

pub async fn issue_setup(auth: &AuthSession, user: &user::Model) -> Result<(), ErrorResponse> {
    issue_for(auth, user, Purpose::FactorSetup, SETUP_KEY).await
}

async fn issue_for(
    auth: &AuthSession,
    user: &user::Model,
    purpose: Purpose,
    key: &str,
) -> Result<(), ErrorResponse> {
    let owner = auth.user_required()?;
    if owner.id != user.id
        || !same_secret(&owner.session_auth_secret, &user.session_auth_secret)
        || !same_optional_secret(owner.password.as_deref(), user.password.as_deref())
        || (purpose == Purpose::SecurityAction
            && (owner.two_fa_enabled != user.two_fa_enabled
                || !same_optional_secret(
                    owner.two_fa_secret.as_deref(),
                    user.two_fa_secret.as_deref(),
                )))
    {
        return Err(required());
    }
    let sid = auth.session().id().ok_or_else(required)?.to_string();
    // Replaces any grant of this purpose the session already holds.
    auth.session()
        .insert(key, &Grant::new(purpose, user, sid))
        .await
        .map_err(|_| internal_error())?;
    auth.session().save().await.map_err(|_| internal_error())?;
    Ok(())
}

/// Reads the grant under `key` for the signed-in owner. Missing, undecodable (legacy), or
/// stale-credential grants read as absent.
async fn current_grant(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
    key: &str,
) -> Result<Option<(Grant, user::Model)>, ErrorResponse> {
    let owner = auth.user_required()?;
    let Some(sid) = auth.session().id().map(|id| id.to_string()) else {
        return Ok(None);
    };
    let Some(grant) = session_challenges::read_payload::<Grant>(auth.session(), key).await else {
        return Ok(None);
    };
    if grant.session_id != sid {
        return Ok(None);
    }
    let user = user::Entity::find_by_id_with_404(db, owner.id).await?;
    if !same_secret(&owner.session_auth_secret, &user.session_auth_secret) {
        return Ok(None);
    }
    Ok(Some((grant, user)))
}

pub async fn available(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
) -> Result<bool, ErrorResponse> {
    available_for(auth, db, Purpose::SecurityAction, SESSION_KEY).await
}

pub async fn setup_available(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
) -> Result<bool, ErrorResponse> {
    available_for(auth, db, Purpose::FactorSetup, SETUP_KEY).await
}

async fn available_for(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
    purpose: Purpose,
    key: &str,
) -> Result<bool, ErrorResponse> {
    let Some((grant, user)) = current_grant(auth, db, key).await? else {
        return Ok(false);
    };
    Ok(grant.purpose == purpose && grant.is_fresh() && grant.matches(&user))
}

/// Atomically consumes a session-bound grant, rejecting expiry and credential/factor changes.
pub async fn consume(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
) -> Result<(), ErrorResponse> {
    consume_for(auth, db, Purpose::SecurityAction, SESSION_KEY).await
}

pub async fn consume_setup(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
) -> Result<(), ErrorResponse> {
    consume_for(auth, db, Purpose::FactorSetup, SETUP_KEY).await
}

async fn consume_for(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
    purpose: Purpose,
    key: &str,
) -> Result<(), ErrorResponse> {
    // Process-local lock: the read-check-remove below must not interleave with another
    // consume on the same session, or one fresh auth could authorise two actions.
    let _guard = session_challenges::lock().await;
    let Some((grant, user)) = current_grant(auth, db, key).await? else {
        return Err(required());
    };
    if grant.purpose != purpose || !grant.is_fresh() || !grant.matches(&user) {
        return Err(required());
    }
    auth.session()
        .remove::<Grant>(key)
        .await
        .map_err(|_| internal_error())?;
    auth.session().save().await.map_err(|_| internal_error())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_expires_at_ttl_boundary() {
        let mut grant = Grant {
            purpose: Purpose::SecurityAction,
            user_id: 1,
            issued_at: Utc::now().timestamp(),
            session_id: String::new(),
            credential: String::new(),
            password: None,
            factor: None,
            factor_enabled: false,
        };
        assert!(grant.is_fresh());
        grant.issued_at -= TTL_SECS;
        assert!(!grant.is_fresh());
    }
}
