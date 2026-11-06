use std::time::Duration;

use tracing::{debug, error, instrument, warn};

use crate::error::GateError;
use crate::hooks::LimiterHooks;
use crate::store::RateLimitStore;

#[derive(Clone, Copy, Debug)]
pub struct AbuseLimiterConfig {
    pub temp_block_attempts: usize,
    pub temp_block_range: usize,
    pub temp_block_duration: usize,
    pub block_retry_limit: usize,
    pub block_range: usize,
    pub block_duration: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockScope {
    #[default]
    Temp,
    Long,
}

impl BlockScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            BlockScope::Temp => "Temp",
            BlockScope::Long => "Long",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "Temp" => Some(BlockScope::Temp),
            "Long" => Some(BlockScope::Long),
            _ => None,
        }
    }
}

/// Cap on any IP-keyed block duration, and so on its Retry-After (A8).
pub const IP_RETRY_CAP_SECS: u64 = 900;

/// Cap on the progressive credential delay (A8).
pub const CREDENTIAL_DELAY_CAP_SECS: u64 = 8;

/// How a failure-only limiter treats a bucket (A8).
///
/// `Credential` covers credential-keyed checks (password, TOTP, reauth). A
/// failure only delays the next attempt, never refuses it, and a success clears
/// the bucket. `Ip` covers IP-keyed checks: after `temp_block_attempts` failures
/// in `block_range` the caller gets a 429 whose Retry-After is capped at
/// [`IP_RETRY_CAP_SECS`]. Neither mode has a long tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureMode {
    Credential,
    Ip,
}

/// Answer from [`RateLimitStore::abuse_peek`] / [`RateLimitStore::abuse_record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    /// No delay, no block.
    Pass,
    /// Credential mode: wait this long before verifying.
    Delay { secs: u64 },
    /// IP mode: refuse until the block ends.
    Blocked { retry_after_secs: u64 },
}

/// Progressive delay after `failures` failures in the window, where the first
/// delayed attempt is the one after `threshold` failures. The delay doubles
/// from 1 s and is capped at [`CREDENTIAL_DELAY_CAP_SECS`]: 1, 2, 4, 8, 8, ...
pub fn credential_delay_secs(failures: u64, threshold: u64) -> u64 {
    if failures < threshold {
        return 0;
    }
    let exp = (failures - threshold).min(63) as u32;
    1u64.checked_shl(exp)
        .unwrap_or(u64::MAX)
        .min(CREDENTIAL_DELAY_CAP_SECS)
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

pub async fn dedup_nx(store: &dyn RateLimitStore, key: &str, ttl_secs: usize) -> bool {
    match store
        .set_nx_ex(key, Duration::from_secs(ttl_secs as u64))
        .await
    {
        Ok(claimed) => claimed,
        Err(err) => {
            warn!(error = %err, %key, "dedup check failed (fail-open)");
            true
        }
    }
}

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

    struct DeadStore;
    #[async_trait::async_trait]
    impl RateLimitStore for DeadStore {
        async fn abuse_peek(
            &self,
            _: &str,
            _: AbuseLimiterConfig,
            _: FailureMode,
        ) -> Result<GateDecision, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn abuse_record(
            &self,
            _: &str,
            _: AbuseLimiterConfig,
            _: FailureMode,
        ) -> Result<GateDecision, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn incr_expire(&self, _: &str, _: Duration) -> Result<(u64, u64), GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn abuse_check(
            &self,
            _: &str,
            _: AbuseLimiterConfig,
        ) -> Result<LimiterDecision, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn set_nx_ex(&self, _: &str, _: Duration) -> Result<bool, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn del(&self, _: &str) -> Result<(), GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn ban_status(&self, _: &str) -> Result<Option<crate::store::BanStatus>, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn clear_limit(&self, _: &str) -> Result<(), GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn set_block(
            &self,
            _: &str,
            _: crate::abuse::BlockScope,
            _: Duration,
        ) -> Result<(), GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
        async fn record_qualifying(
            &self,
            _: &str,
            _: Duration,
            _: Duration,
        ) -> Result<crate::store::QualifyingCounts, GateError> {
            Err(GateError::StoreUnavailable("dead".into()))
        }
    }

    #[test]
    fn credential_delay_is_progressive_from_threshold_and_capped() {
        let seq: Vec<u64> = (0..=7).map(|n| credential_delay_secs(n, 3)).collect();
        assert_eq!(seq, vec![0, 0, 0, 1, 2, 4, 8, 8]);
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
