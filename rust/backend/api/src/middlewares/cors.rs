use axum::{extract::Request, middleware::Next, response::Response};
use tracing::{instrument, warn};

use crate::error::CorsError;
use crate::utils::cors::AllowedOrigins;

#[instrument(skip(req, next), fields(origin))]
pub async fn origin_guard(req: Request, next: Next) -> Result<Response, CorsError> {
    let origin_header = match req.headers().get(axum::http::header::ORIGIN) {
        None => return Ok(next.run(req).await),
        Some(header) => header,
    };

    let origin_str = origin_header.to_str().unwrap_or("<invalid>").to_string();
    tracing::Span::current().record("origin", &*origin_str);

    // The allowed set is built once at boot and attached as a request extension
    // OUTSIDE this middleware; origin_guard never reads env or parses per request.
    // A missing extension fails closed (rejects) — it can only occur if the boot
    // wiring forgot the Extension layer, which would be visible immediately.
    let is_allowed = req
        .extensions()
        .get::<AllowedOrigins>()
        .map(|allowed| allowed.contains_header(origin_header))
        .unwrap_or(false);

    if is_allowed {
        Ok(next.run(req).await)
    } else {
        warn!(origin = %origin_str, "Origin not allowed by CORS");
        Err(CorsError::OriginNotAllowed { origin: origin_str })
    }
}

// Every private API router response is uncacheable by browsers and Cache Storage.
// This does NOT infer authentication from `ruxlog.sid` — CSRF generation mints that
// cookie for anonymous sessions too — so it applies unconditionally. The public
// Quran router never enters this middleware and keeps its immutable/public policy.
pub async fn private_no_store(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    resp
}
