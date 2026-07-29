//! Backend-agnostic rate-limit store.
//!
//! The gate primitives talk to a [`RateLimitStore`] trait instead of any
//! specific backend. The default [`InMemoryStore`] is a std-only, in-process
//! implementation: enforcement is real (every request mutates the same map
//! under one lock) but, being in-memory, state resets on restart. Its
//! [`InMemoryStore::snapshot`] / [`InMemoryStore::restore`] methods let a host
//! app add a durable backing store (e.g. SQLite) as an L2 without the gate
//! crate depending on it.
//!
//! Time is tracked as absolute epoch instants (`SystemTime`) so that a
//! snapshotted-and-restored block/counter expires correctly across a restart
//! (a relative "remaining seconds" would otherwise reset the clock on boot).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::abuse::{AbuseLimiterConfig, BlockScope, LimiterDecision};
use crate::error::GateError;

/// Seconds since the Unix epoch; 0 means "none/expired".
type Epoch = i64;

fn now_epoch() -> Epoch {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn epoch_to_systime(secs: Epoch) -> Option<SystemTime> {
    if secs <= 0 {
        None
    } else {
        UNIX_EPOCH.checked_add(Duration::from_secs(secs as u64))
    }
}

/// A backend-agnostic rate-limit / abuse / dedup store.
///
/// Implementations own atomicity. For [`RateLimitStore::abuse_check`] the
/// counting and any block placement MUST happen atomically (the in-memory
/// impl achieves this by taking one lock for the whole op).
#[async_trait]
pub trait RateLimitStore: Send + Sync {
    /// Fixed-window counter increment (used by the rate-limit layer). Increments
    /// `key`, (re)seeds an expiry of `window` on the first increment in a
    /// window, and returns `(count_after_incr, ttl_remaining_secs)`.
    async fn incr_expire(&self, key: &str, window: Duration) -> Result<(u64, u64), GateError>;

    /// Atomic dual-threshold abuse check. Records one attempt against `key` and
    /// returns the decision (allowed, or blocked with a retry-after). The
    /// implementation performs the sliding-window count + block placement.
    async fn abuse_check(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
    ) -> Result<LimiterDecision, GateError>;

    /// One-shot idempotency claim. Returns `true` if the key was newly created,
    /// `false` if it already existed within its TTL.
    async fn set_nx_ex(&self, key: &str, ttl: Duration) -> Result<bool, GateError>;

    /// Delete a key (best-effort release of a dedup claim). Always succeeds.
    async fn del(&self, key: &str) -> Result<(), GateError>;
}

/// Point-in-time snapshot of an in-memory limiter bucket, used to persist the
/// L1 cache to / restore it from a durable backing store. Expiries are absolute
/// epoch seconds so durability survives restart correctly.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BucketSnapshot {
    pub key: String,
    pub fixed_count: u64,
    /// Absolute epoch-second expiry of the fixed-window counter (0 = none).
    pub fixed_expires_at: Epoch,
    /// Absolute epoch-second expiry of an active abuse block (0 = none).
    pub block_until_at: Epoch,
}

#[derive(Default)]
struct Bucket {
    /// Fixed-window counter + expiry (rate-limit layer).
    fixed_count: u64,
    fixed_expires: Option<SystemTime>,
    /// Sliding-window attempt timestamps (abuse limiter), as epoch seconds.
    attempts: VecDeque<Epoch>,
    /// Active abuse block.
    block_until: Option<SystemTime>,
    block_scope: Option<BlockScope>,
}

/// Std-only, in-process rate-limit store.
#[derive(Default)]
pub struct InMemoryStore {
    limits: Mutex<HashMap<String, Bucket>>,
    claims: Mutex<HashMap<String, SystemTime>>,
}

impl InMemoryStore {
    /// Capture all non-expired buckets for L2 persistence.
    pub fn snapshot(&self) -> Vec<BucketSnapshot> {
        let now = now_epoch();
        let map = self.limits.lock().expect("rate-limit map poisoned");
        map.iter()
            .filter_map(|(key, b)| {
                let fixed_expires_at = match b.fixed_expires {
                    Some(t) => t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
                    None => 0,
                };
                let block_until_at = match b.block_until {
                    Some(t) => t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
                    None => 0,
                };
                if fixed_expires_at <= now && block_until_at <= now {
                    None
                } else {
                    Some(BucketSnapshot {
                        key: key.clone(),
                        fixed_count: b.fixed_count,
                        fixed_expires_at,
                        block_until_at,
                    })
                }
            })
            .collect()
    }

