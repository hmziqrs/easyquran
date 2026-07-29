//! Dual-threshold abuse limiter + one-shot dedup.
//!
//! The counting/blocking algorithm lives in the [`crate::RateLimitStore`]
//! implementation (atomic); these free functions wire the store result through
//! hooks/tracing and apply the documented fail semantics:
//! - [`check`]: **fail-CLOSED** — a store error yields `Err`.
//! - [`dedup_nx`]: **fail-OPEN** — a store error yields `true` so a dedup
//!   outage cannot 5xx the caller.

use std::time::Duration;

use tracing::{debug, error, instrument, warn};

use crate::error::GateError;
use crate::hooks::LimiterHooks;
use crate::store::RateLimitStore;

#[derive(Clone, Copy, Debug)]
pub struct AbuseLimiterConfig {
    pub temp_block_attempts: usize,
    pub temp_block_range: usize,    // seconds
    pub temp_block_duration: usize, // seconds
    pub block_retry_limit: usize,   // long threshold
    pub block_range: usize,         // seconds
    pub block_duration: usize,      // seconds
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockScope {
    Temp,
    Long,
}

#[derive(Debug, Clone)]
pub enum LimiterDecision {
    Allowed {
        short_count: u64,
        long_count: u64,
    },
    Blocked {
        scope: BlockScope,
        retry_after_secs: u64,
        short_count: u64,
        long_count: u64,
    },
}

/// Check the abuse limiter for `key_prefix`. **Fail-CLOSED**: a store error
/// yields `Err(GateError)`. A `Blocked` decision is an `Ok` variant — the caller
/// decides how to respond.
#[instrument(skip(store, hooks), fields(
    scope = %key_prefix,
    decision,
    short_count,
    long_count,
    retry_after
))]
pub async fn check(
    store: &dyn RateLimitStore,
    hooks: &dyn LimiterHooks,
    key_prefix: &str,
    config: AbuseLimiterConfig,
) -> Result<LimiterDecision, GateError> {
    hooks.on_check();

    debug!(
        temp_threshold = config.temp_block_attempts,
        temp_window = config.temp_block_range,
        long_threshold = config.block_retry_limit,
        long_window = config.block_range,
        "Checking abuse limiter"
    );

    let decision = match store.abuse_check(key_prefix, config).await {
        Ok(d) => d,
        Err(err) => {
            error!(
                error = %err,
                key_prefix = %key_prefix,
                "Store error during limiter check"
            );
            return Err(err);
        }
    };

    match &decision {
        LimiterDecision::Allowed {
            short_count,
            long_count,
        } => {
            debug!(short_count, long_count, "Request allowed");
            tracing::Span::current().record("decision", "allowed");
            tracing::Span::current().record("short_count", short_count);
            tracing::Span::current().record("long_count", long_count);
            hooks.on_allowed(*short_count, *long_count);
        }
        LimiterDecision::Blocked {
            scope,
            retry_after_secs,
            short_count,
            long_count,
        } => {
            warn!(
                scope = ?scope,
                retry_after = retry_after_secs,
                short_count,
                long_count,
                "Request blocked by abuse limiter"
            );
            tracing::Span::current().record("decision", "blocked");
            tracing::Span::current().record("short_count", short_count);
            tracing::Span::current().record("long_count", long_count);
            tracing::Span::current().record("retry_after", retry_after_secs);
            hooks.on_blocked(*scope, *retry_after_secs, *short_count, *long_count);
        }
    }

    Ok(decision)
}

/// One-shot dedup gate. Returns `true` when the key was newly created (the
/// caller should proceed) and `false` when it already existed within the window.
///
/// **Fail-OPEN**: a store error yields `true`, so a dedup outage cannot 5xx the
/// caller. The fail-open contract is baked into the `bool` return type.
pub async fn dedup_nx(store: &dyn RateLimitStore, key: &str, ttl_secs: usize) -> bool {
    match store.set_nx_ex(key, Duration::from_secs(ttl_secs as u64)).await {
        Ok(claimed) => claimed,
        Err(err) => {
            warn!(error = %err, %key, "dedup check failed (fail-open)");
            true
        }
    }
}

/// Release a dedup claim early (best-effort) so a gated operation that failed
/// after claiming can be retried within the window. Silent on a store error.
pub async fn release_dedup(store: &dyn RateLimitStore, key: &str) {
    if let Err(err) = store.del(key).await {
        warn!(error = %err, %key, "dedup release failed (fail-open)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::NoHooks;
    use crate::store::InMemoryStore;

    const CFG: AbuseLimiterConfig = AbuseLimiterConfig {
        temp_block_attempts: 3,
        temp_block_range: 60,
        temp_block_duration: 300,
        block_retry_limit: 5,
        block_range: 3600,
        block_duration: 86400,
    };

    /// A store that always errors, to prove the fail semantics.
    struct DeadStore;
    #[async_trait::async_trait]
    impl RateLimitStore for DeadStore {
        async fn incr_expire(&self, _: &str, _: Duration) -> Result<(u64, u64), GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn abuse_check(&self, _: &str, _: AbuseLimiterConfig) -> Result<LimiterDecision, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn set_nx_ex(&self, _: &str, _: Duration) -> Result<bool, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn del(&self, _: &str) -> Result<(), GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
    }

    #[tokio::test]
    async fn dedup_nx_is_fail_open_on_store_error() {
        assert!(dedup_nx(&DeadStore, "k", 300).await);
    }

    #[tokio::test]
    async fn release_dedup_is_silent_on_store_error() {
        release_dedup(&DeadStore, "k").await;
    }

    #[tokio::test]
    async fn check_is_fail_closed_on_store_error() {
        let r = check(&DeadStore, &NoHooks, "k", CFG).await;
        assert!(matches!(r, Err(GateError::StoreUnavailable(_))));
    }

    #[tokio::test]
    async fn check_allows_then_blocks_against_real_store() {
        let store = InMemoryStore::default();
        for _ in 0..2 {
            assert!(matches!(
                check(&store, &NoHooks, "ip", CFG).await.unwrap(),
                LimiterDecision::Allowed { .. }
            ));
        }
        assert!(matches!(
            check(&store, &NoHooks, "ip", CFG).await.unwrap(),
            LimiterDecision::Blocked { .. }
        ));
    }
}
