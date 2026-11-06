use std::collections::BTreeMap;
use std::future::Future;
use std::net::{IpAddr, Ipv6Addr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use tower::{Layer, Service};
use tracing::warn;

use crate::ip::{ClientIpIdentitySource, IdentitySource, RequestIdentity};
use crate::store::RateLimitStore;

static X_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
static X_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
static X_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");

// Process-local count of route-level 429s, keyed by the request path label.
// Bounded: once `MAX_429_PATH_LABELS` distinct paths are held, further new
// paths fold into `OTHER_429_PATH`, so a Raw-keyed limiter under a path-spray
// attack cannot grow this map without bound. Read by the abuse summary only.
const MAX_429_PATH_LABELS: usize = 64;
const OTHER_429_PATH: &str = "(other)";
static BLOCKED_BY_PATH: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

fn record_429(path: &str) {
    if let Ok(mut map) = BLOCKED_BY_PATH.lock() {
        count_bounded(&mut map, path, MAX_429_PATH_LABELS);
    }
}

fn count_bounded(map: &mut BTreeMap<String, u64>, path: &str, max_labels: usize) {
    if let Some(n) = map.get_mut(path) {
        *n += 1;
        return;
    }
    let label = if map.len() < max_labels {
        path.to_string()
    } else {
        OTHER_429_PATH.to_string()
    };
    *map.entry(label).or_insert(0) += 1;
}

/// `(path, count)` pairs of route-level 429s since process start, sorted by
/// path. Bounded to `MAX_429_PATH_LABELS` labels plus one overflow bucket.
pub fn blocked_by_path() -> Vec<(String, u64)> {
    match BLOCKED_BY_PATH.lock() {
        Ok(map) => map.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        Err(_) => Vec::new(),
    }
}

/// Subject scores below this halve the budget. Documented seam (A7): nothing sets
/// a score today, so every limiter runs at the default 1.0.
pub const LOW_SUBJECT_SCORE: f64 = 0.5;

/// Effective budget for one limiter given its subject score. Score 1.0 (the
/// default) keeps `max` exactly. A low score halves it, never below 1. It never
/// blocks a request by itself: the fixed window still does the enforcing.
pub fn scaled_max(max: u64, subject_score: f64) -> u64 {
    if subject_score >= LOW_SUBJECT_SCORE {
        return max;
    }
    (max / 2).max(1)
}

/// Escalation hook invoked by the outer Quran content limiter only. The
/// implementation (api crate `EscalationEngine`) resolves the canonical BanUnit,
/// consults the shared store, and owns the L1 suspicious/qualifying state. The
/// layer calls these three methods at the exact points where the resolved
/// identity + fixed-window count are known; it never parses response bodies.
///
/// All three methods are no-ops for non-external identities (the layer guards
/// `is_external_ip()` before calling, and the implementation re-checks). When
/// escalation is disabled the layer holds `None` and never calls into here, so
/// default-off preserves the exact pre-W3a request path.
#[async_trait::async_trait]
pub trait Escalation: Send + Sync {
    /// Called BEFORE the fixed-window increment. Returns `Some(retry_after_secs)`
    /// when an active Temp/Long ban should short-circuit this request to a 429
    /// without touching the fixed window (so the ban holds even after the fixed
    /// window rolls over). Read-only: never appends a qualifying event.
    async fn active_ban(&self, identity: &RequestIdentity) -> Option<u64>;

    /// Called for an ALLOWED request (`count <= max`). The implementation reads
    /// the private typed classification extension off `response` and records a
    /// suspicious 4xx for the closed set (unknown source / invalid range /
    /// unknown route), resetting the per-window counter when `count == 1`.
    ///
    /// Sync by design: it performs no await (only an extension read + a quick
    /// mutex lock), and a sync signature avoids requiring `Response: Sync`
    /// (`axum::body::Body` is `Send` but not `Sync`, so `&Response` cannot cross
    /// a `Send` boxed-future boundary).
    fn observe_allowed(&self, identity: &RequestIdentity, count: u64, response: &Response);

    /// Called exactly when `count == max_requests + 1` — the one-event-per-window
    /// primitive. If the suspicious counter meets its threshold this is a
    /// qualifying window; the implementation prunes/records history (Long before
    /// Temp) and may create or upgrade a ban. Returns `Some(retry_after_secs)`
    /// when a ban was just created so the 429 can carry the right Retry-After.
    async fn on_first_block(&self, identity: &RequestIdentity) -> Option<u64>;
}

#[derive(Debug, Clone, Copy)]
pub struct BlockInfo {
    pub retry_after_secs: u64,
    pub max_requests: u64,
    pub window_secs: u64,
    pub count: u64,
    pub ttl: u64,
}

type BlockFn = Arc<dyn Fn(BlockInfo) -> Response + Send + Sync>;
type UnavailableFn = Arc<dyn Fn() -> Response + Send + Sync>;
// When false, the layer passes the request through without limiting. Used to
// stack isolated limiters (external content / internal service / health) on one
// router: each self-skips except for the bucket it owns.
type AppliesFn = Arc<dyn Fn(&axum::extract::Request) -> bool + Send + Sync>;

/// Path component of the bucket key: `Fixed`/`Matched` collapse parameterized routes into one bucket; `Raw` fans them into unbounded buckets (memory exhaustion).
#[derive(Clone)]
pub enum PathKey {
    Raw,
    Fixed(&'static str),
    Matched,
}

#[derive(Clone)]
pub struct RateLimitLayer {
    store: Arc<dyn RateLimitStore>,
    max_requests: u64,
    window_secs: u64,
    identity_source: Arc<dyn IdentitySource>,
    path_key: PathKey,
    applies: AppliesFn,
    on_block: BlockFn,
    on_unavailable: UnavailableFn,
    // W3a escalation. `None` on every limiter except the outer quran-v1 content
    // ceiling, so all other routes keep the exact pre-W3a path.
    escalation: Option<Arc<dyn Escalation>>,
    // Upper bound on any Retry-After this limiter emits. `None` keeps the
    // fixed-window ttl unchanged.
    retry_after_cap: Option<u64>,
    // Subject score seam (A7). Default 1.0; see `scaled_max`.
    subject_score: f64,
}

impl RateLimitLayer {
    pub fn new(store: Arc<dyn RateLimitStore>, max_requests: u64, window_secs: u64) -> Self {
        Self {
            store,
            max_requests,
            window_secs,
            identity_source: Arc::new(ClientIpIdentitySource),
            path_key: PathKey::Raw,
            applies: Arc::new(|_| true),
            on_block: Arc::new(default_on_block),
            on_unavailable: Arc::new(default_on_unavailable),
            escalation: None,
            retry_after_cap: None,
            subject_score: 1.0,
        }
    }

    pub fn with_identity(
        store: Arc<dyn RateLimitStore>,
        max_requests: u64,
        window_secs: u64,
        identity_source: Arc<dyn IdentitySource>,
    ) -> Self {
        Self {
            store,
            max_requests,
            window_secs,
            identity_source,
            path_key: PathKey::Raw,
            applies: Arc::new(|_| true),
            on_block: Arc::new(default_on_block),
            on_unavailable: Arc::new(default_on_unavailable),
            escalation: None,
            retry_after_cap: None,
            subject_score: 1.0,
        }
    }

    pub fn builder(
        store: Arc<dyn RateLimitStore>,
        max_requests: u64,
        window_secs: u64,
    ) -> RateLimitLayerBuilder {
        RateLimitLayerBuilder {
            layer: Self::new(store, max_requests, window_secs),
        }
    }

    /// Attach the W3a escalation hook after construction. Used by the outer
    /// quran-v1 content limiter only; every other limiter leaves this unset.
    pub fn with_escalation(mut self, escalation: Option<Arc<dyn Escalation>>) -> Self {
        self.escalation = escalation;
        self
    }
}

pub struct RateLimitLayerBuilder {
    layer: RateLimitLayer,
}

impl RateLimitLayerBuilder {
    pub fn identity_source(mut self, src: Arc<dyn IdentitySource>) -> Self {
        self.layer.identity_source = src;
        self
    }
    pub fn path_key(mut self, mode: PathKey) -> Self {
        self.layer.path_key = mode;
        self
    }
    pub fn applies<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&axum::extract::Request) -> bool + Send + Sync + 'static,
    {
        self.layer.applies = Arc::new(predicate);
        self
    }
    pub fn on_block<F>(mut self, f: F) -> Self
    where
        F: Fn(BlockInfo) -> Response + Send + Sync + 'static,
    {
        self.layer.on_block = Arc::new(f);
        self
    }
    pub fn on_unavailable<F>(mut self, f: F) -> Self
    where
        F: Fn() -> Response + Send + Sync + 'static,
    {
        self.layer.on_unavailable = Arc::new(f);
        self
    }
    /// Subject score seam (A7). Clamped to [0, 1]; non-finite input keeps the
    /// default 1.0. A low score logs an operator event once, at build time.
    pub fn subject_score(mut self, score: f64) -> Self {
        let score = if score.is_finite() {
            score.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if score < LOW_SUBJECT_SCORE {
            warn!(
                subject_score = score,
                "rate-limit subject score is low: budget halved (operator event)"
            );
        }
        self.layer.subject_score = score;
        self
    }
    /// Cap every Retry-After this limiter emits (header and body) at `secs`.
    pub fn retry_after_cap(mut self, secs: u64) -> Self {
        self.layer.retry_after_cap = Some(secs);
        self
    }
    pub fn build(self) -> RateLimitLayer {
        self.layer
    }
}

impl<Inner> Layer<Inner> for RateLimitLayer {
    type Service = RateLimitMiddleware<Inner>;

    fn layer(&self, inner: Inner) -> Self::Service {
        RateLimitMiddleware {
            inner,
            layer: self.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitMiddleware<Inner> {
    inner: Inner,
    layer: RateLimitLayer,
}

impl<Inner> Service<axum::extract::Request> for RateLimitMiddleware<Inner>
where
    Inner: Service<axum::extract::Request, Response = Response> + Clone + Send + 'static,
    Inner::Future: Send + 'static,
{
    type Response = Inner::Response;
    type Error = Inner::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::extract::Request) -> Self::Future {
        let layer = self.layer.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let max = scaled_max(layer.max_requests, layer.subject_score);
            if !(layer.applies)(&req) {
                return inner.call(req).await;
            }
            // Only a verified external IpAddr enters an IP-keyed bucket; an
            // absent identity falls back to a shared non-IP "unknown" bucket that
            // can never parse as a BanUnit (so never escalates under W3a).
            let identity = layer.identity_source.resolve(&req);
            // W3a invariant: one canonical BanUnit keys fixed limiting + history +
            // bans + persistence + export. An external IP's fixed-window bucket
            // must live under the CANONICAL unit (IPv4 /32, IPv6 /64) — the same
            // string the escalation engine bans under and the W3c admin un-ban
            // clears via `ratelimit:{unit.canonical()}:quran-v1`. Keying on the
            // raw IP instead strands a lifted ban's fixed counter under a key the
            // un-ban never touches. `canonical_unit_key` mirrors BanUnit::from_ip
            // in the api crate byte-for-byte; the two MUST stay in lockstep.
            // Internal service identities keep their non-IP bucket label (never
            // escalates/bans); an absent identity falls back to "unknown".
            let id_key = match &identity {
                Some(RequestIdentity::External(ip)) => canonical_unit_key(*ip),
                Some(id) => id.bucket_key(),
                None => "unknown".to_string(),
            };

            // W3a (1): resolve the canonical BanUnit BEFORE the fixed-window
            // increment and short-circuit on an active Temp/Long ban. Only a
            // verified external IP can enter this state machine; the implementation
            // re-checks, so an internal/absent identity never escalates. The fixed
            // window is NOT touched here, so the ban holds after fixed-window
            // rollover. Escalation store errors are best-effort (log + proceed to
            // fixed limiting); they never 5xx the caller.
            let path = match layer.path_key {
                PathKey::Raw => req.uri().path().to_string(),
                PathKey::Fixed(c) => c.to_string(),
                PathKey::Matched => req
                    .extensions()
                    .get::<axum::extract::MatchedPath>()
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| req.uri().path().to_string()),
            };

            if let Some(esc) = &layer.escalation {
                if let Some(id) = identity.as_ref().filter(|i| i.is_external_ip()) {
                    if let Some(retry_after) = esc.active_ban(id).await {
                        record_429(&path);
                        return Ok(blocked_response(
                            &layer,
                            BlockInfo {
                                retry_after_secs: retry_after,
                                max_requests: max,
                                window_secs: layer.window_secs,
                                count: max + 1,
                                ttl: retry_after,
                            },
                        ));
                    }
                }
            }

            let key = format!("ratelimit:{}:{}", id_key, path);

            let (count, ttl) = match layer
                .store
                .incr_expire(&key, Duration::from_secs(layer.window_secs))
                .await
            {
                Ok(values) => values,
                Err(err) => {
                    // Fail closed: store unavailable means we can't enforce the per-IP limit, so 503 beats silently allowing unbounded traffic.
                    warn!(
                        error = %err,
                        key = %key,
                        "Rate-limit store error, rejecting (fail-closed)"
                    );
                    return Ok((layer.on_unavailable)());
                }
            };

            if count > max {
                // W3a (4): the rate window qualifies ONLY at count == max+1.
                // That equality is the one-event-per-window primitive — no
                // dedup_nx, no append on every 429. The engine checks the
                // suspicious counter internally; raw volume alone never bans.
                if count == max + 1 {
                    if let Some(esc) = &layer.escalation {
                        if let Some(id) = identity.as_ref().filter(|i| i.is_external_ip()) {
                            if let Some(retry_after) = esc.on_first_block(id).await {
                                // A ban was just created; carry its (longer)
                                // Retry-After instead of the fixed-window ttl.
                                record_429(&path);
                                return Ok(blocked_response(
                                    &layer,
                                    BlockInfo {
                                        retry_after_secs: retry_after,
                                        max_requests: max,
                                        window_secs: layer.window_secs,
                                        count,
                                        ttl: retry_after,
                                    },
                                ));
                            }
                        }
                    }
                }

                // warn!, not debug!: a route-level 429 is the measurable abuse
                // signal (A5). The identity is the canonical unit key, never a
                // device or account id.
                record_429(&path);
                warn!(
                    identity = %id_key,
                    path,
                    count,
                    limit = max,
                    window_secs = layer.window_secs,
                    "Rate limit exceeded"
                );

                return Ok(blocked_response(
                    &layer,
                    BlockInfo {
                        retry_after_secs: ttl,
                        max_requests: max,
                        window_secs: layer.window_secs,
                        count,
                        ttl,
                    },
                ));
            }

            let response = inner.call(req).await?;

            // W3a (3): the engine reads the private typed classification
            // extension off the allowed response (never JSON/message text) and
            // records suspicious Quran 4xx for the closed set. count == 1 inside
            // observe_allowed signals a fresh fixed window (counter reset).
            if let Some(esc) = &layer.escalation {
                if let Some(id) = identity.as_ref().filter(|i| i.is_external_ip()) {
                    esc.observe_allowed(id, count, &response);
                }
            }

            let remaining = max.saturating_sub(count);
            let mut response = response;
            let headers = response.headers_mut();
            insert_header(headers, &X_RATELIMIT_LIMIT, max);
            insert_header(headers, &X_RATELIMIT_REMAINING, remaining);
            insert_header(headers, &X_RATELIMIT_RESET, ttl);

            Ok(response)
        })
    }
}

// Canonical W3a ban-unit key for an external IP: IPv4 collapses to a /32, IPv6
// to its /64 network (host bits zeroed). This MIRRORS BanUnit::from_ip(ip)
// .canonical() in the api crate (modules/admin_bans_v1/dto.rs) byte-for-byte —
// the same string keys fixed limiting, escalation history, active bans, L2
// persistence, and export, so the W3c admin un-ban (which clears
// `ratelimit:{unit}:quran-v1`) reaps the exact fixed counter the limiter
// increments. The gate crate cannot import BanUnit (the api crate depends on
// it), so the two are kept in lockstep by this mirror; the round-trip tests in
// dto.rs pin the canonical format, and `canonical_unit_key_mirrors_ban_unit`
// below pins this side of the contract.
fn canonical_unit_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => format!("{v4}/32"),
        IpAddr::V6(v6) => {
            // Zero the host bits (bytes 8..16) to form the /64 network, matching
            // ipnet::Ipv6Net::trunc(). std's Ipv6Addr Display emits the same
            // canonical RFC 5952 form ipnet does, so the strings are identical.
            let mut net = v6.octets();
            net[8..].fill(0);
            format!("{}/64", Ipv6Addr::from(net))
        }
    }
}

