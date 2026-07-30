//! Public CORS layer for the `/quran/v1` branch (§8.2).

use std::time::Duration;

use axum::http::{header, HeaderName, Method};
use tower_http::cors::{Any, CorsLayer};

/// Wildcard-CORS layer for the public Quran API.
///
/// `Access-Control-Allow-Origin: *` with **no credentials** (a wildcard origin
/// forbids credentials per CORS). GET/HEAD/OPTIONS only. `If-None-Match` is an
/// allowed request header and `ETag` / `Cache-Control` / `Retry-After` are
/// exposed response headers — without these, a browser's conditional GET fails
/// preflight and the JS client cannot read `ETag`, which silently breaks the
/// entire conditional-GET design for the primary consumer (§8.2).
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
        .expose_headers([
            header::ETAG,
            header::CACHE_CONTROL,
            header::RETRY_AFTER,
        ])
        // Mandatory false: a wildcard origin cannot carry credentials.
        .allow_credentials(false)
        .max_age(Duration::from_secs(86400))
}
