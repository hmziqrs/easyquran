
use axum::extract::{FromRequestParts, Request};
use axum::middleware::Next;
use axum::response::Response;

pub async fn resolve_client_ip(mut req: Request, next: Next) -> Response {
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

    #[tokio::test]
    async fn resolver_publishes_cf_connecting_ip_to_extension() {
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
