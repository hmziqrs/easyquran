use base64::prelude::*;
use oauth2::PkceCodeVerifier;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tower_sessions::Session;
use tracing::{error, warn};

use crate::error::{ErrorCode, ErrorResponse};
use crate::services::session_challenges::{self, Quota};

pub const NATIVE_TOKEN_NONCE_TTL_SECS: u64 = 600;
/// Process-wide cap on native sign-in nonces. Native nonces carry no session, so they
/// stay in memory. At the cap a new challenge is refused, never evicted.
const MAX_NATIVE_NONCES: usize = 10_000;

/// Browser OAuth states live in the caller's session payload (A13). Ten-minute life,
/// eight pending flows per session; a full session refuses the ninth.
const OAUTH_SESSION_KEY: &str = "rux_oauth_states";
const OAUTH_QUOTA: Quota = Quota {
    per_session: 8,
    ttl_secs: 600,
};

type NativeNonceMap = HashMap<String, (NativeTokenProvider, Instant)>;

static NATIVE_NONCES: OnceLock<Mutex<NativeNonceMap>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTokenProvider {
    Google,
    Apple,
}

pub struct NativeTokenChallenge {
    pub nonce: String,
    pub provider_nonce: String,
}

fn native_nonces() -> &'static Mutex<NativeNonceMap> {
    NATIVE_NONCES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reap_stale_native_nonces(map: &mut NativeNonceMap) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < NATIVE_TOKEN_NONCE_TTL_SECS);
}

pub fn issue_native_token_challenge(
    provider: NativeTokenProvider,
) -> Result<NativeTokenChallenge, ErrorResponse> {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    let nonce = BASE64_URL_SAFE_NO_PAD.encode(bytes);
    let provider_nonce = match provider {
        NativeTokenProvider::Google => nonce.clone(),
        NativeTokenProvider::Apple => hex::encode(Sha256::digest(nonce.as_bytes())),
    };

    let mut map = native_nonces().lock().map_err(|e| {
        error!(error = %e, "Native token nonce map poisoned");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to issue sign-in challenge")
    })?;
    reap_stale_native_nonces(&mut map);
    if map.len() >= MAX_NATIVE_NONCES {
        warn!("native nonce cap reached; refusing new sign-in challenge");
        return Err(ErrorResponse::new(ErrorCode::RateLimited)
            .with_message("Too many pending sign-in attempts. Try again shortly."));
    }
    map.insert(provider_nonce.clone(), (provider, Instant::now()));

    Ok(NativeTokenChallenge {
        nonce,
        provider_nonce,
    })
}

pub fn consume_native_token_nonce(
    provider: NativeTokenProvider,
    provider_nonce: Option<&str>,
) -> Result<(), ErrorResponse> {
    let provider_nonce = provider_nonce
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(invalid_native_nonce)?;
    let stored = {
        let mut map = native_nonces().lock().map_err(|e| {
            error!(error = %e, "Native token nonce map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to verify sign-in challenge")
        })?;
        reap_stale_native_nonces(&mut map);
        map.remove(provider_nonce)
    };

    match stored {
        Some((stored_provider, _)) if stored_provider == provider => Ok(()),
        _ => Err(invalid_native_nonce()),
    }
}

fn invalid_native_nonce() -> ErrorResponse {
    warn!("Invalid, expired, provider-mismatched, or consumed native token nonce");
    ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid sign-in challenge")
}

#[allow(clippy::result_large_err)]
pub fn oauth_session_id(session: &Session) -> Result<String, ErrorResponse> {
    session.id().map(|id| id.to_string()).ok_or_else(|| {
        warn!("OAuth attempted without a session id");
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("No active session")
    })
}

pub async fn store_oauth_state(
    session: &Session,
    state_secret: &str,
    pkce_verifier_secret: &str,
    nonce: Option<&str>,
    link_user_id: Option<i32>,
) -> Result<(), ErrorResponse> {
    let payload = json!({ "v": pkce_verifier_secret, "n": nonce, "lu": link_user_id }).to_string();
    session_challenges::insert(
        session,
        OAUTH_SESSION_KEY,
        OAUTH_QUOTA,
        state_secret,
        payload,
    )
    .await
}

#[derive(Deserialize)]
struct StoredState {
    v: String,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    lu: Option<i32>,
}

pub struct ConsumedOauthState {
    pub pkce_verifier: Option<PkceCodeVerifier>,
    pub nonce: Option<String>,
    /// Account this flow intends to link the provider identity onto (link intent); `None` for
    /// login flows. `serde(default)` keeps payloads stored before the field existed readable.
    pub link_user_id: Option<i32>,
}

