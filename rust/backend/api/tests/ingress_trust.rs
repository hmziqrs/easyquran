//! Ingress trust in production: a forged client address must never reach a
//! rate-limit bucket or the escalation ledger. Own test binary because APP_ENV
//! is process-wide (same pattern as `quran_prod_gate.rs`).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{middleware, Extension, Router};
use axum_client_ip::ClientIpSource;
use rux_request_gate::{InMemoryStore, PathKey, RateLimitLayer, RateLimitStore};
use tower::ServiceExt;

use ruxlog::config::settings::EscalationConfig;
use ruxlog::middlewares::client_ip::{
    resolve_client_ip, IngressMarker, InternalApiToken, TrustedProxyCidrs,
};
use ruxlog::services::rate_limit_store::escalation::EscalationEngine;

const MARKER: &str = "0123456789abcdef0123456789abcdef";
const REAL_IP: &str = "198.51.100.7";
const FORGED_IP: &str = "203.0.113.66";
const EDGE_PEER: &str = "172.18.0.5";

static PROD_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_prod_env() {
    PROD_ENV.get_or_init(|| std::env::set_var("APP_ENV", "production"));
}

fn app(store: Arc<InMemoryStore>, peer: &str, limit: u64, escalate: bool) -> Router {
    ensure_prod_env();
    let peer: SocketAddr = format!("{peer}:40000").parse().unwrap();
    let cidrs = TrustedProxyCidrs(Arc::new(vec!["172.16.0.0/12".parse().unwrap()]));
    let escalation = escalate.then(|| {
        std::env::set_var("QURAN_BAN_ESCALATION_ENABLED", "true");
        std::env::set_var("QURAN_BAN_ALLOWLIST", "127.0.0.1/32");
        let cfg = EscalationConfig::from_env(2_000).expect("escalation config");
        let engine: Arc<dyn rux_request_gate::Escalation> =
            Arc::new(EscalationEngine::new(cfg, store.clone()));
        engine
    });
    let limiter = RateLimitLayer::builder(store, limit, 60)
        .path_key(PathKey::Fixed("quran-v1"))
        .build()
        .with_escalation(escalation);
    Router::new()
        .route("/quran", get(|| async { "ok" }))
        .layer(limiter)
        .layer(middleware::from_fn(resolve_client_ip))
        .layer(Extension(IngressMarker(Arc::from(MARKER))))
        .layer(Extension(InternalApiToken(Arc::from("internal-token"))))
        .layer(Extension(cidrs))
        .layer(Extension(ClientIpSource::CfConnectingIp))
        .layer(Extension(ConnectInfo(peer)))
}

fn request(headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder().uri("/quran");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).unwrap()
}

fn bucket_keys(store: &InMemoryStore) -> Vec<String> {
    store.snapshot().into_iter().map(|b| b.key).collect()
}

#[tokio::test]
async fn forged_header_without_marker_is_rejected_and_keys_nothing() {
    let store = Arc::new(InMemoryStore::default());
    let resp = app(store.clone(), EDGE_PEER, 600, false)
        .oneshot(request(&[("cf-connecting-ip", FORGED_IP)]))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(bucket_keys(&store).iter().all(|k| !k.contains(FORGED_IP)));
}

#[tokio::test]
async fn wrong_marker_is_rejected() {
    let store = Arc::new(InMemoryStore::default());
    let wrong = "ffffffffffffffffffffffffffffffff";
    let resp = app(store.clone(), EDGE_PEER, 600, false)
        .oneshot(request(&[
            ("x-eq-ingress-marker", wrong),
            ("cf-connecting-ip", FORGED_IP),
        ]))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(bucket_keys(&store).is_empty());
}

#[tokio::test]
async fn correct_marker_makes_one_bucket_on_the_real_ip() {
    let store = Arc::new(InMemoryStore::default());
    let resp = app(store.clone(), EDGE_PEER, 600, false)
        .oneshot(request(&[
            ("x-eq-ingress-marker", MARKER),
            ("cf-connecting-ip", REAL_IP),
            ("x-forwarded-for", FORGED_IP),
        ]))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let keys = bucket_keys(&store);
    assert_eq!(keys.len(), 1, "{keys:?}");
    assert!(keys[0].contains(REAL_IP), "{keys:?}");
}

#[tokio::test]
async fn forged_floods_never_touch_the_ban_ledger() {
    let store = Arc::new(InMemoryStore::default());
    let router = app(store.clone(), EDGE_PEER, 1, true);
    for _ in 0..25 {
        let resp = router
            .clone()
            .oneshot(request(&[("cf-connecting-ip", FORGED_IP)]))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    let ban_key = format!("quran-ban:{FORGED_IP}/32");
    assert!(store.ban_status(&ban_key).await.unwrap().is_none());
}

#[tokio::test]
async fn peer_outside_cidrs_is_rejected_even_with_the_marker() {
    let store = Arc::new(InMemoryStore::default());
    let resp = app(store.clone(), "203.0.113.200", 600, false)
        .oneshot(request(&[
            ("x-eq-ingress-marker", MARKER),
            ("cf-connecting-ip", REAL_IP),
        ]))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(bucket_keys(&store).is_empty());
}

#[tokio::test]
async fn duplicate_client_ip_headers_fail_closed() {
    let store = Arc::new(InMemoryStore::default());
    let resp = app(store.clone(), EDGE_PEER, 600, false)
        .oneshot(request(&[
            ("x-eq-ingress-marker", MARKER),
            ("cf-connecting-ip", REAL_IP),
            ("cf-connecting-ip", FORGED_IP),
        ]))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(bucket_keys(&store).is_empty());
}

#[tokio::test]
async fn internal_token_needs_a_trusted_peer() {
    let store = Arc::new(InMemoryStore::default());
    let outside = app(store.clone(), "203.0.113.200", 600, false)
        .oneshot(request(&[("x-easyquran-internal-token", "internal-token")]))
        .await
        .unwrap();
    assert_eq!(outside.status(), StatusCode::BAD_REQUEST);
    let inside = app(store, EDGE_PEER, 600, false)
        .oneshot(request(&[("x-easyquran-internal-token", "internal-token")]))
        .await
        .unwrap();
    assert_eq!(inside.status(), StatusCode::OK);
}
