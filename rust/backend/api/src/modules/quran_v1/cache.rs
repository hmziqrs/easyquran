use axum::body::Body;
use axum::http::{header, HeaderMap, Response, StatusCode};
use serde::Serialize;

pub const ARABIC_CACHE: &str =
    "public, max-age=300, s-maxage=86400, stale-while-revalidate=604800, stale-if-error=604800";

pub const SEARCH_CACHE: &str = "public, max-age=60, s-maxage=300";

pub const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

pub const NO_STORE: &str = "no-store";

const VARY: &str = "Accept-Encoding";

/// Weak on purpose: compression varies bytes for an unchanged rep, so a strong validator would corrupt caches.
pub fn weak_etag(content_version: &str, canonical_key: &str) -> String {
    format!("W/\"{content_version}:{canonical_key}\"")
}

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

/// Distinct from `respond_cached`: /search's ETag folds `searchVersion`, not just `contentVersion`; merging cross-pollutes search caches.
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
