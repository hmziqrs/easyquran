use axum::extract::{FromRequestParts, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use subtle::ConstantTimeEq;

use rux_request_gate::{InternalServiceId, RequestIdentity};

/// Server-only shared secret used to recognize trusted Docker-internal SSR.
/// Layered as an extension by main.rs; never logged or surfaced publicly.
#[derive(Clone)]
pub struct InternalApiToken(pub std::sync::Arc<str>);

impl std::fmt::Debug for InternalApiToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.0.is_empty() { "unset" } else { "set" };
        f.debug_tuple("InternalApiToken").field(&state).finish()
    }
}

// Docker/localhost healthchecks carry no CF header and no internal token; they
// must never enter identity, rate-limit, or route-blocker state.
fn is_health_route(path: &str) -> bool {
    path == "/healthz" || path.ends_with("/health/ready")
}

fn reject_missing_external_identity(is_prod: bool) -> Response {
    let body = Json(serde_json::json!({
        "error": "missing_client_identity",
        "message": if is_prod {
            "External request missing the required CF-Connecting-IP header (origin ingress must be restricted to Cloudflare ranges)."
        } else {
            "External request identity could not be resolved."
        }
    }));
    (StatusCode::BAD_REQUEST, body).into_response()
}

pub async fn resolve_client_ip(mut req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if is_health_route(&path) {
        return next.run(req).await;
    }

    let internal_token: std::sync::Arc<str> = req
        .extensions()
        .get::<InternalApiToken>()
        .map(|t| t.0.clone())
        .unwrap_or_else(|| std::sync::Arc::<str>::from(""));
    let is_prod = crate::config::settings::is_production().unwrap_or(true);

    // Trusted internal SSR (Bun): constant-time match on the server-only token.
    // Present-but-invalid NEVER grants internal treatment — it falls through to
    // external resolution, which rejects in production without a CF header.
    if let Some(provided) = req.headers().get("x-easyquran-internal-token") {
        let expected = internal_token.as_bytes();
        let matches = !expected.is_empty()
            && provided.as_bytes().len() == expected.len()
            && bool::from(provided.as_bytes().ct_eq(expected));
        if matches {
            req.extensions_mut()
                .insert(RequestIdentity::InternalService(InternalServiceId::WebSsr));
            return next.run(req).await;
        }
        tracing::warn!(
            path = %path,
            "X-EasyQuran-Internal-Token present but invalid; denying internal treatment"
        );
    }

    // External identity via the layered ClientIpSource (CfConnectingIp in prod).
    let (mut parts, body) = req.into_parts();
    match axum_client_ip::ClientIp::from_request_parts(&mut parts, &()).await {
        Ok(client_ip) => {
            let ip = client_ip.0;
            parts.extensions.insert(client_ip);
            parts.extensions.insert(RequestIdentity::External(ip));
            req = Request::from_parts(parts, body);
            next.run(req).await
        }
        Err(e) => {
            if is_prod {
                tracing::warn!(
                    path = %path,
                    error = %e,
                    "External request missing CF-Connecting-IP in production; rejecting"
                );
                return reject_missing_external_identity(is_prod);
            }
            tracing::warn!(
                error = %e,
                "client IP resolution failed in non-production; request uses the 'unknown' bucket"
            );
            req = Request::from_parts(parts, body);
            next.run(req).await
        }
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::{middleware, routing::get, Extension};
    use axum_client_ip::{ClientIp, ClientIpSource};
    use rux_request_gate::RequestIdentity;
    use tower::ServiceExt;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::config::settings::TEST_ENV_MUTEX.lock().unwrap()
    }

    #[tokio::test]
    async fn resolver_publishes_cf_connecting_ip_to_extension() {
        let _g = env_lock();
        let app: Router = Router::new()
            .route(
                "/",
                get(|Extension(ip): Extension<ClientIp>| async move { ip.0.to_string() }),
            )
            .layer(middleware::from_fn(resolve_client_ip))
            .layer(Extension(ClientIpSource::CfConnectingIp));
        let resp = app
            .oneshot(
                Request::builder()
                    .header("cf-connecting-ip", "203.0.113.50")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 32).await.unwrap();
        assert_eq!(&body[..], b"203.0.113.50");
    }

    #[tokio::test]
    async fn resolver_ignores_spoofed_xff_when_cf_configured() {
        let _g = env_lock();
        let app: Router = Router::new()
            .route(
                "/",
                get(|Extension(ip): Extension<ClientIp>| async move { ip.0.to_string() }),
            )
            .layer(middleware::from_fn(resolve_client_ip))
            .layer(Extension(ClientIpSource::CfConnectingIp));
        let resp = app
            .oneshot(
                Request::builder()
                    .header("x-forwarded-for", "1.2.3.4")
                    .header("cf-connecting-ip", "203.0.113.99")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 32).await.unwrap();
        assert_eq!(&body[..], b"203.0.113.99");
    }

    #[tokio::test]
    async fn health_route_is_exempt_from_identity_resolution() {
        let _g = env_lock();
        let app: Router = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .layer(middleware::from_fn(resolve_client_ip))
            .layer(Extension(ClientIpSource::CfConnectingIp));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    fn echo_identity_router() -> Router {
        Router::new()
            .route(
                "/",
                get(|Extension(id): Extension<RequestIdentity>| async move { format!("{id:?}") }),
            )
            .layer(middleware::from_fn(resolve_client_ip))
            .layer(Extension(ClientIpSource::CfConnectingIp))
    }

    #[tokio::test]
    async fn external_identity_resolved_from_cf_header() {
        let _g = env_lock();
        let app = echo_identity_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .header("cf-connecting-ip", "198.51.100.7")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("External"), "got: {s}");
        assert!(s.contains("198.51.100.7"), "got: {s}");
    }

    #[tokio::test]
    async fn forged_token_alone_is_not_trusted_as_internal() {
        let _g = env_lock();
        // No InternalApiToken extension wired → empty expected token → the
        // forged header must not grant internal treatment; resolves as external.
        let app = echo_identity_router();
        let resp = app
            .oneshot(
                Request::builder()
                    .header("x-easyquran-internal-token", "attacker-forged")
                    .header("cf-connecting-ip", "203.0.113.10")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(
            s.contains("External"),
            "forged token must not grant internal: {s}"
        );
    }

    fn router_with_token(token: &str) -> Router {
        echo_identity_router().layer(Extension(InternalApiToken(std::sync::Arc::from(token))))
    }

    #[tokio::test]
    async fn valid_internal_token_assigns_service_identity() {
        let _g = env_lock();
        let app = router_with_token("test-internal-token-secret");
        let resp = app
            .oneshot(
                Request::builder()
                    .header("x-easyquran-internal-token", "test-internal-token-secret")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("InternalService"), "got: {s}");
        assert!(s.contains("WebSsr"), "got: {s}");
    }

    #[tokio::test]
    async fn invalid_internal_token_gets_no_privilege() {
        let _g = env_lock();
        let app = router_with_token("test-internal-token-secret");
        let resp = app
            .oneshot(
                Request::builder()
                    .header("x-easyquran-internal-token", "wrong")
                    .header("cf-connecting-ip", "203.0.113.20")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(
            s.contains("External"),
            "invalid token must fall back to external: {s}"
        );
    }

    #[tokio::test]
    async fn internal_token_compared_constant_time_rejects_close_value() {
        // Same-length wrong token must not match (timing/specificity guard).
        let _g = env_lock();
        let app = router_with_token("abcdef0123456789");
        let resp = app
            .oneshot(
                Request::builder()
                    .header("x-easyquran-internal-token", "abcdef0123456780")
                    .header("cf-connecting-ip", "203.0.113.21")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(
            s.contains("External"),
            "close-but-wrong token must not match: {s}"
        );
    }

    #[tokio::test]
    async fn production_rejects_external_request_missing_cf_header() {
        // RUST_ENV=production flips the gate: an external request without
        // CF-Connecting-IP is rejected (400), never collapsed into 'unknown'.
        let _g = env_lock();
        let prev = std::env::var("RUST_ENV").ok();
        std::env::set_var("RUST_ENV", "production");

        let app = echo_identity_router();

        // Missing CF header → rejected.
        let missing = app
            .clone()
            .oneshot(Request::builder().body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            missing.status(),
            axum::http::StatusCode::BAD_REQUEST,
            "production must reject an external request missing CF-Connecting-IP"
        );

        // With CF header → resolved as External (not rejected).
        let with_cf = app
            .oneshot(
                Request::builder()
                    .header("cf-connecting-ip", "203.0.113.30")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(with_cf.status(), axum::http::StatusCode::OK);

        match prev {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
    }

    #[tokio::test]
    async fn production_invalid_token_without_cf_is_rejected() {
        // An invalid token must not buy internal treatment; without a CF header
        // the request is treated as external and rejected in production.
        let _g = env_lock();
        let prev = std::env::var("RUST_ENV").ok();
        std::env::set_var("RUST_ENV", "production");

        let app = router_with_token("real-secret");
        let resp = app
            .oneshot(
                Request::builder()
                    .header("x-easyquran-internal-token", "wrong-secret")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);

        match prev {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
    }

    #[tokio::test]
    async fn health_route_exemption_holds_in_production() {
        // /healthz and /quran/health/ready stay exempt even in production: the
        // Docker healthcheck carries no CF header and must not be rejected.
        let _g = env_lock();
        let prev = std::env::var("RUST_ENV").ok();
        std::env::set_var("RUST_ENV", "production");

        let app: Router = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route("/quran/health/ready", get(|| async { "ok" }))
            .layer(middleware::from_fn(resolve_client_ip))
            .layer(Extension(ClientIpSource::CfConnectingIp));

        for path in ["/healthz", "/quran/health/ready"] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                axum::http::StatusCode::OK,
                "{path} must be exempt in production"
            );
        }

        match prev {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
    }
}
