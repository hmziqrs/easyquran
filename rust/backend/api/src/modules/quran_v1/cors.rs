use std::time::Duration;

use axum::http::{header, HeaderName, Method};
use tower_http::cors::{Any, CorsLayer};

/// Headers are load-bearing for the browser conditional-GET flow: trimming If-None-Match/ETag/Cache-Control/Retry-After silently breaks preflight.
pub fn public_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS])
        .allow_headers([
            header::IF_NONE_MATCH,
            header::IF_MODIFIED_SINCE,
            header::ACCEPT,
            header::ACCEPT_ENCODING,
            header::CACHE_CONTROL,
            HeaderName::from_static("pragma"),
        ])
        .expose_headers([header::ETAG, header::CACHE_CONTROL, header::RETRY_AFTER])
        .allow_credentials(false)
        .max_age(Duration::from_secs(86400))
}
