use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rand::Rng;
use rux_auth::AuthBackend as _;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;

use crate::db::sea_models::user;
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::auth::AuthSession;

const PENDING_KEY: &str = "rux_auth_pending_totp";
const BOOTSTRAP_KEY: &str = "rux_auth_pending_totp_bootstrap";
const TTL: Duration = Duration::from_secs(300);
const MAX_GRANTS: usize = 10_000;

#[derive(Clone)]
pub struct PendingGrant {
    pub user_id: i32,
    session_id: String,
    credential_secret: String,
    factor_secret: String,
}

impl PendingGrant {
    pub fn matches_user(&self, user: &user::Model) -> bool {
        self.user_id == user.id
            && user.two_fa_enabled
            && bool::from(
                self.credential_secret
                    .as_bytes()
                    .ct_eq(user.session_auth_secret.as_bytes()),
            )
            && user.two_fa_secret.as_ref().is_some_and(|secret| {
                bool::from(self.factor_secret.as_bytes().ct_eq(secret.as_bytes()))
            })
    }
}

type GrantMap = HashMap<String, (PendingGrant, Instant)>;

static GRANTS: OnceLock<Mutex<GrantMap>> = OnceLock::new();

fn grants() -> &'static Mutex<GrantMap> {
    GRANTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn internal_error() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InternalServerError)
        .with_message("Unable to prepare second factor")
}

fn reap(map: &mut GrantMap) {
    map.retain(|_, (_, issued)| issued.elapsed() < TTL);
}

pub async fn issue(auth: &AuthSession, user: &user::Model) -> Result<String, ErrorResponse> {
    if auth.user.is_some() || !user.two_fa_enabled {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed));
    }
    let factor_secret = user.two_fa_secret.clone().ok_or_else(internal_error)?;
    if auth.session().id().is_none() {
        auth.session()
            .insert(BOOTSTRAP_KEY, true)
            .await
            .map_err(|_| internal_error())?;
        auth.session().save().await.map_err(|_| internal_error())?;
    }
    let session_id = auth.session().id().ok_or_else(internal_error)?.to_string();
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    let token = hex::encode(bytes);
    let grant = PendingGrant {
        user_id: user.id,
        session_id: session_id.clone(),
        credential_secret: user.session_auth_secret.clone(),
        factor_secret,
    };
    {
        let mut map = grants().lock().map_err(|_| internal_error())?;
        reap(&mut map);
        map.retain(|_, (existing, _)| existing.session_id != session_id);
        if map.len() >= MAX_GRANTS {
            return Err(ErrorResponse::new(ErrorCode::RateLimited));
        }
        map.insert(token.clone(), (grant, Instant::now()));
    }
    if auth.session().insert(PENDING_KEY, &token).await.is_err()
        || auth.session().save().await.is_err()
    {
        grants()
            .lock()
            .map_err(|_| internal_error())?
            .remove(&token);
        return Err(internal_error());
    }
    auth.session()
        .remove::<bool>(BOOTSTRAP_KEY)
        .await
        .map_err(|_| internal_error())?;
    Ok(token)
}

pub async fn take(auth: &AuthSession, token: &str) -> Result<Option<PendingGrant>, ErrorResponse> {
    let Some(session_id) = auth.session().id().map(|id| id.to_string()) else {
        return Ok(None);
    };
    let grant = {
        let mut map = grants().lock().map_err(|_| internal_error())?;
        reap(&mut map);
        if map
            .get(token)
            .is_some_and(|(grant, _)| grant.session_id == session_id)
        {
            map.remove(token).map(|(grant, _)| grant)
        } else {
            None
        }
    };
    if grant.is_some() {
        let pending = auth
            .session()
            .get::<String>(PENDING_KEY)
            .await
            .map_err(|_| internal_error())?;
        if pending.as_deref() == Some(token) {
            auth.session()
                .remove::<String>(PENDING_KEY)
                .await
                .map_err(|_| internal_error())?;
        }
    }
    Ok(grant)
}

pub async fn response(auth: &AuthSession) -> Result<Option<Value>, ErrorResponse> {
    if auth.user.is_some() {
        return Ok(None);
    }
    let Some(token) = auth
        .session()
        .get::<String>(PENDING_KEY)
        .await
        .map_err(|_| internal_error())?
    else {
        return Ok(None);
    };
    let session_id = auth.session().id().map(|id| id.to_string());
    let grant = {
        let mut map = grants().lock().map_err(|_| internal_error())?;
        reap(&mut map);
        map.get(&token)
            .filter(|(grant, _)| Some(grant.session_id.as_str()) == session_id.as_deref())
            .map(|(grant, _)| grant.clone())
    };
    if let Some(grant) = grant {
        let user = auth
            .backend()
            .get_user(&grant.user_id)
            .await
            .map_err(|_| internal_error())?;
        if user.as_ref().is_some_and(|user| grant.matches_user(user)) {
            return Ok(Some(
                json!({ "status": "totp_required", "totp_token": token }),
            ));
        }
        let _ = take(auth, &token).await?;
    }
    auth.session()
        .remove::<String>(PENDING_KEY)
        .await
        .map_err(|_| internal_error())?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_grants_are_reaped_even_without_store_expiry() {
        let grant = PendingGrant {
            user_id: 1,
            session_id: "session".into(),
            credential_secret: "credential".into(),
            factor_secret: "factor".into(),
        };
        let mut map = GrantMap::from([
            ("old".into(), (grant.clone(), Instant::now() - TTL)),
            ("live".into(), (grant, Instant::now())),
        ]);
        reap(&mut map);
        assert!(!map.contains_key("old"));
        assert!(map.contains_key("live"));
    }
}
