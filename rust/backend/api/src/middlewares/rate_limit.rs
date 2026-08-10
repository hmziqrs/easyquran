use axum::response::IntoResponse;

use crate::error::{ErrorCode, ErrorResponse};
use crate::state::AppState;

pub use rux_request_gate::{PathKey, RateLimitLayer};

fn blocked_response(info: rux_request_gate::BlockInfo) -> axum::response::Response {
    ErrorResponse::new(ErrorCode::RateLimited)
        .with_message(format!(
            "Too many requests. Try again in {} seconds.",
            info.retry_after_secs
        ))
        .with_retry_after(info.retry_after_secs)
        .into_response()
}

pub fn rate_limit_layer(state: &AppState, max_requests: u64, window_secs: u64) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), max_requests, window_secs)
        .on_block(blocked_response)
        .build()
}

/// IP-only (path-independent) key required: per-path keying lets parameterized routes (e.g. /ayahs/{s}/{a} × 6236) fan into thousands of buckets, bypassing the per-IP ceiling.
pub fn rate_limit_layer_branch(
    state: &AppState,
    max_requests: u64,
    window_secs: u64,
    fixed_label: &'static str,
) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), max_requests, window_secs)
        .path_key(PathKey::Fixed(fixed_label))
        .on_block(blocked_response)
        .build()
}

fn identity_is_internal(req: &axum::extract::Request) -> bool {
    matches!(
        req.extensions().get::<rux_request_gate::RequestIdentity>(),
        Some(rux_request_gate::RequestIdentity::InternalService(_))
    )
}

fn is_health_route(req: &axum::extract::Request) -> bool {
    let p = req.uri().path();
    p == "/healthz" || p.ends_with("/health/ready")
}

/// External content ceiling for the public Quran router. Applies only to
/// external identities on non-health routes; internal service + health each have
/// their own isolated, non-escalating buckets.
pub fn quran_content_layer(state: &AppState) -> RateLimitLayer {
    RateLimitLayer::builder(state.gate_store.clone(), 600, 60)
        .path_key(PathKey::Fixed("quran-v1"))
        .applies(|req| !is_health_route(req) && !identity_is_internal(req))
        .on_block(blocked_response)
        .build()
}

/// Trusted Docker-internal SSR (Bun) bucket. Separate non-escalating policy; the
/// internal identity is a fixed non-IP label that never shares an external IP
/// bucket and can never enter W3a IP escalation.
pub fn quran_internal_layer(state: &AppState) -> RateLimitLayer {
    let max = state
        .settings
        .rate_limit
        .internal_requests_per_minute
        .max(1);
    RateLimitLayer::builder(state.gate_store.clone(), max, 60)
        .path_key(PathKey::Fixed("quran-v1"))
        .applies(identity_is_internal)
        .on_block(blocked_response)
        .build()
}

