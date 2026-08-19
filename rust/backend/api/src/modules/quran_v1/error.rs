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

// W3a typed suspicious-error classification. Attached as a PRIVATE response
// extension before serialization; the outer quran-v1 limiter's escalation engine
// reads this typed extension (never JSON/message text) to decide whether a 4xx
// belongs to the closed suspicious set. Only the closed set carries a variant —
// ordinary not-found content, search validation, 5xx, and successful reads get
// NO extension, so they can never inflate the suspicious counter.
//
// `range_too_large` (a distinct, unambiguous abuse signal — requesting far more
// ayahs than the cap) and the reshaped unknown-route 404 are classified here in
// error.rs. `unknown_source`/invalid-range-from-to are produced in controller
// call sites via `.classified(...)`; the builder is ready for that one-line
// wiring per site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuranErrorClass {
    /// Unknown translation source id requested.
    UnknownSource,
    /// Invalid range bounds (e.g. range_too_large, impossible from/to).
    InvalidRange,
    /// No Quran route matched (bodyless 404 reshaped by shape_routing_errors).
    UnknownRoute,
}

pub struct QuranApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    detail: Option<Value>,
    class: Option<QuranErrorClass>,
}

impl QuranApiError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", msg)
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_input", msg)
    }

    pub fn method_not_allowed(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", msg)
    }

    pub fn range_too_large(requested: u32) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "range_too_large",
            message: format!(
                "range_too_large: requested {requested} ayahs but the max is {RESPONSE_CAP}; \
                 paginate with ?cursor=<globalIndex>&limit=<={RESPONSE_CAP}>"
            ),
            detail: Some(json!({ "max": RESPONSE_CAP, "requested": requested })),
            // A sustained flood of over-cap range requests is the canonical
            // suspicious shape W3a escalates on; ordinary not-found is not.
            class: Some(QuranErrorClass::InvalidRange),
        }
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            detail: None,
            class: None,
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", msg)
    }

    #[allow(dead_code)]
    pub fn with_detail(mut self, detail: impl Serialize) -> Self {
        self.detail = serde_json::to_value(detail).ok();
        self
    }

    /// Tag this error as belonging to the W3a suspicious closed set. Call sites
    /// that build a suspicious 4xx (unknown source id, invalid range bounds) use
    /// this so the escalation engine counts it without parsing message text.
    pub fn classified(mut self, class: QuranErrorClass) -> Self {
        self.class = Some(class);
        self
    }
}

// 5xx detail (sqlx text, server file paths) must stay tracing-only; release
// bodies carry a generic message. 4xx messages are safe, purpose-built strings.
fn render_message(status: StatusCode, detail: &str) -> String {
    if status.is_server_error() && !cfg!(debug_assertions) {
        "internal error".to_string()
    } else {
        detail.to_string()
    }
}

impl IntoResponse for QuranApiError {
    fn into_response(self) -> Response {
        let class = self.class;
        if self.status.is_server_error() {
            tracing::error!(
                code = self.code,
                message = %self.message,
                "quran internal error returned"
            );
        }
        let message = render_message(self.status, &self.message);
        let body = json!({
            "error": {
                "code": self.code,
                "message": message,
                "detail": self.detail,
            }
        });
        let bytes = serde_json::to_vec(&body).expect("quran error serializes");
        let mut resp = Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(bytes))
            .expect("quran error response builds");
        // Attach the typed classification BEFORE the body is finalized so the
        // outer limiter (outside this router) can read it off the response. The
        // engine consumes this extension; it never inspects status/JSON/text.
        if let Some(class) = class {
            resp.extensions_mut().insert(class);
        }
        // 5xx gets no-store so a CDN can't pin a transient failure.
        if self.status.is_server_error() {
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
        resp
    }
}

