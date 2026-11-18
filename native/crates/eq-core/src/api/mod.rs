//! Backend API client (PARITY I1-I14): native session transport
//! (`x-eq-session`, contracts/devices.md D6), CSRF, the error envelope,
//! typed endpoint calls and wire types over [`crate::net::Transport`], and
//! the auth state machine.

use std::{future::Future, pin::Pin};

pub mod auth_state;
pub mod config;
pub mod endpoints;
pub mod error;
pub mod secret;
pub mod session;
pub mod types;

pub use auth_state::{AuthMachine, AuthPhase, SignInStep, TransitionKind};
pub use config::ApiConfig;
pub use endpoints::SessionProbe;
pub use error::{ApiError, BanNotice, ErrorKind, HttpError};
pub use secret::Secret;
pub use session::{ApiClient, ApiReply};

/// A boxed, non-`Send` future of an API call (the NSH engine's futures are
/// not `Send`).
pub type ApiFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ApiError>> + 'a>>;

/// A JSON POST through the session transport (CSRF, `x-eq-session`, 401
/// handling included). The sync slice depends on this, not on
/// [`ApiClient`], so it can be tested with a fake.
pub trait ApiPost {
    /// `POST path` with `body`; the reply body (`Null` when empty).
    fn post_json(&self, path: &str, body: serde_json::Value) -> ApiFuture<'_, serde_json::Value>;
}

#[cfg(test)]
mod tests;
