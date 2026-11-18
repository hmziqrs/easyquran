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

// Sync goes through `ApiClient` too: it implements
// `sync::transport::{SyncPost, SyncGet}` (session.rs), so every sync call
// carries `x-eq-session` and CSRF and obeys the D6 rules.

#[cfg(test)]
mod tests;
