//! The auth seam: one small trait so the real provider (Firebase Auth, in
//! another project) replaces [`MockAuthProvider`] in one file. The mock is
//! a fake local user: no network, no account, no credentials.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use nsh_core::element::SharedString;

use crate::foundation::error::AppError;

/// A sign-in in flight, boxed so the trait stays object-safe.
pub type AuthFut<'a> = Pin<Box<dyn Future<Output = Result<AuthUser, AppError>> + 'a>>;

/// Who is signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthUser {
    /// The provider's id for the user (the mock's is always the same).
    pub id: SharedString,
    /// The email they signed in with.
    pub email: SharedString,
    /// What the app shows.
    pub display_name: SharedString,
}

/// The app's identity provider.
pub trait AuthProvider: Send + Sync {
    /// The signed-in user, or `None`.
    fn current_user(&self) -> Option<AuthUser>;

    /// Signs in with `email` and `password`. The failure is an
    /// [`AppError`] with code `"invalid"` (bad input) or the provider's
    /// own; no provider here ever sees a real credential.
    fn sign_in(&self, email: &str, password: &str) -> AuthFut<'_>;

    /// Signs out. After it, [`AuthProvider::current_user`] is `None`.
    fn sign_out(&self);
}

/// The local mock: any well-formed email and a non-empty password signs in
/// a fake user, kept in memory only.
#[derive(Debug, Default)]
pub struct MockAuthProvider {
    user: Mutex<Option<AuthUser>>,
}

impl MockAuthProvider {
    /// The id every mock user gets.
    const MOCK_ID: &'static str = "mock-user-1";

    /// The display name the mock user gets.
    const MOCK_NAME: &'static str = "Mock User";
}

impl AuthProvider for MockAuthProvider {
    fn current_user(&self) -> Option<AuthUser> {
        self.user.lock().expect("mock user").clone()
    }

    fn sign_in(&self, email: &str, password: &str) -> AuthFut<'_> {
        let email = email.trim().to_owned();
        let password = password.to_owned();
        Box::pin(async move {
            if crate::foundation::validation::email_error(&email).is_some() {
                return Err(AppError::new("invalid", "Enter a valid email address"));
            }
            if password.is_empty() {
                return Err(AppError::new("invalid", "Enter a password"));
            }
            let user = AuthUser {
                id: Self::MOCK_ID.into(),
                email: email.into(),
                display_name: Self::MOCK_NAME.into(),
            };
            *self.user.lock().expect("mock user") = Some(user.clone());
            Ok(user)
        })
    }

    fn sign_out(&self) {
        *self.user.lock().expect("mock user") = None;
    }
}

/// The app's shared provider. Swapping the implementation means editing
/// this file's `PROVIDER` line; call sites never change.
pub fn provider() -> Arc<dyn AuthProvider> {
    static PROVIDER: std::sync::LazyLock<Arc<dyn AuthProvider>> =
        std::sync::LazyLock::new(|| Arc::new(MockAuthProvider::default()));
    PROVIDER.clone()
}
