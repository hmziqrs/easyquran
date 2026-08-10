use oauth2::PkceCodeVerifier;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tower_sessions::Session;
use tracing::{error, warn};

use crate::error::{ErrorCode, ErrorResponse};

const STATE_TTL_SECS: u64 = 600;

type StateMap = HashMap<String, (String, Instant)>;

static OAUTH_STATES: OnceLock<Mutex<StateMap>> = OnceLock::new();

fn oauth_states() -> &'static Mutex<StateMap> {
    OAUTH_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reap_stale(map: &mut StateMap) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < STATE_TTL_SECS);
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