// Shared 429 builder: applies on_block + the standard x-ratelimit headers.
// Hoisted so both the fixed-window block and the W3a active-ban / just-created
// ban short-circuits emit an identical shaped response.
fn blocked_response(layer: &RateLimitLayer, mut info: BlockInfo) -> Response {
    if let Some(cap) = layer.retry_after_cap {
        info.retry_after_secs = info.retry_after_secs.min(cap);
    }
    let retry_after = info.retry_after_secs;
    let mut response = (layer.on_block)(info);
    let headers = response.headers_mut();
    insert_header(headers, &X_RATELIMIT_LIMIT, info.max_requests);
    insert_header(headers, &X_RATELIMIT_REMAINING, 0u64);
    insert_header(headers, &X_RATELIMIT_RESET, info.ttl);
    insert_header(headers, &axum::http::header::RETRY_AFTER, retry_after);
    response
}

fn insert_header(headers: &mut axum::http::HeaderMap, name: &HeaderName, value: u64) {
    let val =
        HeaderValue::from_str(&value.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0"));
    headers.insert(name, val);
}

fn default_on_block(info: BlockInfo) -> Response {
    (
        axum::http::StatusCode::TOO_MANY_REQUESTS,
        axum::Json(serde_json::json!({
            "error": "rate_limited",
            "message": format!("Too many requests. Try again in {} seconds.", info.retry_after_secs),
            "retryAfter": info.retry_after_secs,
        })),
    )
        .into_response()
}

fn default_on_unavailable() -> Response {
    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        axum::Json(serde_json::json!({
            "error": "rate limit service unavailable",
            "message": "Could not reach the rate-limit store. Try again shortly."
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip::IdentitySource;
    use crate::store::InMemoryStore;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use tower::Service;
    use tower::ServiceExt;

    // Always-200 inner service so the limiter's allow/block path is the only
    // variable under test.
    #[derive(Clone)]
    struct OkService;
    impl Service<axum::extract::Request> for OkService {
        type Response = Response;
        type Error = std::convert::Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Response, Self::Error>> + Send>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: axum::extract::Request) -> Self::Future {
            Box::pin(async { Ok(Response::new(axum::body::Body::empty())) })
        }
    }

    struct StaticExternal;
    impl IdentitySource for StaticExternal {
        fn resolve(&self, _: &axum::extract::Request) -> Option<RequestIdentity> {
            Some(RequestIdentity::External(IpAddr::V4(Ipv4Addr::new(
                203, 0, 113, 5,
            ))))
        }
    }

    struct CountingEscalation {
        first_blocks: AtomicU32,
        observes: AtomicU32,
        active_bans: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Escalation for CountingEscalation {
        async fn active_ban(&self, _: &RequestIdentity) -> Option<u64> {
            self.active_bans.fetch_add(1, Ordering::Relaxed);
            None
        }
        fn observe_allowed(&self, _: &RequestIdentity, _: u64, _: &Response) {
            self.observes.fetch_add(1, Ordering::Relaxed);
        }
        async fn on_first_block(&self, _: &RequestIdentity) -> Option<u64> {
            self.first_blocks.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    fn req() -> axum::extract::Request {
        axum::http::Request::builder()
            .method("GET")
            .uri("/quran/x")
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn flood_calls_on_first_block_once_per_window() {
        // The count == max+1 equality is the one-event-per-window primitive: a
        // flood of 429s (count max+2, max+3, ...) in the same window must NOT
        // re-invoke on_first_block. observe_allowed runs once per ALLOWED request.
        let store = Arc::new(InMemoryStore::default()) as Arc<dyn RateLimitStore>;
        let esc = Arc::new(CountingEscalation {
            first_blocks: AtomicU32::new(0),
            observes: AtomicU32::new(0),
            active_bans: AtomicU32::new(0),
        });
        let layer = RateLimitLayer::builder(store, 5, 60)
            .identity_source(Arc::new(StaticExternal))
            .path_key(PathKey::Fixed("quran-v1"))
            .build()
            .with_escalation({
                let esc_dyn: Arc<dyn Escalation> = esc.clone();
                Some(esc_dyn)
            });
        let svc = layer.layer(OkService);

        // 5 allowed (count 1..5) + 5 blocked flood (count 6..10) in one window.
        let mut blocked = 0u32;
        let mut allowed = 0u32;
        for _ in 0..10 {
            let resp = svc.clone().oneshot(req()).await.unwrap();
            if resp.status() == axum::http::StatusCode::OK {
                allowed += 1;
            } else {
                blocked += 1;
            }
        }
        assert_eq!(allowed, 5);
        assert_eq!(blocked, 5);

        assert_eq!(
            esc.first_blocks.load(Ordering::Relaxed),
            1,
            "on_first_block must fire exactly once per window (count == max+1)"
        );
        assert_eq!(
            esc.observes.load(Ordering::Relaxed),
            5,
            "observe_allowed runs once per allowed request"
        );
        // active_ban is consulted before EVERY request (5 allowed + 5 blocked).
        assert_eq!(esc.active_bans.load(Ordering::Relaxed), 10);
    }

    #[tokio::test]
    async fn escalation_none_preserves_exact_legacy_path() {
        // When no escalation is attached, the layer never resolves a typed
        // identity for escalation and behaves exactly as before W3a.
        let store = Arc::new(InMemoryStore::default()) as Arc<dyn RateLimitStore>;
        let layer = RateLimitLayer::builder(store.clone(), 2, 60)
            .identity_source(Arc::new(StaticExternal))
            .path_key(PathKey::Fixed("quran-v1"))
            .build();
        let svc = layer.layer(OkService);

        let r1 = svc.clone().oneshot(req()).await.unwrap();
        assert_eq!(r1.status(), axum::http::StatusCode::OK);
        let r3 = svc.clone().oneshot(req()).await.unwrap();
        let r3b = svc.clone().oneshot(req()).await.unwrap();
        assert_eq!(r3.status(), axum::http::StatusCode::OK);
        // 3rd request is the first block (max=2 → count 3 > 2).
        assert_eq!(r3b.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn canonical_unit_key_mirrors_ban_unit() {
        // W3a invariant: the fixed-window bucket key for an external IP MUST be
        // the canonical ban unit (IPv4 /32, IPv6 /64 truncated), the same string
        // the escalation engine bans under and the W3c admin un-ban clears via
        // `ratelimit:{unit}:quran-v1`. This pins this side of the mirror; the
        // round-trip tests in admin_bans_v1/dto.rs pin BanUnit's side.
        assert_eq!(
            canonical_unit_key(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 5))),
            "203.0.113.5/32"
        );
        // Host bits truncated to the /64 network.
        assert_eq!(
            canonical_unit_key(IpAddr::V6(Ipv6Addr::new(
                0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x1
            ))),
            "2001:db8::/64"
        );
        // Every address in the same /64 collapses to one unit.
        let a = canonical_unit_key(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x1,
        )));
        let b = canonical_unit_key(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0x0db8, 0, 0, 0, 0, 0, 0xabcd,
        )));
        assert_eq!(a, b, "a /64 range must key to one fixed bucket");
    }

    #[test]
    fn subject_score_default_keeps_the_budget() {
        // A7 seam: the default 1.0 must be a no-op for every budget.
        for max in [1u64, 2, 60, 600] {
            assert_eq!(scaled_max(max, 1.0), max);
        }
        assert_eq!(
            scaled_max(60, 0.5),
            60,
            "0.5 is not low: the floor is strict"
        );
    }

    #[test]
    fn subject_score_low_halves_budget_never_below_one() {
        assert_eq!(scaled_max(60, 0.25), 30);
        assert_eq!(scaled_max(3, 0.0), 1);
        assert_eq!(scaled_max(1, 0.0), 1);
    }

    #[test]
    fn subject_score_builder_clamps_and_defaults_non_finite() {
        let store = Arc::new(InMemoryStore::default()) as Arc<dyn RateLimitStore>;
        let base = || RateLimitLayer::builder(store.clone(), 5, 60);
        assert_eq!(base().build().subject_score, 1.0);
        assert_eq!(base().subject_score(f64::NAN).build().subject_score, 1.0);
        assert_eq!(base().subject_score(2.5).build().subject_score, 1.0);
        assert_eq!(base().subject_score(-3.0).build().subject_score, 0.0);
    }

    #[tokio::test]
    async fn low_subject_score_lowers_the_budget_but_default_does_not() {
        // Default score: max 4 means four allowed, the fifth blocked.
        let store = Arc::new(InMemoryStore::default()) as Arc<dyn RateLimitStore>;
        let default = RateLimitLayer::builder(store.clone(), 4, 60)
            .path_key(PathKey::Fixed("a7-default"))
            .build()
            .layer(OkService);
        let mut ok = 0;
        for _ in 0..5 {
            if default.clone().oneshot(req()).await.unwrap().status() == axum::http::StatusCode::OK
            {
                ok += 1;
            }
        }
        assert_eq!(ok, 4);

        // Low score 0.25: budget 4 -> 2, so only two pass.
        let low = RateLimitLayer::builder(store, 4, 60)
            .path_key(PathKey::Fixed("a7-low"))
            .subject_score(0.25)
            .build()
            .layer(OkService);
        let mut ok = 0;
        let mut limit_header = None;
        for _ in 0..5 {
            let resp = low.clone().oneshot(req()).await.unwrap();
            if resp.status() == axum::http::StatusCode::OK {
                ok += 1;
            } else {
                limit_header = resp.headers().get("x-ratelimit-limit").cloned();
            }
        }
        assert_eq!(ok, 2);
        assert_eq!(
            limit_header.unwrap(),
            "2",
            "emitted limit follows the budget"
        );
    }

    #[test]
    fn path_label_map_is_bounded_and_overflow_is_counted() {
        let mut map = BTreeMap::new();
        for i in 0..200 {
            count_bounded(&mut map, &format!("/spray/{i}"), 8);
        }
        // 8 distinct labels plus one overflow bucket, nothing dropped.
        assert_eq!(map.len(), 9);
        assert_eq!(map.get(OTHER_429_PATH), Some(&192));
        assert_eq!(map.values().sum::<u64>(), 200);
    }

    #[derive(Clone)]
    struct SharedBuf(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for SharedBuf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn route_level_429_logs_at_warn_and_is_counted() {
        let buf = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let sink = SharedBuf(buf.clone());
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || sink.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let store = Arc::new(InMemoryStore::default()) as Arc<dyn RateLimitStore>;
        let layer = RateLimitLayer::builder(store, 1, 60)
            .identity_source(Arc::new(StaticExternal))
            .path_key(PathKey::Fixed("a5-warn-route"))
            .build();
        let mut svc = layer.layer(OkService);
        let first = svc.call(req()).await.unwrap();
        assert_eq!(first.status(), axum::http::StatusCode::OK);
        let second = svc.call(req()).await.unwrap();
        assert_eq!(second.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);

        let logged = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        let line = logged
            .lines()
            .find(|l| l.contains("Rate limit exceeded"))
            .unwrap_or_else(|| panic!("no 429 log line in: {logged}"));
        assert!(line.contains("WARN"), "429 must log at warn: {line}");
        assert!(line.contains("a5-warn-route"), "path field missing: {line}");
        assert!(line.contains("limit=1"), "limit field missing: {line}");
        assert!(line.contains("count=2"), "count field missing: {line}");

        assert!(
            blocked_by_path()
                .iter()
                .any(|(p, n)| p == "a5-warn-route" && *n >= 1),
            "route 429 not counted"
        );
    }
}
