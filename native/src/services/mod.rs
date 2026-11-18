//! Service seams for the projects built elsewhere: `http.rs` over NSH's
//! `fetch`, `auth.rs` with a mock provider, `analytics.rs` and
//! `monitoring.rs` as sinks. Owned by the foundation area, except
//! `fake_api.rs`, owned by the data area.
//!
//! Each foundation seam is one small trait with one local implementation:
//! swapping in the real library (`nsh-http`, `nsh-auth`) is editing that
//! one file. The seams do not fake the other projects' depth — no retry
//! policy, no token refresh, no batching — only the calls the app makes.

pub mod analytics;
pub mod auth;
pub mod fake_api;
pub mod http;
pub mod monitoring;