/// Single-use CSRF guard: the slot is removed on read (`session_challenges::take`) before the
/// payload is parsed, so a replayed `state` observes `None`.
pub async fn consume_oauth_state(
    session: &Session,
    state_secret: &str,
) -> Result<ConsumedOauthState, ErrorResponse> {
    let payload = session_challenges::take(session, OAUTH_SESSION_KEY, OAUTH_QUOTA, state_secret)
        .await?
        .ok_or_else(|| {
            warn!("Invalid, expired, already-consumed, or session-mismatched OAuth state");
            ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid OAuth state")
        })?;

    let parsed: StoredState = serde_json::from_str(&payload).map_err(|e| {
        error!(error = ?e, "Failed to parse stored OAuth state");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Corrupt OAuth state")
    })?;

    Ok(ConsumedOauthState {
        pkce_verifier: if parsed.v.is_empty() {
            None
        } else {
            Some(PkceCodeVerifier::new(parsed.v))
        },
        nonce: parsed.n,
        link_user_id: parsed.lu,
    })
}

/// Cancelled/error callback detection for link flows: the provider echoes `state` back, so consume
/// it (single-use — the flow is abandoned either way) and report the link target it carried.
pub async fn link_user_id_from_cancelled_state(
    session: &Session,
    state_secret: Option<&str>,
) -> Option<i32> {
    let state_secret = state_secret.filter(|secret| !secret.trim().is_empty())?;
    match consume_oauth_state(session, state_secret).await {
        Ok(state) => state.link_user_id,
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tower_sessions::MemoryStore;

    fn fresh_session() -> Session {
        Session::new(None, Arc::new(MemoryStore::default()), None)
    }

    #[test]
    fn google_challenge_nonce_is_consumed_once() {
        let challenge = issue_native_token_challenge(NativeTokenProvider::Google).unwrap();

        consume_native_token_nonce(NativeTokenProvider::Google, Some(&challenge.provider_nonce))
            .unwrap();

        assert!(
            consume_native_token_nonce(
                NativeTokenProvider::Google,
                Some(&challenge.provider_nonce)
            )
            .is_err(),
            "replayed challenge must fail"
        );
    }

    #[test]
    fn apple_challenge_returns_sha256_provider_nonce() {
        let challenge = issue_native_token_challenge(NativeTokenProvider::Apple).unwrap();

        assert_eq!(
            challenge.provider_nonce,
            hex::encode(Sha256::digest(challenge.nonce.as_bytes()))
        );
    }

    #[tokio::test]
    async fn state_per_session_quota_refuses_ninth() {
        let session = fresh_session();
        for i in 0..8 {
            store_oauth_state(&session, &format!("quota-state-{i}"), "", None, None)
                .await
                .unwrap();
        }
        let refused = store_oauth_state(&session, "quota-state-8", "", None, None)
            .await
            .unwrap_err();
        assert!(matches!(refused.code, ErrorCode::RateLimited));
        // Refusal evicts nothing: the first pending flow still completes.
        consume_oauth_state(&session, "quota-state-0")
            .await
            .expect("refused insert must not evict an existing flow");
    }

    #[tokio::test]
    async fn state_round_trips_link_user_id() {
        let session = fresh_session();
        store_oauth_state(
            &session,
            "link-state",
            "link-verifier",
            Some("link-nonce"),
            Some(42),
        )
        .await
        .unwrap();

        let consumed = consume_oauth_state(&session, "link-state").await.unwrap();

        assert_eq!(consumed.link_user_id, Some(42));
        assert_eq!(consumed.nonce.as_deref(), Some("link-nonce"));
        assert_eq!(
            consumed.pkce_verifier.map(|v| v.secret().to_string()),
            Some("link-verifier".to_string())
        );
    }

    #[tokio::test]
    async fn state_is_single_use() {
        let session = fresh_session();
        store_oauth_state(&session, "once", "", None, None)
            .await
            .unwrap();
        consume_oauth_state(&session, "once").await.unwrap();
        assert!(consume_oauth_state(&session, "once").await.is_err());
    }

    #[tokio::test]
    async fn state_round_trips_login_without_link_intent() {
        let session = fresh_session();
        store_oauth_state(&session, "login-state", "", None, None)
            .await
            .unwrap();

        let consumed = consume_oauth_state(&session, "login-state").await.unwrap();

        assert_eq!(consumed.link_user_id, None);
    }

    #[tokio::test]
    async fn state_parses_payload_stored_before_link_intent_existed() {
        // Payload body written before the `lu` field was added; serde(default) keeps it readable.
        let session = fresh_session();
        session_challenges::insert(
            &session,
            OAUTH_SESSION_KEY,
            OAUTH_QUOTA,
            "legacy-state",
            r#"{"v":"","n":null}"#.to_string(),
        )
        .await
        .unwrap();

        let consumed = consume_oauth_state(&session, "legacy-state").await.unwrap();

        assert_eq!(consumed.link_user_id, None);
    }
}
