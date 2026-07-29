//! Resolve the real client IP once and publish it as the
//! `axum_client_ip::ClientIp` extension so every downstream layer (the
//! rate-limit / abuse limiter, metrics, audit) keys on the true client — not the
//! immediate TCP peer (Traefik) or a fallback `"unknown"`.
//!
//! Why this exists: handlers resolve the IP lazily via the `ClientIp` extractor,
//! but a rate-limit middleware runs *before* handler extraction and reads the
//! resolved `ClientIp` extension directly — which was never populated, so the
//! limiter was keyed on `"unknown"`. This layer closes that gap by resolving
//! once (per `IP_SOURCE`) and inserting the extension.
//!
//! Deployment topology: **Cloudflare → Traefik → this backend**. Set
//! `IP_SOURCE=CfConnectingIp` in production so Cloudflare's authoritative
//! `CF-Connecting-IP` header is used. That header is trusted only because the
//! backend is reachable solely via Traefik on a private network — a
//! directly-exposed backend must not trust forwarded headers (they'd be
//! spoofable, letting an attacker bypass per-IP limits).

use axum::extract::{FromRequestParts, Request};
use axum::middleware::Next;
use axum::response::Response;

/// Run the configured `ClientIp` extraction (per `IP_SOURCE`, already installed
/// as the `ClientIpSource` extension by the outer `ip_source` layer) and, on
/// success, insert the resolved [`axum_client_ip::ClientIp`] into request
/// extensions. On failure (no source / unresolvable header) nothing is inserted
/// and downstream consumers fall back to `"unknown"` — fail-safe, never a panic.
pub async fn resolve_client_ip(mut req: Request, next: Next) -> Response {
    // `ClientIp` resolves from request parts (headers or ConnectInfo, per the
    // ClientIpSource config). Resolve it here, insert the result as an extension,
    // then reassemble the request for the downstream service.
    let (mut parts, body) = req.into_parts();
    if let Ok(client_ip) =
        axum_client_ip::ClientIp::from_request_parts(&mut parts, &()).await
    {
        parts.extensions.insert(client_ip);
    }
    req = Request::from_parts(parts, body);
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::{middleware, routing::get, Extension};
    use axum_client_ip::{ClientIp, ClientIpSource};
    use tower::ServiceExt;

    /// The resolver publishes the `ClientIp` *extension* (what the rate-limit
    /// layer reads), derived from Cloudflare's `CF-Connecting-IP` when
    /// `IP_SOURCE=CfConnectingIp`. Extracting `Extension<ClientIp>` succeeds
    /// only if the resolver inserted it — so a 200 + correct body proves the fix.
    #[tokio::test]
    async fn resolver_publishes_cf_connecting_ip_to_extension() {
        let app: Router = Router::new()
            .route(
                "/",
                // Reads the extension the resolver inserts — NOT the extractor.
                get(|Extension(ip): Extension<ClientIp>| async move { ip.0.to_string() }),
            )
            .layer(middleware::from_fn(resolve_client_ip))
            // Mirrors main.rs: the `ip_source` layer sits OUTER to the resolver
            // so the `ClientIpSource` config is present when the resolver runs.
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

    /// Spoofed `X-Forwarded-For` must be ignored when `IP_SOURCE=CfConnectingIp`
    /// — the resolver trusts Cloudflare's header, not the client's.
    #[tokio::test]
    async fn resolver_ignores_spoofed_xff_when_cf_configured() {
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
}