pub async fn shape_routing_errors(req: Request, next: Next) -> Response {
    let resp = next.run(req).await;
    let status = resp.status();
    match status {
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
            let allow = resp.headers().get(header::ALLOW).cloned();
            let hdrs = resp.headers().clone();
            let bytes = axum::body::to_bytes(resp.into_body(), 65_536)
                .await
                .unwrap_or_default();
            if !bytes.is_empty() {
                let mut passthrough = Response::builder()
                    .status(status)
                    .body(Body::from(bytes))
                    .expect("quran passthrough response builds");
                *passthrough.headers_mut() = hdrs;
                if let Some(allow) = allow {
                    passthrough.headers_mut().insert(header::ALLOW, allow);
                }
                return passthrough;
            }
            match status {
                // Bodyless 404 = no Quran route matched (axum's default). This is
                // the "unknown Quran route" member of the W3a suspicious closed
                // set: a sustained flood of route-probing is abuse-shaped, so tag
                // it. Ordinary not-found CONTENT (surah/ayah) already carries a
                // JSON body and passthroughs above without this extension.
                StatusCode::NOT_FOUND => {
                    let mut resp = QuranApiError::not_found("no such route under /quran")
                        .classified(QuranErrorClass::UnknownRoute)
                        .into_response();
                    if let Some(allow) = allow {
                        resp.headers_mut().insert(header::ALLOW, allow);
                    }
                    resp
                }
                StatusCode::METHOD_NOT_ALLOWED => {
                    let mut shaped = QuranApiError::method_not_allowed(
                        "method not allowed; accepted methods: GET, HEAD, OPTIONS",
                    )
                    .into_response();
                    if let Some(allow) = allow {
                        shaped.headers_mut().insert(header::ALLOW, allow);
                    }
                    shaped
                }
                _ => unreachable!(),
            }
        }
        _ => resp,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;
    #[test]
    fn range_too_large_carries_invalid_range_class() {
        let resp = QuranApiError::range_too_large(999_999).into_response();
        assert_eq!(
            resp.extensions().get::<QuranErrorClass>(),
            Some(&QuranErrorClass::InvalidRange),
            "range_too_large must be classified InvalidRange"
        );
    }

    #[test]
    fn classified_builder_tags_unknown_source() {
        let resp = QuranApiError::invalid("unknown source 'x'")
            .classified(QuranErrorClass::UnknownSource)
            .into_response();
        assert_eq!(
            resp.extensions().get::<QuranErrorClass>(),
            Some(&QuranErrorClass::UnknownSource)
        );
    }

    #[test]
    fn ordinary_not_found_carries_no_class() {
        // Ordinary not-found CONTENT (surah/ayah not found) must NOT be tagged —
        // it is excluded from the suspicious closed set.
        let resp = QuranApiError::not_found("surah 999 not found").into_response();
        assert!(resp.extensions().get::<QuranErrorClass>().is_none());
    }

    #[test]
    fn plain_invalid_and_5xx_carry_no_class() {
        let invalid = QuranApiError::invalid("malformed query").into_response();
        assert!(invalid.extensions().get::<QuranErrorClass>().is_none());
        let server_err = QuranApiError::internal("boom").into_response();
        assert!(server_err.extensions().get::<QuranErrorClass>().is_none());
    }

    #[test]
    fn client_error_messages_are_never_redacted() {
        assert_eq!(
            render_message(StatusCode::NOT_FOUND, "surah 999 not found"),
            "surah 999 not found"
        );
        assert_eq!(
            render_message(StatusCode::BAD_REQUEST, "malformed query"),
            "malformed query"
        );
    }

    #[test]
    fn server_error_detail_stays_out_of_release_bodies() {
        let detail = "error ReturnedErr(| IO: /srv/quran/db.sqlite not found)";
        if cfg!(debug_assertions) {
            assert_eq!(
                render_message(StatusCode::INTERNAL_SERVER_ERROR, detail),
                detail
            );
        } else {
            assert_eq!(
                render_message(StatusCode::INTERNAL_SERVER_ERROR, detail),
                "internal error"
            );
            assert!(!render_message(StatusCode::INTERNAL_SERVER_ERROR, detail).contains("/srv"));
        }
    }

    #[tokio::test]
    async fn shape_routing_errors_tags_bodyless_404_as_unknown_route() {
        use axum::routing::get;
        // A router with no matching route yields a bodyless 404 that
        // shape_routing_errors reshapes and must tag UnknownRoute.
        let app = axum::Router::new()
            .route("/exists", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(shape_routing_errors));
        let req = Request::builder()
            .uri("/nope/not/a/route")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.extensions().get::<QuranErrorClass>(),
            Some(&QuranErrorClass::UnknownRoute)
        );
    }
}
