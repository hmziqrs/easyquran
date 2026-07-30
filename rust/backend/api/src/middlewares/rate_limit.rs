//! ruxlog adapter: builds a [`rux_request_gate::RateLimitLayer`] wired with the
//! app's 429 error envelope so the wire contract (ErrorCode::RateLimited /
//! SRV_004 + `Retry-After` + `x-ratelimit-*` headers) is unchanged. The
//! fixed-window Lua, the fail-closed 503, and the IP-spoofing regression tests
//! now live in the crate.

use axum::response::IntoResponse;

use crate::error::{ErrorCode, ErrorResponse};
use crate::state::AppState;

pub use rux_request_gate::{PathKey, RateLimitLayer};

/// Build the per-IP/per-path rate-limit layer for a route.
///
/// The 429 body is built from the app's `ErrorResponse` (SRV_004) via the
/// crate's `on_block` hook, matching the pre-extraction response byte-for-byte.
/// The crate still injects the `x-ratelimit-*` + `Retry-After` headers itself.
pub fn rate_limit_layer(state: &AppState, max_requests: u64, window_secs: u64) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), max_requests, window_secs)
        .on_block(|info| {
            ErrorResponse::new(ErrorCode::RateLimited)
                .with_message(format!(
                    "Too many requests. Try again in {} seconds.",
                    info.retry_after_secs
                ))
                .with_retry_after(info.retry_after_secs)
                .into_response()
        })
        .build()
}

/// Coarse branch-wide rate-limit layer keyed on **IP only** (path-independent)
/// — §8.2: "the existing limiter keys per path, so 'coarse across all Quran
/// routes' needs a path-independent key strategy." Without this, parameterized
/// routes (`/ayahs/{s}/{a}` × 6,236, `/pages/N`, …) fan into ~7,750 independent
/// per-IP buckets and the 600/min ceiling is bypassable by rotating path values.
pub fn rate_limit_layer_branch(
    state: &AppState,
    max_requests: u64,
    window_secs: u64,
    fixed_label: &'static str,
) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), max_requests, window_secs)
        .path_key(PathKey::Fixed(fixed_label))
        .on_block(|info| {
            ErrorResponse::new(ErrorCode::RateLimited)
                .with_message(format!(
                    "Too many requests. Try again in {} seconds.",
                    info.retry_after_secs
                ))
                .with_retry_after(info.retry_after_secs)
                .into_response()
        })
        .build()
}
