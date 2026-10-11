use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rand::Rng;
use serde_json::json;
use subtle::ConstantTimeEq;

use crate::db::sea_models::user;
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::auth::AuthSession;

const SESSION_KEY: &str = "rux_auth_fresh_grant";
const SETUP_KEY: &str = "rux_auth_factor_setup";
const TTL: Duration = Duration::from_secs(300);
const MAX_GRANTS: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    SecurityAction,
    FactorSetup,
    OAuthLink,
}

#[derive(Clone)]
struct Grant {
    purpose: Purpose,
    session_id: String,
    user_id: i32,
    credential_secret: String,
    password: Option<String>,
    factor_secret: Option<String>,
    factor_enabled: bool,
}

impl Grant {
    fn matches(&self, session_id: &str, user: &user::Model) -> bool {
        self.session_id == session_id
            && self.user_id == user.id
            && same_secret(&self.credential_secret, &user.session_auth_secret)
            && same_optional_secret(self.password.as_deref(), user.password.as_deref())
            && same_optional_secret(self.factor_secret.as_deref(), user.two_fa_secret.as_deref())
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

type GrantMap = HashMap<String, (Grant, Instant)>;
static GRANTS: OnceLock<Mutex<GrantMap>> = OnceLock::new();

fn grants() -> &'static Mutex<GrantMap> {
    GRANTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reap(map: &mut GrantMap) {
    map.retain(|_, (_, issued)| issued.elapsed() < TTL);
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
    let session_id = auth.session().id().ok_or_else(required)?.to_string();
    let grant = Grant {
        purpose: Purpose::OAuthLink,
        session_id: session_id.clone(),
        user_id: user.id,
        credential_secret: user.session_auth_secret.clone(),
        password: user.password.clone(),
        factor_secret: user.two_fa_secret.clone(),
        factor_enabled: user.two_fa_enabled,
    };
    let mut map = grants().lock().map_err(|_| internal_error())?;
    reap(&mut map);
    if map.len() >= MAX_GRANTS {
        return Err(ErrorResponse::new(ErrorCode::RateLimited));
    }
    map.insert(
        format!("link:{session_id}:{state_secret}"),
        (grant, Instant::now()),
    );
    Ok(())
}

pub async fn consume_link(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
    state_secret: &str,
) -> Result<user::Model, ErrorResponse> {
    let owner = auth.user_required()?;
    let session_id = auth.session().id().ok_or_else(required)?.to_string();
    let user = user::Entity::find_by_id_with_404(db, owner.id).await?;
    if !same_secret(&owner.session_auth_secret, &user.session_auth_secret) {
        return Err(required());
    }
    let mut map = grants().lock().map_err(|_| internal_error())?;
    reap(&mut map);
    let key = format!("link:{session_id}:{state_secret}");
    let grant = map
        .remove(&key)
        .map(|(grant, _)| grant)
        .ok_or_else(required)?;
    if grant.purpose != Purpose::OAuthLink || !grant.matches(&session_id, &user) {
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
    let session_id = auth.session().id().ok_or_else(required)?.to_string();
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    let token = hex::encode(bytes);
    let grant = Grant {
        purpose,
        session_id: session_id.clone(),
        user_id: user.id,
        credential_secret: user.session_auth_secret.clone(),
        password: user.password.clone(),
        factor_secret: user.two_fa_secret.clone(),
        factor_enabled: user.two_fa_enabled,
    };
    {
        let mut map = grants().lock().map_err(|_| internal_error())?;
        reap(&mut map);
        map.retain(|_, (existing, _)| {
            existing.session_id != session_id || existing.purpose != purpose
        });
        if map.len() >= MAX_GRANTS {
            return Err(ErrorResponse::new(ErrorCode::RateLimited));
        }
        map.insert(token.clone(), (grant, Instant::now()));
    }
    if auth.session().insert(key, &token).await.is_err() || auth.session().save().await.is_err() {
        grants()
            .lock()
            .map_err(|_| internal_error())?
            .remove(&token);
        return Err(internal_error());
    }
    Ok(())
}

async fn current_grant(
    auth: &AuthSession,
    db: &sea_orm::DatabaseConnection,
    key: &str,
) -> Result<Option<(String, String, user::Model)>, ErrorResponse> {
    let owner = auth.user_required()?;
    let Some(session_id) = auth.session().id().map(|id| id.to_string()) else {
        return Ok(None);
    };
    let Some(token) = auth
        .session()
        .get::<String>(key)
        .await
        .map_err(|_| internal_error())?
    else {
        return Ok(None);
    };
    let user = user::Entity::find_by_id_with_404(db, owner.id).await?;
    if !same_secret(&owner.session_auth_secret, &user.session_auth_secret) {
        return Ok(None);
    }
    Ok(Some((token, session_id, user)))
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
    let Some((token, session_id, user)) = current_grant(auth, db, key).await? else {
        return Ok(false);
    };
    let mut map = grants().lock().map_err(|_| internal_error())?;
    reap(&mut map);
    Ok(map
        .get(&token)
        .is_some_and(|(grant, _)| grant.purpose == purpose && grant.matches(&session_id, &user)))
}

/// Atomically consumes a cookie-bound grant, rejecting expiry and credential/factor changes.
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
    let Some((token, session_id, user)) = current_grant(auth, db, key).await? else {
        return Err(required());
    };
    let consumed = {
        let mut map = grants().lock().map_err(|_| internal_error())?;
        reap(&mut map);
        if map
            .get(&token)
            .is_some_and(|(grant, _)| grant.purpose == purpose && grant.matches(&session_id, &user))
        {
            map.remove(&token);
            true
        } else {
            false
        }
    };
    if !consumed {
        return Err(required());
    }
    auth.session()
        .remove::<String>(key)
        .await
        .map_err(|_| internal_error())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_grants_are_reaped_at_boundary() {
        let grant = Grant {
            purpose: Purpose::SecurityAction,
            session_id: "sid".into(),
            user_id: 1,
            credential_secret: "secret".into(),
            password: None,
            factor_secret: None,
            factor_enabled: false,
        };
        let mut map = HashMap::from([
            ("expired".into(), (grant.clone(), Instant::now() - TTL)),
            ("live".into(), (grant, Instant::now())),
        ]);
        reap(&mut map);
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("live"));
    }
}
