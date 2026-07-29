//! Session-bound, single-use OAuth state stored in a process-global in-memory
//! TTL map.
//!
//! This is the shared CSRF/PKCE binding used by every third-party OAuth
//! provider module (Facebook, GitHub, Apple, Google). It mirrors the proven
//! `google_auth_v1` implementation: the authorize request's CSRF `state`
//! secret is bound to the caller's tower-session id, and the PKCE verifier +
//! optional OIDC nonce are recovered atomically at the callback by removing
//! the entry (single-use, replay-proof).
//!
//! Previously a Redis `SET` (EX 600) / `GETDEL` pair; now a process-global
//! `OnceLock<Mutex<HashMap>>` with the same 10-min TTL and atomic single-take
//! semantics — the same pattern used by `reset_token` and the checkout-intent
//! store. A restart drops outstanding states, which just forces the user to
//! re-initiate the OAuth flow.
//!
//! Kept provider-agnostic so each provider module only worries about its own
//! authorize/token/userinfo shape, not the session + state plumbing.

use oauth2::PkceCodeVerifier;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tower_sessions::Session;
use tracing::{error, warn};

use crate::error::{ErrorCode, ErrorResponse};

// ── in-memory single-use OAuth state store ────────────────────────────────
// Replaces the prior Redis `SET`/`GETDEL` store. Process-global, keyed by
// `{session_id}:{state_secret}`; removed on consume so a replayed state
// observes `None`. Opportunistically reaped on each access so the table cannot
// grow without bound now that there is no Redis TTL doing it for us.

/// OAuth state TTL, in seconds. 10 min — long enough for an OAuth round-trip,
/// short enough to bound a CSRF/PKCE replay window. Matches the prior Redis
/// `EX(600)`.
const STATE_TTL_SECS: u64 = 600;

type StateMap = HashMap<String, (String, Instant)>;

static OAUTH_STATES: OnceLock<Mutex<StateMap>> = OnceLock::new();

fn oauth_states() -> &'static Mutex<StateMap> {
    OAUTH_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Reap entries older than [`STATE_TTL_SECS`] from `map`. Called under the lock
/// so the housekeeping is consistent with the insert/remove.
fn reap_stale(map: &mut StateMap) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < STATE_TTL_SECS);
}

fn state_key(session_id: &str, state_secret: &str) -> String {
    // Namespaced so it can't collide with session/checkout/reset-token keys.
    format!("oauth:state:{session_id}:{state_secret}")
}

/// Extract the caller's tower-sessions id, required to bind the OAuth state.
/// Fail closed: without a session we cannot bind state, so we refuse to proceed.
#[allow(clippy::result_large_err)]
pub fn oauth_session_id(session: &Session) -> Result<String, ErrorResponse> {
    session.id().map(|id| id.to_string()).ok_or_else(|| {
        warn!("OAuth attempted without a session id");
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("No active session")
    })
}

/// Persist the (session-bound) state → {PKCE verifier, OIDC nonce} mapping,
/// single-use, 10 min. The nonce rides alongside the verifier so it is recovered
/// atomically at the callback and used to validate an OIDC `id_token`
/// (`nonce` claim) when the provider issues one (e.g. Apple). For pure-OAuth2
/// providers (Facebook, GitHub) the nonce is unused and stored as `None`.
/// `pkce_verifier_secret` may be empty for providers that do not use PKCE
/// (Facebook); an empty value round-trips as `None` at consume time.
///
/// Synchronous: the store is process-local, so no I/O is awaited.
pub fn store_oauth_state(
    session_id: &str,
    state_secret: &str,
    pkce_verifier_secret: &str,
    nonce: Option<&str>,
) -> Result<(), ErrorResponse> {
    let payload = json!({ "v": pkce_verifier_secret, "n": nonce }).to_string();
    let mut map = oauth_states().lock().map_err(|e| {
        error!(error = %e, "OAuth state map poisoned");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Failed to store OAuth state")
    })?;
    reap_stale(&mut map);
    map.insert(state_key(session_id, state_secret), (payload, Instant::now()));
    Ok(())
}

#[derive(Deserialize)]
struct StoredState {
    v: String,
    #[serde(default)]
    n: Option<String>,
}

/// The single-use OAuth state recovered at the callback: the PKCE verifier plus
/// the optional OIDC nonce bound to that authorize request.
pub struct ConsumedOauthState {
    /// `None` when the provider flow does not use PKCE (stored empty).
    pub pkce_verifier: Option<PkceCodeVerifier>,
    pub nonce: Option<String>,
}

/// Atomically look up AND delete the session-bound state, returning the PKCE
/// verifier and OIDC nonce. Removing the entry on read makes the state
/// single-use (a replayed state observes `None`). Fails closed if the state is
/// missing, expired, or belongs to another session.
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
