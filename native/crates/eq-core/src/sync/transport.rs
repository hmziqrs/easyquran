//! The seam between the sync engine and the API client. `api::ApiClient`
//! implements both traits over its D6 session transport (`x-eq-session`,
//! CSRF, rotation, 401 → session lost): a JSON body in, the parsed 2xx JSON
//! body out, every failure a [`NetError`]. A non-2xx answer arrives as
//! [`NetError::Http`](crate::net::NetError::Http) with its status,
//! `Retry-After` and server `message`, which the status table
//! (contracts/sync.md, `round_status::classify`) acts on.

use serde_json::Value;

use crate::net::NetFuture;

/// POSTs a sync body to an API path (`/bookmark/v1/sync`, ...).
pub trait SyncPost {
    fn post_json(&self, path: &str, body: Value) -> NetFuture<'_, Value>;
}

/// GETs an API path (`/reading-session/v1/archive?...`). Only the thread
/// history pages need it.
pub trait SyncGet {
    fn get_json(&self, path: &str) -> NetFuture<'_, Value>;
}
