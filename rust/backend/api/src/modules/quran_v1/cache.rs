//! Conditional-GET + cache headers for the Quran API (§8.1).
//!
//! The ETag is **weak** and derived from `contentVersion + canonical-resource-key`
//! (not the body bytes) so a CDN can validate cached JSON without re-reading it,
//! and so two requests differing only in `script` do not collide on one ETag.
//! The shared compression layer varies the encoded bytes for an unchanged
//! representation, which is legal for a weak validator and illegal for a strong
//! one — hence `W/`.

use axum::body::Body;
use axum::http::{header, HeaderMap, Response, StatusCode};
use serde::Serialize;

/// Arabic content cache (§8.1): short browser TTL (unpurgeable), long shared
/// TTL (CDN-purgeable) + stale windows.
pub const ARABIC_CACHE: &str =
    "public, max-age=300, s-maxage=86400, stale-while-revalidate=604800, stale-if-error=604800";

/// Search cache (§8.1): pure function of (contentVersion, searchVersion, q,
/// script, limit, offset).
pub const SEARCH_CACHE: &str = "public, max-age=60, s-maxage=300";

/// Pinned-by-date resources (§8.5: a supplied `date` is immutable).
pub const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

/// Operational endpoints (§6.4, §8.4): a CDN must never pin a failure or a
/// readiness probe.
pub const NO_STORE: &str = "no-store";

const VARY: &str = "Accept-Encoding";

/// `W/"<content-version>:<canonical-resource-key>"` (§8.1).
pub fn weak_etag(content_version: &str, canonical_key: &str) -> String {
    format!("W/\"{content_version}:{canonical_key}\"")
}

/// Compare an `If-None-Match` header value against an ETag, tolerating the weak
/// `W/` prefix, comma-separated lists, and `*` (RFC 9110 §8.8.3/§8.8.1).
pub fn etag_matches(if_none_match: &str, etag: &str) -> bool {
    let trimmed = if_none_match.trim();
    if trimmed == "*" {
        return true;
    }
    let target = normalize_tag(etag);
    trimmed.split(',').any(|t| normalize_tag(t) == target)
}

fn normalize_tag(t: &str) -> &str {
    t.trim().trim_start_matches("W/").trim_start_matches("w/")
}

pub fn if_none_match(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
}

/// Serialize `body` and respond 200 — or 304 if the client's `If-None-Match`
/// matches. Both carry `ETag`, `Cache-Control`, and `Vary: Accept-Encoding`; a
/// `Vary` mismatch between 200 and 304 corrupts shared caches (§8.1).
pub fn respond_cached<T: Serialize + ?Sized>(
    body: &T,
    content_version: &str,
    canonical_key: &str,
    cache_control: &str,
    inm: Option<&str>,
) -> Response<Body> {
    let etag = weak_etag(content_version, canonical_key);
    if let Some(inm) = inm {
        if etag_matches(inm, &etag) {
            return Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::ETAG, etag.as_str())
                .header(header::CACHE_CONTROL, cache_control)
                .header(header::VARY, VARY)
                .body(Body::empty())
                .expect("304 response builds");
        }
    }
    let bytes = serde_json::to_vec(body).expect("quran response serializes");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::ETAG, etag.as_str())
        .header(header::CACHE_CONTROL, cache_control)
        .header(header::VARY, VARY)
        .body(Body::from(bytes))
        .expect("200 response builds")
}

/// Like [`respond_cached`] but with an explicit, caller-supplied ETag (used by
/// `/search`, whose ETag folds in `searchVersion` as well as `contentVersion`).
pub fn respond_cached_with_etag<T: Serialize + ?Sized>(
    body: &T,
    etag: &str,
    cache_control: &str,
    inm: Option<&str>,
) -> Response<Body> {
    if let Some(inm) = inm {
        if etag_matches(inm, etag) {
            return Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::ETAG, etag)
                .header(header::CACHE_CONTROL, cache_control)
                .header(header::VARY, VARY)
                .body(Body::empty())
                .expect("304 response builds");
        }
    }
    let bytes = serde_json::to_vec(body).expect("quran response serializes");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::ETAG, etag)
        .header(header::CACHE_CONTROL, cache_control)
        .header(header::VARY, VARY)
        .body(Body::from(bytes))
        .expect("200 response builds")
}