    /// Re-seed buckets from an L2 snapshot (called once at boot before serving
    /// traffic). Expired entries (expiries already in the past) are dropped.
    pub fn restore(&self, snaps: Vec<BucketSnapshot>) {
        let now = now_epoch();
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        for s in snaps {
            let mut b = Bucket::default();
            if s.fixed_expires_at > now {
                b.fixed_count = s.fixed_count;
                b.fixed_expires = epoch_to_systime(s.fixed_expires_at);
            }
            if s.block_until_at > now {
                b.block_until = epoch_to_systime(s.block_until_at);
                b.block_scope = Some(BlockScope::Temp);
            }
            if b.fixed_expires.is_some() || b.block_until.is_some() {
                map.insert(s.key, b);
            }
        }
    }
}

fn count_since(attempts: &VecDeque<Epoch>, now: Epoch, window: Duration) -> u64 {
    let threshold = now.saturating_sub(window.as_secs() as i64);
    attempts.iter().filter(|&&t| t >= threshold).count() as u64
}

#[async_trait]
impl RateLimitStore for InMemoryStore {
    async fn incr_expire(&self, key: &str, window: Duration) -> Result<(u64, u64), GateError> {
        let now = SystemTime::now();
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let b = map.entry(key.to_string()).or_default();
        if b.fixed_expires.map_or(true, |t| t <= now) {
            b.fixed_count = 0;
            b.fixed_expires = Some(now + window);
        }
        b.fixed_count = b.fixed_count.saturating_add(1);
        let ttl = remaining_secs_with_future(b.fixed_expires, window);
        Ok((b.fixed_count, ttl))
    }

    async fn abuse_check(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
    ) -> Result<LimiterDecision, GateError> {
        let now = SystemTime::now();
        let now_e = now_epoch();
        let short_window = Duration::from_secs(cfg.temp_block_range as u64);
        let long_window = Duration::from_secs(cfg.block_range as u64);
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let b = map.entry(key.to_string()).or_default();

        // Drop attempts older than the long window.
        let long_cutoff = now_e.saturating_sub(cfg.block_range as i64);
        while b.attempts.front().map_or(false, |&t| t < long_cutoff) {
            b.attempts.pop_front();
        }

        // If a block is still active, count this attempt but keep the block.
        if let Some(until) = b.block_until {
            if until > now {
                b.attempts.push_back(now_e);
                let short_count = count_since(&b.attempts, now_e, short_window);
                let long_count = b.attempts.len() as u64;
                let remaining = remaining_secs_with_future(Some(until), Duration::ZERO);
                let scope = b.block_scope.unwrap_or(BlockScope::Temp);
                return Ok(LimiterDecision::Blocked {
                    scope,
                    retry_after_secs: remaining,
                    short_count,
                    long_count,
                });
            } else {
                b.block_until = None;
                b.block_scope = None;
            }
        }

        b.attempts.push_back(now_e);
        let short_count = count_since(&b.attempts, now_e, short_window);
        let long_count = b.attempts.len() as u64;

        if short_count as usize >= cfg.temp_block_attempts {
            b.block_until = Some(now + Duration::from_secs(cfg.temp_block_duration as u64));
            b.block_scope = Some(BlockScope::Temp);
            Ok(LimiterDecision::Blocked {
                scope: BlockScope::Temp,
                retry_after_secs: cfg.temp_block_duration as u64,
                short_count,
                long_count,
            })
        } else if long_count as usize >= cfg.block_retry_limit {
            b.block_until = Some(now + Duration::from_secs(cfg.block_duration as u64));
            b.block_scope = Some(BlockScope::Long);
            Ok(LimiterDecision::Blocked {
                scope: BlockScope::Long,
                retry_after_secs: cfg.block_duration as u64,
                short_count,
                long_count,
            })
        } else {
            Ok(LimiterDecision::Allowed {
                short_count,
                long_count,
            })
        }
    }

