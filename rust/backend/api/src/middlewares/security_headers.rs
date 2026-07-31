use axum::{
    extract::Request,
    http::{header, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

const NOSNIFF: &str = "nosniff";
const DENY: &str = "DENY";
const REFERRER_POLICY: &str = "strict-origin-when-cross-origin";
const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=()";
const XSS_PROTECTION: &str = "0";

const DEFAULT_HSTS: &str = "max-age=31536000; includeSubDomains";

const DEFAULT_CSP: &str = "default-src 'self'; \
    script-src 'self' 'wasm-unsafe-eval' 'unsafe-eval'; \
    style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
    font-src 'self' https://fonts.gstatic.com; \
    img-src 'self' data: https:; \
    media-src 'self'; \
    connect-src 'self'; \
    object-src 'none'; \
    base-uri 'self'; \
    frame-ancestors 'none'; \
    form-action 'self'";

fn hsts_header_value() -> Option<HeaderValue> {
    let raw = std::env::var("HSTS_HEADER").unwrap_or_else(|_| DEFAULT_HSTS.to_string());
    if raw.is_empty() {
        return None;
    }
    HeaderValue::from_str(&raw).ok()
}

fn csp_header_value() -> Option<HeaderValue> {
    let raw = std::env::var("CONTENT_SECURITY_POLICY").unwrap_or_else(|_| DEFAULT_CSP.to_string());
    if raw.is_empty() {
        return None;
    }
    HeaderValue::from_str(&raw).ok()
}

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static(NOSNIFF),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static(DENY));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static(REFERRER_POLICY),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    headers.insert("x-xss-protection", HeaderValue::from_static(XSS_PROTECTION));

    if let Some(hsts) = hsts_header_value() {
        headers.insert(header::STRICT_TRANSPORT_SECURITY, hsts);
    }
    if let Some(csp) = csp_header_value() {
        headers.insert(header::CONTENT_SECURITY_POLICY, csp);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn ok() -> &'static str {
        "ok"
    }

    async fn run_headers() -> axum::http::HeaderMap {
        let app = axum::Router::new()
            .route("/", axum::routing::get(ok))
            .layer(axum::middleware::from_fn(security_headers));
        let res = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        res.headers().clone()
    }

    #[tokio::test]
    async fn hsts_and_csp_present_with_secure_defaults() {
        let h = run_headers().await;
        let hsts = h
            .get(header::STRICT_TRANSPORT_SECURITY)
            .expect("HSTS header present")
            .to_str()
            .unwrap();
        assert!(hsts.contains("max-age=31536000"));
        assert!(hsts.contains("includeSubDomains"));
        let csp = h
            .get(header::CONTENT_SECURITY_POLICY)
            .expect("CSP header present")
            .to_str()
            .unwrap();
        assert!(
            csp.contains("script-src 'self' 'wasm-unsafe-eval' 'unsafe-eval'"),
            "script-src must be 'self' + wasm/eval keywords, got: {csp}"
        );
        let script_src = csp
            .split("script-src ")
            .nth(1)
            .and_then(|s| s.split(';').next())
            .unwrap_or("");
        assert!(
            !script_src.contains("'unsafe-inline'"),
            "script-src must NEVER grant 'unsafe-inline' (stored-XSS invariant): {csp}"
        );
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }
}
