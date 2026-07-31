use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use tower_sessions::Session;

use super::state::AuthSessionState;
use crate::error::{AuthError, AuthErrorCode};
use crate::traits::{AuthBackend, AuthUser};

#[async_trait::async_trait]
pub trait SessionRevocation {
    async fn is_session_revoked(&self, _tower_session_id: &str) -> Result<bool, AuthError> {
        Ok(false)
    }
}

/// Constant-time equality: a `==` on auth hashes leaks bytes via timing.
/// Do not simplify to `==`.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

const SESSION_KEY: &str = "rux_auth";

pub struct AuthSession<B: AuthBackend> {
    pub user: Option<B::User>,

    pub state: Option<AuthSessionState<<B::User as AuthUser>::Id>>,

    session: Session,

    backend: B,
}

impl<B: AuthBackend + SessionRevocation> AuthSession<B> {
    pub async fn new(backend: B, session: Session) -> Self {
        let auth_state: Option<AuthSessionState<<B::User as AuthUser>::Id>> =
            session.get(SESSION_KEY).await.ok().flatten();

        let user = if let Some(ref state) = auth_state {
            match backend.get_user(&state.user_id).await {
                Ok(Some(user)) => {
                    // Invalidate if the credential changed since login (e.g. password reset).
                    if !ct_eq(&state.session_auth_hash, user.session_auth_hash()) {
                        tracing::warn!("Session auth hash mismatch — invalidating stale session");
                        let _ = session.delete().await;
                        None
                    } else if is_revoked(&backend, &session).await {
                        tracing::info!(
                            user_id = ?state.user_id,
                            "Session revoked server-side — invalidating"
                        );
                        let _ = session.delete().await;
                        None
                    } else {
                        Some(user)
                    }
                }
                Ok(None) => {
                    let _ = session.delete().await;
                    None
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to load user from session");
                    None
                }
            }
        } else {
            None
        };

        let auth_state = if user.is_some() { auth_state } else { None };

        Self {
            user,
            state: auth_state,
            session,
            backend,
        }
    }
}

impl<B: AuthBackend> AuthSession<B> {
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub async fn login(&mut self, user: &B::User) -> Result<(), AuthError> {
        let mut state = AuthSessionState::new(user.id(), user.email_verified());
        state.session_auth_hash = user.session_auth_hash().to_vec();

        self.session.cycle_id().await?;
        self.session.insert(SESSION_KEY, &state).await?;
        self.user = Some(user.clone());
        self.state = Some(state);

        self.backend.on_login(user).await?;

        Ok(())
    }

    pub async fn login_with_metadata(
        &mut self,
        user: &B::User,
        device: Option<String>,
        ip_address: Option<String>,
    ) -> Result<(), AuthError> {
        let mut state = AuthSessionState::new(user.id(), user.email_verified())
            .with_metadata(device, ip_address);
        state.session_auth_hash = user.session_auth_hash().to_vec();

        self.session.cycle_id().await?;
        self.session.insert(SESSION_KEY, &state).await?;
        self.user = Some(user.clone());
        self.state = Some(state);

        self.backend.on_login(user).await?;

        Ok(())
    }

    pub async fn logout(&mut self) -> Result<(), AuthError> {
        if let Some(state) = &self.state {
            self.backend.on_logout(&state.user_id).await?;
        }

        self.session.delete().await?;
        self.user = None;
        self.state = None;

        Ok(())
    }

    pub async fn update_ban_status(
        &mut self,
        status: &crate::traits::BanStatus,
    ) -> Result<(), AuthError> {
        if let Some(state) = &mut self.state {
            state.update_ban_status(status);
            self.session.insert(SESSION_KEY, state).await?;
        }
        Ok(())
    }

    pub async fn refresh_verification(&mut self) -> Result<(), AuthError> {
        if let (Some(user), Some(state)) = (&self.user, &mut self.state) {
            state.refresh_verification(user.email_verified());
            self.session.insert(SESSION_KEY, state).await?;
        }
        Ok(())
    }

