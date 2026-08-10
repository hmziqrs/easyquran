use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use tower::{Layer, Service};
use tracing::{debug, warn};

use crate::ip::{ClientIpIdentitySource, IdentitySource, RequestIdentity};
use crate::store::RateLimitStore;

static X_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
static X_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
static X_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");

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
            if !(layer.applies)(&req) {
                return inner.call(req).await;
            }
            // Only a verified external IpAddr enters an IP-keyed bucket; an
            // absent identity falls back to a shared non-IP "unknown" bucket that
            // can never parse as a BanUnit (so never escalates under W3a).
            let identity = layer.identity_source.resolve(&req);
            let id_key = match &identity {
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
            if let Some(esc) = &layer.escalation {
                if let Some(id) = identity.as_ref().filter(|i| i.is_external_ip()) {
                    if let Some(retry_after) = esc.active_ban(id).await {
                        return Ok(blocked_response(
                            &layer,
                            BlockInfo {
                                retry_after_secs: retry_after,
                                max_requests: layer.max_requests,
                                window_secs: layer.window_secs,
                                count: layer.max_requests + 1,
                                ttl: retry_after,
                            },
                        ));
                    }
                }
            }

            let path = match layer.path_key {
                PathKey::Raw => req.uri().path().to_string(),
                PathKey::Fixed(c) => c.to_string(),
                PathKey::Matched => req
                    .extensions()
                    .get::<axum::extract::MatchedPath>()
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| req.uri().path().to_string()),
            };
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

            if count > layer.max_requests {
                // W3a (4): the rate window qualifies ONLY at count == max+1.
                // That equality is the one-event-per-window primitive — no
                // dedup_nx, no append on every 429. The engine checks the
                // suspicious counter internally; raw volume alone never bans.
                if count == layer.max_requests + 1 {
                    if let Some(esc) = &layer.escalation {
                        if let Some(id) = identity.as_ref().filter(|i| i.is_external_ip()) {
                            if let Some(retry_after) = esc.on_first_block(id).await {
                                // A ban was just created; carry its (longer)
                                // Retry-After instead of the fixed-window ttl.
                                return Ok(blocked_response(
                                    &layer,
                                    BlockInfo {
                                        retry_after_secs: retry_after,
                                        max_requests: layer.max_requests,
                                        window_secs: layer.window_secs,
                                        count,
                                        ttl: retry_after,
                                    },
                                ));
                            }
                        }
                    }
                }

                debug!(
                    identity = %id_key,
                    path,
                    count,
                    max_requests = layer.max_requests,
                    window_secs = layer.window_secs,
                    "Rate limit exceeded"
                );

                return Ok(blocked_response(
                    &layer,
                    BlockInfo {
                        retry_after_secs: ttl,
                        max_requests: layer.max_requests,
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

            let remaining = layer.max_requests.saturating_sub(count);
            let mut response = response;
            let headers = response.headers_mut();
            insert_header(headers, &X_RATELIMIT_LIMIT, layer.max_requests);
            insert_header(headers, &X_RATELIMIT_REMAINING, remaining);
            insert_header(headers, &X_RATELIMIT_RESET, ttl);

            Ok(response)
        })
    }
}

// Shared 429 builder: applies on_block + the standard x-ratelimit headers.
// Hoisted so both the fixed-window block and the W3a active-ban / just-created
// ban short-circuits emit an identical shaped response.
fn blocked_response(layer: &RateLimitLayer, info: BlockInfo) -> Response {
    let retry_after = info.retry_after_secs;
    let mut response = (layer.on_block)(info);
    let headers = response.headers_mut();
    insert_header(headers, &X_RATELIMIT_LIMIT, layer.max_requests);
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
    use std::net::{IpAddr, Ipv4Addr};
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
}