/// Public readiness bucket. Identity-independent (health is exempt from identity
/// resolution, so health checks key into one isolated non-IP bucket); never
/// shares content or escalation state.
pub fn quran_health_layer(state: &AppState) -> RateLimitLayer {
    let max = state.settings.rate_limit.health_requests_per_minute.max(1);
    RateLimitLayer::builder(state.gate_store.clone(), max, 60)
        .path_key(PathKey::Fixed("quran-health"))
        .applies(is_health_route)
        .on_block(blocked_response)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rux_request_gate::{InMemoryStore, InternalServiceId, RateLimitStore, RequestIdentity};
    use std::net::IpAddr;
    use std::sync::Arc;
    use tower::{Layer, Service, ServiceExt};

    // Tiny max so buckets exhaust in 3 requests; shared store so cross-bucket
    // interference is observable at both the status and the key level.
    const MAX: u64 = 2;

    // Always-200 inner service: the limiter's allow/block decision is the only
    // variable under test.
    #[derive(Clone)]
    struct OkService;
    impl Service<axum::extract::Request> for OkService {
        type Response = axum::response::Response;
        type Error = std::convert::Infallible;
        type Future = std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
        >;
        fn poll_ready(
            &mut self,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: axum::extract::Request) -> Self::Future {
            Box::pin(async { Ok(axum::response::Response::new(axum::body::Body::empty())) })
        }
    }

    // Mirror the three public quran_*_layer builders exactly (same applies
    // predicates, same Fixed path keys) but with a small max. The real builders
    // need a full AppState; these exercise the SAME predicates against the same
    // RateLimitLayer so isolation is proven at the layer that production uses.
    fn content_layer(store: Arc<dyn RateLimitStore>) -> RateLimitLayer {
        RateLimitLayer::builder(store, MAX, 60)
            .path_key(PathKey::Fixed("quran-v1"))
            .applies(|req| !is_health_route(req) && !identity_is_internal(req))
            .build()
    }
    fn internal_layer(store: Arc<dyn RateLimitStore>) -> RateLimitLayer {
        RateLimitLayer::builder(store, MAX, 60)
            .path_key(PathKey::Fixed("quran-v1"))
            .applies(identity_is_internal)
            .build()
    }
    fn health_layer(store: Arc<dyn RateLimitStore>) -> RateLimitLayer {
        RateLimitLayer::builder(store, MAX, 60)
            .path_key(PathKey::Fixed("quran-health"))
            .applies(is_health_route)
            .build()
    }

    fn ext_ip() -> IpAddr {
        "203.0.113.5".parse().unwrap()
    }
    fn request(path: &str, identity: RequestIdentity) -> axum::extract::Request {
        let mut r = axum::http::Request::builder()
            .method("GET")
            .uri(path)
            .body(axum::body::Body::empty())
            .unwrap();
        r.extensions_mut().insert(identity);
        r
    }
    fn external_req() -> axum::extract::Request {
        request("/quran/v1/surahs", RequestIdentity::External(ext_ip()))
    }
    fn internal_req() -> axum::extract::Request {
        request(
            "/quran/v1/surahs",
            RequestIdentity::InternalService(InternalServiceId::WebSsr),
        )
    }
    // Health check carries the SAME external IP as external_req: proving the
    // content vs health split is by path key, not by identity.
    fn health_req() -> axum::extract::Request {
        request("/healthz", RequestIdentity::External(ext_ip()))
    }

    use axum::http::StatusCode;

    #[test]
    fn applies_predicates_route_each_request_to_exactly_one_bucket() {
        // Health route detection: exact path + ready suffix.
        assert!(is_health_route(&request(
            "/healthz",
            RequestIdentity::External(ext_ip())
        )));
        assert!(is_health_route(&request(
            "/quran/health/ready",
            RequestIdentity::External(ext_ip())
        )));
        assert!(!is_health_route(&external_req()));

        // Internal-service identity detection keys off the RequestIdentity
        // extension, not the path or IP.
        assert!(identity_is_internal(&internal_req()));
        assert!(!identity_is_internal(&external_req()));
        assert!(!identity_is_internal(&health_req()));

        // An external non-health request satisfies ONLY the content predicate.
        let e = external_req();
        assert!(!is_health_route(&e) && !identity_is_internal(&e));
        // An internal request satisfies ONLY the internal predicate: it is on a
        // non-health path (so health is false) but is internal (content is false).
        let i = internal_req();
        assert!(!is_health_route(&i) && identity_is_internal(&i));
        // A health request satisfies ONLY the health predicate.
        let h = health_req();
        assert!(is_health_route(&h) && !identity_is_internal(&h));
    }

    fn count_for(mem: &Arc<InMemoryStore>, key: &str) -> u64 {
        mem.snapshot()
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.fixed_count)
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn content_fill_does_not_deplete_internal_or_health() {
        let mem = Arc::new(InMemoryStore::default());
        let store = mem.clone() as Arc<dyn RateLimitStore>;
        let svc = content_layer(store.clone()).layer(
            internal_layer(store.clone()).layer(health_layer(store.clone()).layer(OkService)),
        );

        // Exhaust the CONTENT bucket (max=2): two pass, third is 429.
        assert_eq!(
            svc.clone().oneshot(external_req()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            svc.clone().oneshot(external_req()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            svc.clone().oneshot(external_req()).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS,
            "3rd external request must exhaust the content bucket"
        );

        // Internal bucket is independent: an internal request still passes.
        assert_eq!(
            svc.clone().oneshot(internal_req()).await.unwrap().status(),
            StatusCode::OK,
            "internal bucket must be untouched after content fill"
        );
        // Health bucket is independent (same IP, different path key): still passes.
        assert_eq!(
            svc.clone().oneshot(health_req()).await.unwrap().status(),
            StatusCode::OK,
            "health bucket must be untouched after content fill (same IP, separate key)"
        );

        // Key-level evidence: content key reached 3; internal/health stayed at 1.
        assert_eq!(count_for(&mem, "ratelimit:203.0.113.5/32:quran-v1"), 3);
        assert_eq!(count_for(&mem, "ratelimit:internal-webssr:quran-v1"), 1);
        assert_eq!(count_for(&mem, "ratelimit:203.0.113.5/32:quran-health"), 1);
    }

    #[tokio::test]
    async fn internal_fill_does_not_deplete_content_or_health() {
        let mem = Arc::new(InMemoryStore::default());
        let store = mem.clone() as Arc<dyn RateLimitStore>;
        let svc = content_layer(store.clone()).layer(
            internal_layer(store.clone()).layer(health_layer(store.clone()).layer(OkService)),
        );

        // Exhaust the INTERNAL bucket.
        assert_eq!(
            svc.clone().oneshot(internal_req()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            svc.clone().oneshot(internal_req()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            svc.clone().oneshot(internal_req()).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS,
            "3rd internal request must exhaust the internal bucket only"
        );

        // Content bucket untouched (different identity key: external IP vs
        // internal-webssr, even though the Fixed path label is shared).
        assert_eq!(
            svc.clone().oneshot(external_req()).await.unwrap().status(),
            StatusCode::OK,
            "content bucket must be untouched after internal fill"
        );
        // Health bucket untouched.
        assert_eq!(
            svc.clone().oneshot(health_req()).await.unwrap().status(),
            StatusCode::OK,
            "health bucket must be untouched after internal fill"
        );

        assert_eq!(count_for(&mem, "ratelimit:internal-webssr:quran-v1"), 3);
        assert_eq!(count_for(&mem, "ratelimit:203.0.113.5/32:quran-v1"), 1);
        assert_eq!(count_for(&mem, "ratelimit:203.0.113.5/32:quran-health"), 1);
    }

    #[tokio::test]
    async fn health_fill_does_not_deplete_content_or_internal() {
        let mem = Arc::new(InMemoryStore::default());
        let store = mem.clone() as Arc<dyn RateLimitStore>;
        let svc = content_layer(store.clone()).layer(
            internal_layer(store.clone()).layer(health_layer(store.clone()).layer(OkService)),
        );

        // Exhaust the HEALTH bucket.
        assert_eq!(
            svc.clone().oneshot(health_req()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            svc.clone().oneshot(health_req()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            svc.clone().oneshot(health_req()).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS,
            "3rd health request must exhaust the health bucket only"
        );

        // Content bucket untouched (same external IP, but path key quran-v1 !=
        // quran-health, so the count is isolated).
        assert_eq!(
            svc.clone().oneshot(external_req()).await.unwrap().status(),
            StatusCode::OK,
            "content bucket must be untouched after health fill (same IP, separate path key)"
        );
        // Internal bucket untouched.
        assert_eq!(
            svc.clone().oneshot(internal_req()).await.unwrap().status(),
            StatusCode::OK,
            "internal bucket must be untouched after health fill"
        );

        assert_eq!(count_for(&mem, "ratelimit:203.0.113.5/32:quran-health"), 3);
        assert_eq!(count_for(&mem, "ratelimit:203.0.113.5/32:quran-v1"), 1);
        assert_eq!(count_for(&mem, "ratelimit:internal-webssr:quran-v1"), 1);
    }
}
