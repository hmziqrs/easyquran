use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use tower::{Layer, Service};
use tracing::{debug, warn};

use crate::ip::{ClientIpSource, IpSource};
use crate::store::RateLimitStore;

static X_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
static X_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
static X_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");

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

/// Path component of the bucket key. `Fixed`/`Matched` collapse parameterized
/// routes (`/ayahs/{s}/{a}`, `/pages/N`) into one bucket — `Raw` would fan them
/// into unbounded independent buckets (memory exhaustion).
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
    ip_source: Arc<dyn IpSource>,
    path_key: PathKey,
    on_block: BlockFn,
    on_unavailable: UnavailableFn,
}

impl RateLimitLayer {
    pub fn new(store: Arc<dyn RateLimitStore>, max_requests: u64, window_secs: u64) -> Self {
        Self {
            store,
            max_requests,
            window_secs,
            ip_source: Arc::new(ClientIpSource),
            path_key: PathKey::Raw,
            on_block: Arc::new(default_on_block),
            on_unavailable: Arc::new(default_on_unavailable),
        }
    }

    pub fn with_ip(
        store: Arc<dyn RateLimitStore>,
        max_requests: u64,
        window_secs: u64,
        ip_source: Arc<dyn IpSource>,
    ) -> Self {
        Self {
            store,
            max_requests,
            window_secs,
            ip_source,
            path_key: PathKey::Raw,
            on_block: Arc::new(default_on_block),
            on_unavailable: Arc::new(default_on_unavailable),
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
}

pub struct RateLimitLayerBuilder {
    layer: RateLimitLayer,
}

impl RateLimitLayerBuilder {
    pub fn ip_source(mut self, ip: Arc<dyn IpSource>) -> Self {
        self.layer.ip_source = ip;
        self
    }
    pub fn path_key(mut self, mode: PathKey) -> Self {
        self.layer.path_key = mode;
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
            let ip = layer.ip_source.resolve(&req);
            let path = match layer.path_key {
                PathKey::Raw => req.uri().path().to_string(),
                PathKey::Fixed(c) => c.to_string(),
                PathKey::Matched => req
                    .extensions()
                    .get::<axum::extract::MatchedPath>()
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| req.uri().path().to_string()),
            };
            let key = format!("ratelimit:{}:{}", ip, path);

            let (count, ttl) = match layer
                .store
                .incr_expire(&key, Duration::from_secs(layer.window_secs))
                .await
            {
                Ok(values) => values,
                Err(err) => {
                    // Fail closed: if the store is unavailable we cannot enforce
                    // a per-IP limit, so rejecting (503) is safer than silently
                    // allowing unbounded traffic.
                    warn!(
                        error = %err,
                        key = %key,
                        "Rate-limit store error, rejecting (fail-closed)"
                    );
                    return Ok((layer.on_unavailable)());
                }
            };

            if count > layer.max_requests {
                debug!(
                    ip = %ip,
                    path,
                    count,
                    max_requests = layer.max_requests,
                    window_secs = layer.window_secs,
                    "Rate limit exceeded"
                );

                let retry_after = ttl;
                let info = BlockInfo {
                    retry_after_secs: retry_after,
                    max_requests: layer.max_requests,
                    window_secs: layer.window_secs,
                    count,
                    ttl,
                };

                let mut response = (layer.on_block)(info);
                let headers = response.headers_mut();
                insert_header(headers, &X_RATELIMIT_LIMIT, layer.max_requests);
                insert_header(headers, &X_RATELIMIT_REMAINING, 0u64);
                insert_header(headers, &X_RATELIMIT_RESET, ttl);
                insert_header(headers, &axum::http::header::RETRY_AFTER, retry_after);

                return Ok(response);
            }

            let response = inner.call(req).await?;

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

fn insert_header(headers: &mut axum::http::HeaderMap, name: &HeaderName, value: u64) {
    let val = HeaderValue::from_str(&value.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));
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
