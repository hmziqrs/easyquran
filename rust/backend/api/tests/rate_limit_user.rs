// Subject-keyed write limiters over the real router layers: the per-user
// budget and the IP ceiling both bind, the user bucket is keyed by the verified
// session's user id (never an IP), Retry-After is present and capped, and
// unauthenticated traffic keeps the IP-only layer.
mod reading_common;

use std::net::IpAddr;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

use migration::{Migrator, MigratorTrait};
use reading_common::{login_cookie, router_with_state, seed_user, state};
use rux_request_gate::{InMemoryStore, RequestIdentity};
use ruxlog::middlewares::rate_limit::{
    self, ANALYTICS_IP_MAX, ANALYTICS_USER_MAX, SYNC_IP_MAX, SYNC_USER_MAX,
};
use ruxlog::modules::bookmark_v1;
use ruxlog::state::AppState;

/// Test-only ingress: production `resolve_client_ip` stamps `RequestIdentity`
/// from the proxy chain. Here the client IP comes from `x-test-ip` so one test
/// can make the same user appear on several addresses.
async fn stamp_ip(mut req: Request<Body>, next: Next) -> Response {
    if let Some(ip) = req
        .headers()
        .get("x-test-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<IpAddr>().ok())
    {
        req.extensions_mut().insert(RequestIdentity::External(ip));
    }
    next.run(req).await
}

struct Budget {
    class: &'static str,
    user_max: u64,
    ip_max: u64,
    window_secs: u64,
}

struct Env {
    app: Router,
    mem: Arc<InMemoryStore>,
    cookie_a: String,
    uid_a: i32,
    cookie_b: String,
    uid_b: i32,
}

async fn env(budget: Budget) -> Env {
    let mut st: AppState = state().await;
    Migrator::up(&st.sea_db, None).await.expect("migrations");
    // Own store so the bucket keys are observable.
    let mem = Arc::new(InMemoryStore::default());
    st.gate_store = mem.clone();
    let a = seed_user(&st, "limita").await;
    let b = seed_user(&st, "limitb").await;
    let cookie_a = login_cookie(&st, &a).await;
    let cookie_b = login_cookie(&st, &b).await;
    let routes = rate_limit::authenticated_write_limits(
        bookmark_v1::routes(),
        &st,
        budget.class,
        budget.user_max,
        budget.ip_max,
        budget.window_secs,
    );
    let app = router_with_state(st, routes).layer(middleware::from_fn(stamp_ip));
    Env {
        app,
        mem,
        cookie_a,
        uid_a: a.id,
        cookie_b,
        uid_b: b.id,
    }
}

async fn post_sync(
    app: &Router,
    ip: &str,
    cookie: Option<&str>,
    uid: i32,
) -> (StatusCode, Option<String>, Value) {
    let body = json!({ "expectedUserId": uid, "mutations": [], "since": 0 });
    let mut req = Request::builder()
        .method("POST")
        .uri("/sync")
        .header("content-type", "application/json")
        .header("x-test-ip", ip);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let req = req
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let retry = res
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        retry,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn keys(mem: &InMemoryStore) -> Vec<String> {
    mem.snapshot().into_iter().map(|s| s.key).collect()
}

#[tokio::test]
async fn user_on_two_ips_cannot_exceed_the_user_bucket() {
    let e = env(Budget {
        class: "bookmark:sync",
        user_max: 3,
        ip_max: 30,
        window_secs: 60,
    })
    .await;
    for _ in 0..3 {
        let (s, _, _) = post_sync(&e.app, "198.51.100.1", Some(&e.cookie_a), e.uid_a).await;
        assert_eq!(s, StatusCode::OK);
    }
    // Same user, fresh address: the IP bucket is empty but the user bucket is not.
    let (s, retry, _) = post_sync(&e.app, "198.51.100.2", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(
        s,
        StatusCode::TOO_MANY_REQUESTS,
        "user bucket must bind across IPs"
    );
    assert!(retry.is_some(), "429 carries Retry-After");

    // Still blocked on the original address too: the user bucket is shared.
    let (s, _, _) = post_sync(&e.app, "198.51.100.1", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn blocked_user_gets_retry_after_capped_at_900() {
    // Window longer than the cap: the emitted Retry-After must be clamped.
    let e = env(Budget {
        class: "bookmark:sync",
        user_max: 1,
        ip_max: 10,
        window_secs: 1200,
    })
    .await;
    let (s, _, _) = post_sync(&e.app, "198.51.100.3", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::OK);
    let (s, retry, body) = post_sync(&e.app, "198.51.100.3", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(retry.as_deref(), Some("900"), "Retry-After capped at 900 s");
    assert_eq!(body["retryAfter"], 900, "body agrees with the header");
}

#[tokio::test]
async fn subject_keys_hold_the_user_id_and_never_an_ip() {
    let e = env(Budget {
        class: "bookmark:sync",
        user_max: 5,
        ip_max: 50,
        window_secs: 60,
    })
    .await;
    let (s, _, _) = post_sync(&e.app, "203.0.113.9", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::OK);

    let all = keys(&e.mem);
    let subject: Vec<&String> = all
        .iter()
        .filter(|k| k.starts_with("ratelimit:user:"))
        .collect();
    assert_eq!(subject.len(), 1, "one subject bucket: {all:?}");
    let expected = format!("ratelimit:user:{}:bookmark:sync", e.uid_a);
    assert_eq!(subject[0], &expected);
    assert!(
        !subject[0].contains("203.0.113"),
        "subject key must not carry the IP: {}",
        subject[0]
    );
    // The address appears only in the IP ceiling's own key (matched route path).
    let ip_keys: Vec<&String> = all.iter().filter(|k| k.contains("203.0.113")).collect();
    assert_eq!(ip_keys.len(), 1, "IP ceiling key: {ip_keys:?}");
    assert!(
        ip_keys[0].starts_with("ratelimit:203.0.113.9/32:"),
        "IP key shape: {ip_keys:?}"
    );
}

#[tokio::test]
async fn unauthenticated_requests_keep_only_the_ip_layer() {
    let e = env(Budget {
        class: "bookmark:sync",
        user_max: 1,
        ip_max: 2,
        window_secs: 60,
    })
    .await;
    // No session: the route guard answers 401 and the user bucket never runs.
    let (s1, _, _) = post_sync(&e.app, "198.51.100.4", None, e.uid_a).await;
    let (s2, _, _) = post_sync(&e.app, "198.51.100.4", None, e.uid_a).await;
    assert_eq!(s1, StatusCode::UNAUTHORIZED);
    assert_eq!(s2, StatusCode::UNAUTHORIZED);
    // Third anonymous request from the same IP hits the IP ceiling.
    let (s3, retry, _) = post_sync(&e.app, "198.51.100.4", None, e.uid_a).await;
    assert_eq!(s3, StatusCode::TOO_MANY_REQUESTS);
    assert!(retry.is_some());
    assert!(
        keys(&e.mem)
            .iter()
            .all(|k| !k.starts_with("ratelimit:user:")),
        "no subject bucket without a verified session"
    );
}

#[tokio::test]
async fn distinct_users_on_one_ip_have_separate_budgets() {
    let e = env(Budget {
        class: "bookmark:sync",
        user_max: 1,
        ip_max: 10,
        window_secs: 60,
    })
    .await;
    let (s, _, _) = post_sync(&e.app, "198.51.100.5", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = post_sync(&e.app, "198.51.100.5", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    // Carrier NAT: user B shares the address but not A's budget.
    let (s, _, _) = post_sync(&e.app, "198.51.100.5", Some(&e.cookie_b), e.uid_b).await;
    assert_eq!(
        s,
        StatusCode::OK,
        "user B must not inherit user A's exhaustion"
    );
}

#[tokio::test]
async fn ip_ceiling_binds_even_with_user_budget_left() {
    let e = env(Budget {
        class: "bookmark:sync",
        user_max: 5,
        ip_max: 2,
        window_secs: 60,
    })
    .await;
    let (s, _, _) = post_sync(&e.app, "198.51.100.6", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = post_sync(&e.app, "198.51.100.6", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = post_sync(&e.app, "198.51.100.6", Some(&e.cookie_a), e.uid_a).await;
    assert_eq!(
        s,
        StatusCode::TOO_MANY_REQUESTS,
        "IP layer still enforced alongside user layer"
    );
}

// Compile-time: the IP ceiling beside each authenticated budget is at least 10x
// the user budget. A regression fails the build, not a runtime assert.
const _: () = assert!(SYNC_IP_MAX >= 10 * SYNC_USER_MAX);
const _: () = assert!(ANALYTICS_IP_MAX >= 10 * ANALYTICS_USER_MAX);
