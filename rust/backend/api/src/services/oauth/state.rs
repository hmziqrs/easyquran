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

const STATE_TTL_SECS: u64 = 600;
pub const NATIVE_TOKEN_NONCE_TTL_SECS: u64 = STATE_TTL_SECS;
const MAX_NATIVE_NONCES: usize = 10_000;
const MAX_OAUTH_STATES: usize = 10_000;

type StateMap = HashMap<String, (String, Instant)>;

static OAUTH_STATES: OnceLock<Mutex<StateMap>> = OnceLock::new();
static NATIVE_NONCES: OnceLock<Mutex<NativeNonceMap>> = OnceLock::new();

type NativeNonceMap = HashMap<String, (NativeTokenProvider, Instant)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTokenProvider {
    Google,
    Apple,
}

pub struct NativeTokenChallenge {
    pub nonce: String,
    pub provider_nonce: String,
}

fn oauth_states() -> &'static Mutex<StateMap> {
    OAUTH_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn native_nonces() -> &'static Mutex<NativeNonceMap> {
    NATIVE_NONCES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reap_stale(map: &mut StateMap) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < STATE_TTL_SECS);
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
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (_, issued_at))| *issued_at)
            .map(|(key, _)| key.clone())
        {
            map.remove(&oldest);
        }
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

fn state_key(session_id: &str, state_secret: &str) -> String {
    format!("oauth:state:{session_id}:{state_secret}")
}

#[allow(clippy::result_large_err)]
pub fn oauth_session_id(session: &Session) -> Result<String, ErrorResponse> {
    session.id().map(|id| id.to_string()).ok_or_else(|| {
        warn!("OAuth attempted without a session id");
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("No active session")
    })
}

pub fn store_oauth_state(
    session_id: &str,
    state_secret: &str,
    pkce_verifier_secret: &str,
    nonce: Option<&str>,
) -> Result<(), ErrorResponse> {
    let payload = json!({ "v": pkce_verifier_secret, "n": nonce }).to_string();
    let mut map = oauth_states().lock().map_err(|e| {
        error!(error = %e, "OAuth state map poisoned");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to store OAuth state")
    })?;
    reap_stale(&mut map);
    if map.len() >= MAX_OAUTH_STATES {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (_, stored_at))| *stored_at)
            .map(|(key, _)| key.clone())
        {
            map.remove(&oldest);
        }
    }
    map.insert(
        state_key(session_id, state_secret),
        (payload, Instant::now()),
    );
    Ok(())
}

#[derive(Deserialize)]
struct StoredState {
    v: String,
    #[serde(default)]
    n: Option<String>,
}

pub struct ConsumedOauthState {
    pub pkce_verifier: Option<PkceCodeVerifier>,
    pub nonce: Option<String>,
}

/// Single-use CSRF guard: `map.remove(...)` below must stay a remove-on-read (never `get`/peek, don't defer past a successful parse) so a replayed `state` observes `None`. No test pins this — do not weaken without one.
pub fn consume_oauth_state(
    session_id: &str,
    state_secret: &str,
) -> Result<ConsumedOauthState, ErrorResponse> {
    let stored = {
        let mut map = oauth_states().lock().map_err(|e| {
            error!(error = %e, "OAuth state map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to verify OAuth state")
        })?;
        reap_stale(&mut map);
        map.remove(&state_key(session_id, state_secret))
    };

    let (payload, _at) = stored.ok_or_else(|| {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn state_store_caps_map_size() {
        for i in 0..(MAX_OAUTH_STATES + 5) {
            store_oauth_state(
                &format!("cap-session-{i}"),
                &format!("cap-state-{i}"),
                "",
                None,
            )
            .unwrap();
        }

        let map = oauth_states().lock().unwrap();
        assert_eq!(
            map.len(),
            MAX_OAUTH_STATES,
            "OAUTH_STATES must evict-oldest instead of growing without bound"
        );
    }
}
