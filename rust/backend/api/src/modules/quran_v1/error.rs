//! §6.4 error contract for the Quran API: `{"error":{"code","message","detail"}}`.
//!
//! Distinct from the app-wide `ErrorResponse` (which elides `message` in release
//! and uses `type`/`context` naming) so the public Quran contract is stable and
//! self-describing in production, and so every 4xx — including extractor
//! rejections (unknown query param, non-numeric path) AND routing-level 404/405
//! (via [`shape_routing_errors`]) — shares one body shape.

use axum::body::Body;
use axum::extract::{FromRequestParts, Path, Query, Request};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::quran::RESPONSE_CAP;

/// A Quran API error carrying the §6.4 envelope fields.
pub struct QuranApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    detail: Option<Value>,
}

impl QuranApiError {
    /// 404 — unknown Quran identifier/translation ID, or an unknown route (§6.4).
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", msg)
    }

    /// 400 — generic bad request (unknown script/value, bad range/date/query).
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_input", msg)
    }

    /// 405 — method not allowed (§6.4). The `Allow` header is attached by the
    /// caller (axum's 405 already carries it; [`shape_routing_errors`] preserves it).
    pub fn method_not_allowed(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", msg)
    }

    /// 400 `range_too_large` — span exceeds the 300-ayah cap (§6.1/§6.4). The
    /// `{max, requested}` detail rides in `detail` (§6.4).
    pub fn range_too_large(requested: u32) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "range_too_large",
            message: format!(
                "range_too_large: requested {requested} ayahs but the max is {RESPONSE_CAP}; \
                 paginate with ?cursor=<globalIndex>&limit=<={RESPONSE_CAP}>"
            ),
            detail: Some(json!({ "max": RESPONSE_CAP, "requested": requested })),
        }
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// Attach a structured `detail` payload (§6.4).
    #[allow(dead_code)]
    pub fn with_detail(mut self, detail: impl Serialize) -> Self {
        self.detail = serde_json::to_value(detail).ok();
        self
    }
}

impl IntoResponse for QuranApiError {
    fn into_response(self) -> Response {
        // §6.4: `Content-Type: application/json; charset=utf-8` (set explicitly —
        // axum::Json omits the charset).
        let body = json!({
            "error": {
                "code": self.code,
                "message": self.message,
                "detail": self.detail,
            }
        });
        let bytes = serde_json::to_vec(&body).expect("quran error serializes");
        let mut resp = Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(bytes))
            .expect("quran error response builds");
        // §6.4: every 5xx carries Cache-Control: no-store so a CDN cannot pin a failure.
        if self.status.is_server_error() {
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        resp
    }
}

/// Shape routing-level errors into the §6.4 envelope. axum's default 404
/// (unknown route) and 405 (disallowed method) return an empty body; this layer
/// re-wraps them so EVERY error on the public branch shares one body shape,
/// preserving the 405 `Allow` header (§6.4).
pub async fn shape_routing_errors(req: Request, next: Next) -> Response {
    let resp = next.run(req).await;
    match resp.status() {
        StatusCode::NOT_FOUND => {
            QuranApiError::not_found("no such route under /quran/v1").into_response()
        }
        StatusCode::METHOD_NOT_ALLOWED => {
            let allow = resp.headers().get(header::ALLOW).cloned();
            let mut shaped = QuranApiError::method_not_allowed(
                "method not allowed; accepted methods: GET, HEAD, OPTIONS",
            )
            .into_response();
            if let Some(allow) = allow {
                shaped.headers_mut().insert(header::ALLOW, allow);
            }
            shaped
        }
        _ => resp,
    }
}

/// Query extractor that maps ANY rejection (unknown field via
/// `deny_unknown_fields`, or a malformed value) to the §6.4 envelope instead of
/// axum's stock 400 body.
pub struct QQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for QQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QuranApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(v)| QQuery(v))
            .map_err(|_| QuranApiError::invalid("malformed or unknown query parameter"))
    }
}

/// Path extractor that maps a parse failure (e.g. non-numeric `{surah}`) to the
/// §6.4 envelope.
pub struct QPath<T>(pub T);

impl<T, S> FromRequestParts<S> for QPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = QuranApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(v)| QPath(v))
            .map_err(|_| QuranApiError::invalid("malformed path parameter"))
    }
}
