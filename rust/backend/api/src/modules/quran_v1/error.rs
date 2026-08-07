
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

pub struct QuranApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    detail: Option<Value>,
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

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", msg)
    }

    #[allow(dead_code)]
    pub fn with_detail(mut self, detail: impl Serialize) -> Self {
        self.detail = serde_json::to_value(detail).ok();
        self
    }
}

impl IntoResponse for QuranApiError {
    fn into_response(self) -> Response {
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
            let bytes = axum::body::to_bytes(resp.into_body(), 65_536)
                .await
                .unwrap_or_default();
            if !bytes.is_empty() {
                let mut passthrough = Response::builder()
                    .status(status)
                    .body(Body::from(bytes))
                    .expect("quran passthrough response builds");
                if let Some(allow) = allow {
                    passthrough.headers_mut().insert(header::ALLOW, allow);
                }
                return passthrough;
            }
            match status {
                StatusCode::NOT_FOUND => {
                    QuranApiError::not_found("no such route under /quran").into_response()
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
