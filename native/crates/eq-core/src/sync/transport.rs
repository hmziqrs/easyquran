//! The seam between the sync engine and the API client (owned by `api/`).
//! The API client implements [`SyncPost`] over its session transport
//! (`x-eq-session`, CSRF, base URL). The trait shape is fixed so the lead can
//! unify it with the client's own: a JSON body in, the parsed 2xx JSON body
//! out, every failure a [`NetError`].
//!
//! A non-2xx answer must reach the engine with its status, `Retry-After` and
//! body, because the status table (contracts/sync.md) acts on each. Until
//! `NetError` grows an HTTP variant, the client encodes it with
//! [`SyncHttpError::into_net_error`] and the engine reads it back with
//! [`SyncHttpError::from_net_error`].

use serde_json::{Value, json};

use crate::net::{NetError, NetFuture};

/// POSTs a sync body to an API path (`/bookmark/v1/sync`, ...).
pub trait SyncPost {
    fn post_json(&self, path: &str, body: Value) -> NetFuture<'_, Value>;
}

/// GETs an API path (`/reading-session/v1/archive?...`). Only the thread
/// history pages need it.
pub trait SyncGet {
    fn get_json(&self, path: &str) -> NetFuture<'_, Value>;
}

/// A response with a non-2xx status.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncHttpError {
    pub status: u16,
    /// The `Retry-After` header in seconds, else the body's `retryAfter`.
    pub retry_after: Option<u64>,
    /// The parsed body, or `null` when it was not JSON.
    pub body: Value,
}

const MARKER: &str = "eq-sync-http ";

impl SyncHttpError {
    pub fn new(status: u16, retry_after: Option<u64>, body: Value) -> Self {
        Self {
            status,
            retry_after,
            body,
        }
    }

    /// Encodes the error as `NetError::Refused` with a stable marker. The body
    /// carries the server's error object only (never a session or CSRF value).
    pub fn into_net_error(self) -> NetError {
        let payload = json!({
            "status": self.status,
            "retryAfter": self.retry_after,
            "body": self.body,
        });
        NetError::Refused(format!("{MARKER}{payload}"))
    }

    /// The HTTP error a [`SyncPost`] encoded, or `None` for a network error.
    pub fn from_net_error(error: &NetError) -> Option<Self> {
        let NetError::Refused(text) = error else {
            return None;
        };
        let payload: Value = serde_json::from_str(text.strip_prefix(MARKER)?).ok()?;
        let status = u16::try_from(payload.get("status")?.as_u64()?).ok()?;
        let body = payload.get("body").cloned().unwrap_or(Value::Null);
        let retry_after = payload
            .get("retryAfter")
            .and_then(Value::as_u64)
            .or_else(|| body.get("retryAfter").and_then(Value::as_u64));
        Some(Self {
            status,
            retry_after,
            body,
        })
    }

    /// The server's `message`, when the build sends one.
    pub fn message(&self) -> Option<&str> {
        self.body.get("message").and_then(Value::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_errors_round_trip_through_net_error() {
        let err = SyncHttpError::new(429, Some(120), json!({"type": "RateLimited"}));
        let back = SyncHttpError::from_net_error(&err.clone().into_net_error()).unwrap();
        assert_eq!(back, err);
        let body_only = SyncHttpError::new(429, None, json!({"retryAfter": 30}));
        let back = SyncHttpError::from_net_error(&body_only.into_net_error()).unwrap();
        assert_eq!(back.retry_after, Some(30));
        assert_eq!(
            SyncHttpError::from_net_error(&NetError::Offline("dns".into())),
            None
        );
    }
}
