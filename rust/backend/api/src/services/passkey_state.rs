use crate::db::sea_models::user;
use base64::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tower_sessions::Session;
use tracing::{error, warn};
use webauthn_rs::prelude::{DiscoverableAuthentication, PasskeyRegistration};

use crate::error::{ErrorCode, ErrorResponse};
use crate::services::session_challenges::{self, Quota};

/// Pending passkey challenges live in the caller's session payload (A13): they survive a
/// restart and are bound to that session's cookie. Five-minute life, eight per session.
const SESSION_KEY: &str = "rux_passkey_challenges";
const QUOTA: Quota = Quota {
    per_session: 8,
    ttl_secs: 300,
};

#[derive(Serialize, Deserialize)]
struct RegistrationBinding {
    user_id: i32,
    credential_secret: String,
    factor_secret: Option<String>,
    factor_enabled: bool,
}

impl RegistrationBinding {
    fn from_user(user: &user::Model) -> Self {
        Self {
            user_id: user.id,
            credential_secret: user.session_auth_secret.clone(),
            factor_secret: user.two_fa_secret.clone(),
            factor_enabled: user.two_fa_enabled,
        }
    }

    fn matches(&self, user: &user::Model) -> bool {
        self.user_id == user.id
            && self.factor_enabled == user.two_fa_enabled
            && bool::from(
                self.credential_secret
                    .as_bytes()
                    .ct_eq(user.session_auth_secret.as_bytes()),
            )
            && self.factor_secret == user.two_fa_secret
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum EntryRef<'a> {
    Registration {
        state: &'a PasskeyRegistration,
        binding: RegistrationBinding,
    },
    Authentication {
        state: &'a DiscoverableAuthentication,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum EntryOwned {
    Registration {
        state: PasskeyRegistration,
        binding: RegistrationBinding,
    },
    Authentication {
        state: DiscoverableAuthentication,
    },
}

async fn issue(entry: &EntryRef<'_>, session: &Session) -> Result<String, ErrorResponse> {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    let handle = BASE64_URL_SAFE_NO_PAD.encode(bytes);
    let payload = serde_json::to_string(entry).map_err(|e| {
        error!(error = ?e, "Failed to serialize passkey state");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to issue passkey challenge")
    })?;
    session_challenges::insert(session, SESSION_KEY, QUOTA, &handle, payload).await?;
    Ok(handle)
}

/// Single-use: `session_challenges::take` removes the slot before it is parsed, so a
/// replayed handle observes `None`. The WebAuthn state never leaves the server.
async fn consume(handle: &str, session: &Session) -> Result<EntryOwned, ErrorResponse> {
    let handle = handle.trim();
    if handle.is_empty() {
        return Err(invalid_state());
    }
    let payload = session_challenges::take(session, SESSION_KEY, QUOTA, handle)
        .await?
        .ok_or_else(invalid_state)?;
    serde_json::from_str(&payload).map_err(|e| {
        error!(error = ?e, "Failed to parse stored passkey state");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Corrupt passkey state")
    })
}

fn invalid_state() -> ErrorResponse {
    warn!("Invalid, expired, or already-consumed passkey state handle");
    ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid passkey challenge")
}

pub async fn issue_registration(
    user: &user::Model,
    session: &Session,
    state: &PasskeyRegistration,
) -> Result<String, ErrorResponse> {
    issue(
        &EntryRef::Registration {
            state,
            binding: RegistrationBinding::from_user(user),
        },
        session,
    )
    .await
}

pub async fn consume_registration(
    handle: &str,
    user: &user::Model,
    session: &Session,
) -> Result<PasskeyRegistration, ErrorResponse> {
    match consume(handle, session).await? {
        EntryOwned::Registration { state, binding } if binding.matches(user) => Ok(state),
        _ => Err(invalid_state()),
    }
}

pub async fn issue_authentication(
    state: &DiscoverableAuthentication,
    session: &Session,
) -> Result<String, ErrorResponse> {
    issue(&EntryRef::Authentication { state }, session).await
}

pub async fn consume_authentication(
    handle: &str,
    session: &Session,
) -> Result<DiscoverableAuthentication, ErrorResponse> {
    match consume(handle, session).await? {
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
    use std::sync::Arc;
    use tower_sessions::MemoryStore;

    fn fresh_session() -> Session {
        Session::new(None, Arc::new(MemoryStore::default()), None)
    }

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
            verification_identity_hash: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn svc() -> WebauthnService {
        WebauthnService::new("example.com", "https://example.com", "Test")
            .expect("dev WebAuthn service must construct for a real origin")
    }

    #[tokio::test]
    async fn authentication_state_round_trips_once() {
        let session = fresh_session();
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state, &session).await.unwrap();

        consume_authentication(&handle, &session).await.unwrap();

        assert!(
            consume_authentication(&handle, &session).await.is_err(),
            "replayed handle must fail"
        );
    }

    #[tokio::test]
    async fn registration_state_round_trips_once_and_binds_user() {
        let session = fresh_session();
        let (_challenge, state) = svc().start_registration(&make_user()).unwrap();
        let handle = issue_registration(&make_user(), &session, &state)
            .await
            .unwrap();

        consume_registration(&handle, &make_user(), &session)
            .await
            .unwrap();

        assert!(
            consume_registration(&handle, &make_user(), &session)
                .await
                .is_err(),
            "replayed handle must fail"
        );
    }

    #[tokio::test]
    async fn registration_handle_rejects_other_user() {
        let session = fresh_session();
        let (_challenge, state) = svc().start_registration(&make_user()).unwrap();
        let handle = issue_registration(&make_user(), &session, &state)
            .await
            .unwrap();

        let mut other_user = make_user();
        other_user.id = 2;
        assert!(consume_registration(&handle, &other_user, &session)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn unknown_handle_rejected() {
        let session = fresh_session();
        assert!(consume_authentication("does-not-exist", &session)
            .await
            .is_err());
        assert!(
            consume_registration("does-not-exist", &make_user(), &session)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn empty_handle_rejected() {
        let session = fresh_session();
        assert!(consume_authentication("", &session).await.is_err());
        assert!(consume_authentication("   ", &session).await.is_err());
    }

    #[tokio::test]
    async fn handle_is_opaque_and_bounded() {
        let session = fresh_session();
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state, &session).await.unwrap();

        assert_eq!(handle.len(), 43, "32 random bytes base64url-encoded");
        assert!(
            handle
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "handle must be opaque — no state bytes leak into it"
        );
    }

    #[tokio::test]
    async fn authentication_handle_cannot_be_consumed_as_registration() {
        let session = fresh_session();
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state, &session).await.unwrap();

        assert!(
            consume_registration(&handle, &make_user(), &session)
                .await
                .is_err(),
            "kind mismatch must fail"
        );
    }

    #[tokio::test]
    async fn registration_rejects_changed_credentials_or_factor() {
        let user = make_user();
        let (_, state) = svc().start_registration(&user).unwrap();
        let mut credential_changed = user.clone();
        credential_changed.session_auth_secret = "changed".into();
        let mut factor_changed = user.clone();
        factor_changed.two_fa_enabled = true;
        factor_changed.two_fa_secret = Some("new-factor".into());
        for current in [credential_changed, factor_changed] {
            let session = fresh_session();
            let handle = issue_registration(&user, &session, &state).await.unwrap();
            assert!(consume_registration(&handle, &current, &session)
                .await
                .is_err());
        }
    }

    #[tokio::test]
    async fn challenge_survives_restart_from_session_store() {
        // State lives in the session payload, not in process memory: a Session object
        // rebuilt over the same store (the post-restart view) still consumes the handle.
        let store = Arc::new(MemoryStore::default());
        let before = Session::new(None, store.clone(), None);
        let (_challenge, state) = svc().start_login().unwrap();
        let handle = issue_authentication(&state, &before).await.unwrap();

        let after_restart = Session::new(before.id(), store, None);
        consume_authentication(&handle, &after_restart)
            .await
            .expect("unused challenge must survive a restart");
    }

    #[tokio::test]
    async fn full_session_refuses_new_challenge() {
        let session = fresh_session();
        let (_challenge, state) = svc().start_login().unwrap();
        for _ in 0..8 {
            issue_authentication(&state, &session).await.unwrap();
        }
        let refused = issue_authentication(&state, &session).await.unwrap_err();
        assert_eq!(refused.code, ErrorCode::RateLimited);
    }
}