    pub async fn touch(&mut self) -> Result<(), AuthError> {
        if let Some(state) = &mut self.state {
            state.touch();
            self.session.insert(SESSION_KEY, state).await?;
        }
        Ok(())
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    pub fn user_required(&self) -> Result<&B::User, AuthError> {
        self.user
            .as_ref()
            .ok_or_else(|| AuthError::new(AuthErrorCode::Unauthenticated))
    }

    pub fn state_required(
        &self,
    ) -> Result<&AuthSessionState<<B::User as AuthUser>::Id>, AuthError> {
        self.state
            .as_ref()
            .ok_or_else(|| AuthError::new(AuthErrorCode::Unauthenticated))
    }
}

async fn is_revoked<B>(backend: &B, session: &Session) -> bool
where
    B: AuthBackend + SessionRevocation,
{
    let Some(id) = session.id() else {
        return false;
    };
    let tower_sid = id.to_string();
    match backend.is_session_revoked(&tower_sid).await {
        Ok(true) => true,
        Ok(false) => false,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                tower_session_id = %tower_sid,
                "Revocation check failed (fail-open): session remains valid until store recovers"
            );
            false
        }
    }
}

impl<S, B> FromRequestParts<S> for AuthSession<B>
where
    B: AuthBackend + SessionRevocation + FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                AuthError::new(AuthErrorCode::SessionError)
                    .with_message("Failed to extract session")
            })?;

        let backend = B::from_ref(state);

        let auth_state: Option<AuthSessionState<<B::User as AuthUser>::Id>> =
            session.get(SESSION_KEY).await?;

        let user = if let Some(ref state) = auth_state {
            match backend.get_user(&state.user_id).await {
                Ok(Some(user)) => {
                    // Invalidate if the credential changed since login (e.g. password reset).
                    if !ct_eq(&state.session_auth_hash, user.session_auth_hash()) {
                        tracing::warn!("Session auth hash mismatch — invalidating stale session");
                        let _ = session.delete().await;
                        None
                    } else if is_revoked(&backend, &session).await {
                        tracing::info!(
                            user_id = ?state.user_id,
                            "Session revoked server-side — invalidating"
                        );
                        let _ = session.delete().await;
                        None
                    } else {
                        Some(user)
                    }
                }
                Ok(None) => {
                    let _ = session.delete().await;
                    None
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to load user from session");
                    None
                }
            }
        } else {
            None
        };

        let auth_state = if user.is_some() { auth_state } else { None };

        Ok(Self {
            user,
            state: auth_state,
            session,
            backend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::BanStatus;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tower_sessions::{MemoryStore, Session};

    #[derive(Clone, Debug)]
    struct MockUser {
        id: i32,
        hash: Vec<u8>,
    }

    impl AuthUser for MockUser {
        type Id = i32;
        fn id(&self) -> Self::Id {
            self.id
        }
        fn session_auth_hash(&self) -> &[u8] {
            &self.hash
        }
        fn email_verified(&self) -> bool {
            true
        }
        fn totp_enabled(&self) -> bool {
            false
        }
        fn role_level(&self) -> i32 {
            0
        }
    }

    #[derive(Clone)]
    struct MockBackend;

    #[async_trait]
    impl AuthBackend for MockBackend {
        type User = MockUser;
        async fn get_user(&self, id: &i32) -> Result<Option<MockUser>, AuthError> {
            Ok(Some(MockUser {
                id: *id,
                hash: vec![1, 2, 3],
            }))
        }
        async fn check_ban(&self, _id: &i32) -> Result<BanStatus, AuthError> {
            Ok(BanStatus::NotBanned)
        }
        async fn verify_password(&self, _id: &i32, _password: &str) -> Result<bool, AuthError> {
            Ok(true)
        }
    }

    impl SessionRevocation for MockBackend {}

    #[derive(Clone, Default)]
    struct RevocableBackend {
        revoked: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    }

    #[async_trait]
    impl AuthBackend for RevocableBackend {
        type User = MockUser;
        async fn get_user(&self, id: &i32) -> Result<Option<MockUser>, AuthError> {
            Ok(Some(MockUser {
                id: *id,
                hash: vec![1, 2, 3],
            }))
        }
        async fn check_ban(&self, _id: &i32) -> Result<BanStatus, AuthError> {
            Ok(BanStatus::NotBanned)
        }
        async fn verify_password(&self, _id: &i32, _password: &str) -> Result<bool, AuthError> {
            Ok(true)
        }
    }

    #[async_trait]
    impl SessionRevocation for RevocableBackend {
        async fn is_session_revoked(&self, tower_session_id: &str) -> Result<bool, AuthError> {
            Ok(self
                .revoked
                .lock()
                .map(|g| g.contains(tower_session_id))
                .unwrap_or(false))
        }
    }

    async fn anon_session() -> Session {
        let store = Arc::new(MemoryStore::default());
        let session = Session::new(None, store, None);
        session.insert("anon", true).await.unwrap();
        session.save().await.unwrap();
        session
    }

    #[tokio::test]
    async fn login_rotates_the_session_id() {
        let session = anon_session().await;
        let id_before = session.id().expect("session id present after save");

        let mut auth: AuthSession<MockBackend> = AuthSession::new(MockBackend, session).await;
        auth.login(&MockUser {
            id: 42,
            hash: vec![9, 9, 9],
        })
        .await
        .unwrap();

        auth.session().save().await.unwrap();
        let id_after = auth
            .session()
            .id()
            .expect("rotated session id present after save");
        assert_ne!(
            id_before, id_after,
            "login must rotate the session id (session-fixation defense)"
        );
    }

    #[tokio::test]
    async fn login_with_metadata_rotates_the_session_id() {
        let session = anon_session().await;
        let id_before = session.id().unwrap();

        let mut auth: AuthSession<MockBackend> = AuthSession::new(MockBackend, session).await;
        auth.login_with_metadata(
            &MockUser {
                id: 7,
                hash: vec![4, 5, 6],
            },
            Some("device".to_string()),
            Some("127.0.0.1".to_string()),
        )
        .await
        .unwrap();

        auth.session().save().await.unwrap();
        let id_after = auth.session().id().unwrap();
        assert_ne!(
            id_before, id_after,
            "login_with_metadata must rotate the session id"
        );
    }

    #[tokio::test]
    async fn revoked_session_no_longer_authenticates() {
        let backend = RevocableBackend::default();
        let session = anon_session().await;

        let mut auth: AuthSession<RevocableBackend> =
            AuthSession::new(backend.clone(), session).await;
        auth.login(&MockUser {
            id: 5,
            hash: vec![1, 2, 3],
        })
        .await
        .unwrap();
        auth.session().save().await.unwrap();
        let tower_sid = auth
            .session()
            .id()
            .expect("session id present after save")
            .to_string();
        assert!(
            auth.user.is_some(),
            "session must authenticate before revoke"
        );

        let session_record = auth.session().clone();
        drop(auth);

        {
            let mut g = backend.revoked.lock().unwrap();
            g.insert(tower_sid.clone());
        }

        let next: AuthSession<RevocableBackend> = AuthSession::new(backend, session_record).await;

        assert!(
            next.user.is_none(),
            "a revoked session must not authenticate on the next request"
        );
    }

    #[tokio::test]
    async fn revocation_check_failure_is_fail_open() {
        #[derive(Clone)]
        struct ErroringBackend;
        #[async_trait]
        impl AuthBackend for ErroringBackend {
            type User = MockUser;
            async fn get_user(&self, id: &i32) -> Result<Option<MockUser>, AuthError> {
                Ok(Some(MockUser {
                    id: *id,
                    hash: vec![1, 2, 3],
                }))
            }
            async fn check_ban(&self, _id: &i32) -> Result<BanStatus, AuthError> {
                Ok(BanStatus::NotBanned)
            }
            async fn verify_password(&self, _id: &i32, _password: &str) -> Result<bool, AuthError> {
                Ok(true)
            }
        }
        #[async_trait]
        impl SessionRevocation for ErroringBackend {
            async fn is_session_revoked(&self, _tower_session_id: &str) -> Result<bool, AuthError> {
                Err(AuthError::new(AuthErrorCode::BackendError)
                    .with_message("revocation store unavailable"))
            }
        }

        let session = anon_session().await;
        let mut auth: AuthSession<ErroringBackend> =
            AuthSession::new(ErroringBackend, session).await;
        auth.login(&MockUser {
            id: 9,
            hash: vec![1, 2, 3],
        })
        .await
        .unwrap();
        auth.session().save().await.unwrap();
        let live = auth.session().clone();
        drop(auth);

        let next: AuthSession<ErroringBackend> = AuthSession::new(ErroringBackend, live).await;
        assert!(
            next.user.is_some(),
            "revocation-store error must fail OPEN, not lock the user out"
        );
    }
}
