use axum::response::IntoResponse;

use crate::error::{ErrorCode, ErrorResponse};
use crate::state::AppState;

pub use rux_request_gate::{PathKey, RateLimitLayer};

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

/// IP-only (path-independent) key is required: per-path keying lets parameterized
/// routes (e.g. /ayahs/{s}/{a} × 6236) fan into thousands of buckets, bypassing
/// the per-IP ceiling by rotating path values.
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
