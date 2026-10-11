use std::sync::Arc;

use async_trait::async_trait;
use rux_auth::{
    auth_requirements, check_requirements, AuthBackend, AuthError, AuthErrorCode, AuthSession,
    AuthUser, BanStatus, SessionRevocation,
};
use tower_sessions::{MemoryStore, Session};

#[derive(Clone, Debug)]
struct User;

impl AuthUser for User {
    type Id = i32;

    fn id(&self) -> i32 {
        1
    }

    fn session_auth_hash(&self) -> &[u8] {
        b"test-secret"
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
struct Backend;

#[async_trait]
impl AuthBackend for Backend {
    type User = User;

    async fn get_user(&self, _id: &i32) -> Result<Option<User>, AuthError> {
        Ok(Some(User))
    }

    async fn check_ban(&self, _id: &i32) -> Result<BanStatus, AuthError> {
        Ok(BanStatus::NotBanned)
    }

    async fn verify_password(&self, _id: &i32, _password: &str) -> Result<bool, AuthError> {
        Ok(false)
    }
}

#[async_trait]
impl SessionRevocation for Backend {}

async fn anonymous() -> AuthSession<Backend> {
    AuthSession::new(
        Backend,
        Session::new(None, Arc::new(MemoryStore::default()), None),
    )
    .await
}

#[tokio::test]
async fn implicit_verification_and_role_requirements_reject_anonymous() {
    for requirement in [
        auth_requirements().verified(),
        auth_requirements().unverified(),
        auth_requirements().role_min(3),
    ] {
        let error = check_requirements(&mut anonymous().await, &requirement)
            .await
            .unwrap_err();
        assert_eq!(error.code(), AuthErrorCode::Unauthenticated);
    }
}

#[tokio::test]
async fn contradictory_anonymous_and_role_requirements_fail_closed() {
    let requirement = auth_requirements().unauthenticated().role_min(3);
    assert!(check_requirements(&mut anonymous().await, &requirement)
        .await
        .is_err());
}

#[tokio::test]
async fn role_requirement_accepts_authenticated_user_at_required_level() {
    let mut auth = anonymous().await;
    auth.login(&User).await.unwrap();
    assert!(
        check_requirements(&mut auth, &auth_requirements().role_min(0))
            .await
            .is_ok()
    );
    assert_eq!(
        check_requirements(&mut auth, &auth_requirements().role_min(3))
            .await
            .unwrap_err()
            .code(),
        AuthErrorCode::InsufficientRole
    );
}
