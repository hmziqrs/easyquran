use base64::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tracing::{error, warn};
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration};

use crate::error::{ErrorCode, ErrorResponse};

const STATE_TTL_SECS: u64 = 300;
const MAX_STATES: usize = 10_000;

#[derive(Serialize)]
#[serde(tag = "kind")]
enum EntryRef<'a> {
    Registration {
        state: &'a PasskeyRegistration,
        user_id: i32,
    },
    Authentication {
        state: &'a PasskeyAuthentication,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum EntryOwned {
    Registration {
        state: PasskeyRegistration,
        user_id: i32,
    },
    Authentication {
        state: PasskeyAuthentication,
    },
}

type StateMap = HashMap<String, (String, Instant)>;

static STATES: OnceLock<Mutex<StateMap>> = OnceLock::new();

fn states() -> &'static Mutex<StateMap> {
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reap_stale(map: &mut StateMap) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < STATE_TTL_SECS);
}

fn issue(entry: &EntryRef<'_>) -> Result<String, ErrorResponse> {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    let handle = BASE64_URL_SAFE_NO_PAD.encode(bytes);
    let payload = serde_json::to_string(entry).map_err(|e| {
        error!(error = ?e, "Failed to serialize passkey state");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to issue passkey challenge")
    })?;

    let mut map = states().lock().map_err(|e| {
        error!(error = %e, "Passkey state map poisoned");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to issue passkey challenge")
    })?;
    reap_stale(&mut map);
    if map.len() >= MAX_STATES {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (_, issued_at))| *issued_at)
            .map(|(key, _)| key.clone())
        {
            map.remove(&oldest);
        }
    }
    map.insert(handle.clone(), (payload, Instant::now()));
    Ok(handle)
}

/// Single-use guard: `map.remove(...)` below must stay a remove-on-read (never
/// `get`/peek) so a replayed handle observes `None`. The WebAuthn state itself
/// never leaves the server — a client-forged or replayed state must not reach
/// finish_passkey_authentication/finish_passkey_registration.
fn consume(handle: &str) -> Result<EntryOwned, ErrorResponse> {
    let handle = handle.trim();
    if handle.is_empty() {
        return Err(invalid_state());
    }
    let stored = {
        let mut map = states().lock().map_err(|e| {
            error!(error = %e, "Passkey state map poisoned");
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to verify passkey challenge")
        })?;
        reap_stale(&mut map);
        map.remove(handle)
    };

    let (payload, _at) = stored.ok_or_else(invalid_state)?;
    serde_json::from_str(&payload).map_err(|e| {
        error!(error = ?e, "Failed to parse stored passkey state");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Corrupt passkey state")
    })
}

fn invalid_state() -> ErrorResponse {
    warn!("Invalid, expired, or already-consumed passkey state handle");
    ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid passkey challenge")
}

pub fn issue_registration(
    user_id: i32,
    state: &PasskeyRegistration,
) -> Result<String, ErrorResponse> {
    issue(&EntryRef::Registration { state, user_id })
}

/// The registration state is bound to the user that started registration, so a
/// handle issued in one user's session cannot finish under another's.
pub fn consume_registration(
    handle: &str,
    user_id: i32,
) -> Result<PasskeyRegistration, ErrorResponse> {
    match consume(handle)? {
        EntryOwned::Registration {
            state,
            user_id: bound_user_id,
        } if bound_user_id == user_id => Ok(state),
        _ => Err(invalid_state()),
    }
}

pub fn issue_authentication(state: &PasskeyAuthentication) -> Result<String, ErrorResponse> {
    issue(&EntryRef::Authentication { state })
}

pub fn consume_authentication(handle: &str) -> Result<PasskeyAuthentication, ErrorResponse> {
    match consume(handle)? {
        EntryOwned::Authentication { state } => Ok(state),
        _ => Err(invalid_state()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sea_models::user;
    use crate::services::webauthn::WebauthnService;
    use chrono::TimeZone;

    fn make_user() -> user::Model {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .unwrap()
            .fixed_offset();
        user::Model {
            id: 1,
            name: "Pk User".to_string(),
            email: "pk@example.com".to_string(),
            password: None,
            avatar_id: None,
            is_verified: true,
            role: user::UserRole::User,
            two_fa_enabled: false,
            two_fa_secret: None,
            two_fa_backup_codes: None,
            two_fa_last_totp_counter: None,
            google_id: None,
            oauth_provider: None,
            session_auth_secret: "test-secret".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    fn svc() -> WebauthnService {
        WebauthnService::new("example.com", "https://example.com", "Test")
            .expect("dev WebAuthn service must construct for a real origin")
    }

    #[test]
    fn authentication_state_round_trips_once() {
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state).unwrap();

        consume_authentication(&handle).unwrap();

        assert!(
            consume_authentication(&handle).is_err(),
            "replayed handle must fail"
        );
    }

    #[test]
    fn registration_state_round_trips_once_and_binds_user() {
        let (_challenge, state) = svc().start_registration(&make_user()).unwrap();
        let handle = issue_registration(1, &state).unwrap();

        consume_registration(&handle, 1).unwrap();

        assert!(
            consume_registration(&handle, 1).is_err(),
            "replayed handle must fail"
        );
    }

    #[test]
    fn registration_handle_rejects_other_user() {
        let (_challenge, state) = svc().start_registration(&make_user()).unwrap();
        let handle = issue_registration(1, &state).unwrap();

        assert!(
            consume_registration(&handle, 2).is_err(),
            "a handle issued for user 1 must not finish under user 2"
        );
    }

    #[test]
    fn unknown_handle_rejected() {
        assert!(consume_authentication("does-not-exist").is_err());
        assert!(consume_registration("does-not-exist", 1).is_err());
    }

    #[test]
    fn empty_handle_rejected() {
        assert!(consume_authentication("").is_err());
        assert!(consume_authentication("   ").is_err());
    }

    #[test]
    fn handle_is_opaque_and_bounded() {
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state).unwrap();

        assert_eq!(handle.len(), 43, "32 random bytes base64url-encoded");
        assert!(
            handle
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "handle must be opaque — no state bytes leak into it"
        );
    }

    #[test]
    fn authentication_handle_cannot_be_consumed_as_registration() {
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state).unwrap();

        assert!(
            consume_registration(&handle, 1).is_err(),
            "kind mismatch must fail"
        );
    }
}
