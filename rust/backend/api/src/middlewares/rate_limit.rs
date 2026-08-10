use axum::response::IntoResponse;

use crate::error::{ErrorCode, ErrorResponse};
use crate::state::AppState;

pub use rux_request_gate::{PathKey, RateLimitLayer};

fn blocked_response(info: rux_request_gate::BlockInfo) -> axum::response::Response {
    ErrorResponse::new(ErrorCode::RateLimited)
        .with_message(format!(
            "Too many requests. Try again in {} seconds.",
            info.retry_after_secs
        ))
        .with_retry_after(info.retry_after_secs)
        .into_response()
}

pub fn rate_limit_layer(state: &AppState, max_requests: u64, window_secs: u64) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), max_requests, window_secs)
        .on_block(blocked_response)
        .build()
}

/// IP-only (path-independent) key required: per-path keying lets parameterized routes (e.g. /ayahs/{s}/{a} × 6236) fan into thousands of buckets, bypassing the per-IP ceiling.
pub fn rate_limit_layer_branch(
    state: &AppState,
    max_requests: u64,
    window_secs: u64,
    fixed_label: &'static str,
) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), max_requests, window_secs)
        .path_key(PathKey::Fixed(fixed_label))
        .on_block(blocked_response)
        .build()
}

fn identity_is_internal(req: &axum::extract::Request) -> bool {
    matches!(
        req.extensions().get::<rux_request_gate::RequestIdentity>(),
        Some(rux_request_gate::RequestIdentity::InternalService(_))
    )
}

fn is_health_route(req: &axum::extract::Request) -> bool {
    let p = req.uri().path();
    p == "/healthz" || p.ends_with("/health/ready")
}

/// External content ceiling for the public Quran router. Applies only to
/// external identities on non-health routes; internal service + health each have
/// their own isolated, non-escalating buckets.
pub fn quran_content_layer(state: &AppState) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), 600, 60)
        .path_key(PathKey::Fixed("quran-v1"))
        .applies(|req| !is_health_route(req) && !identity_is_internal(req))
        .on_block(blocked_response)
        .build()
}

/// Trusted Docker-internal SSR (Bun) bucket. Separate non-escalating policy; the
/// internal identity is a fixed non-IP label that never shares an external IP
/// bucket and can never enter W3a IP escalation.
pub fn quran_internal_layer(state: &AppState) -> RateLimitLayer {
    let max = state
        .settings
        .rate_limit
        .internal_requests_per_minute
        .max(1);
    RateLimitLayer::builder(state.gate_store.clone(), max, 60)
        .path_key(PathKey::Fixed("quran-v1"))
        .applies(identity_is_internal)
        .on_block(blocked_response)
        .build()
}

/// Public readiness bucket. Identity-independent (health is exempt from identity
/// resolution, so health checks key into one isolated non-IP bucket); never
/// shares content or escalation state.
pub fn quran_health_layer(state: &AppState) -> RateLimitLayer {
    let max = state.settings.rate_limit.health_requests_per_minute.max(1);
    RateLimitLayer::builder(state.gate_store.clone(), max, 60)
        .path_key(PathKey::Fixed("quran-health"))
        .applies(is_health_route)
        .on_block(blocked_response)
        .build()
}