    async fn set_nx_ex(&self, key: &str, ttl: Duration) -> Result<bool, GateError> {
        let now = SystemTime::now();
        let mut claims = self.claims.lock().expect("claims map poisoned");
        let claimable = claims.get(key).map_or(true, |&expires| expires <= now);
        if claimable {
            claims.insert(key.to_string(), now + ttl);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn del(&self, key: &str) -> Result<(), GateError> {
        let mut claims = self.claims.lock().expect("claims map poisoned");
        claims.remove(key);
        Ok(())
    }
}

/// `remaining_secs` expressed for a *future* `SystemTime` (the common case for
/// live buckets): returns secs until `t`, or `fallback` on None/elapsed.
fn remaining_secs_with_future(t: Option<SystemTime>, fallback: Duration) -> u64 {
    match t {
        Some(t) => t
            .duration_since(SystemTime::now())
            .map(|d| d.as_secs())
            .unwrap_or(fallback.as_secs()),
        None => fallback.as_secs(),
    }
}

/// Blanket impl so an `Arc<RateLimitStore>` (e.g. `Arc<InMemoryStore>` held in
/// the app's `AppState`) can be passed anywhere a `&dyn RateLimitStore` is
/// expected without manual deref at every call site.
#[async_trait]
impl<T: RateLimitStore> RateLimitStore for std::sync::Arc<T> {
    async fn incr_expire(&self, key: &str, window: Duration) -> Result<(u64, u64), GateError> {
        (**self).incr_expire(key, window).await
    }
    async fn abuse_check(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
    ) -> Result<LimiterDecision, GateError> {
        (**self).abuse_check(key, cfg).await
    }
    async fn set_nx_ex(&self, key: &str, ttl: Duration) -> Result<bool, GateError> {
        (**self).set_nx_ex(key, ttl).await
    }
    async fn del(&self, key: &str) -> Result<(), GateError> {
        (**self).del(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: AbuseLimiterConfig = AbuseLimiterConfig {
        temp_block_attempts: 3,
        temp_block_range: 60,
        temp_block_duration: 300,
        block_retry_limit: 5,
        block_range: 3600,
        block_duration: 86400,
    };

    #[tokio::test]
    async fn incr_expire_counts_within_window() {
        let s = InMemoryStore::default();
        let (c1, _) = s.incr_expire("k", Duration::from_secs(60)).await.unwrap();
        let (c2, _) = s.incr_expire("k", Duration::from_secs(60)).await.unwrap();
        assert_eq!((c1, c2), (1, 2));
    }

    #[tokio::test]
    async fn dedup_claim_release_cycle() {
        let s = InMemoryStore::default();
        assert!(s.set_nx_ex("k", Duration::from_secs(60)).await.unwrap());
        assert!(!s.set_nx_ex("k", Duration::from_secs(60)).await.unwrap());
        s.del("k").await.unwrap();
        assert!(s.set_nx_ex("k", Duration::from_secs(60)).await.unwrap());
    }

    #[tokio::test]
    async fn abuse_temp_blocks_at_short_threshold() {
        let s = InMemoryStore::default();
        for _ in 0..2 {
            assert!(matches!(
                s.abuse_check("ip", CFG).await.unwrap(),
                LimiterDecision::Allowed { .. }
            ));
        }
        assert!(matches!(
            s.abuse_check("ip", CFG).await.unwrap(),
            LimiterDecision::Blocked { scope: BlockScope::Temp, .. }
        ));
        assert!(matches!(
            s.abuse_check("ip", CFG).await.unwrap(),
            LimiterDecision::Blocked { .. }
        ));
    }

    #[tokio::test]
    async fn snapshot_restore_preserves_active_block() {
        let s = InMemoryStore::default();
        for _ in 0..3 {
            let _ = s.abuse_check("ip", CFG).await.unwrap();
        }
        let snaps = s.snapshot();
        assert!(snaps.iter().any(|s| s.block_until_at > now_epoch()));

        let s2 = InMemoryStore::default();
        s2.restore(snaps);
        assert!(matches!(
            s2.abuse_check("ip", CFG).await.unwrap(),
            LimiterDecision::Blocked { .. }
        ));
    }
}
