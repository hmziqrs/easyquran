use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::abuse::{
    credential_delay_secs, AbuseLimiterConfig, BlockScope, FailureMode, GateDecision,
    LimiterDecision, IP_RETRY_CAP_SECS,
};
use crate::error::GateError;

type Epoch = i64;

// Longest attempt-rolling window across all callers' AbuseLimiterConfig (block_range; abuse_check itself pops attempts older than this). prune() has no cfg, so this bounds how long an in-progress attempt chain survives eviction.
const MAX_ATTEMPT_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

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

// Count + block must stay under one lock; splitting them lets concurrent attackers race past the threshold (no test pins this).
#[async_trait]
pub trait RateLimitStore: Send + Sync {
    async fn incr_expire(&self, key: &str, window: Duration) -> Result<(u64, u64), GateError>;

    async fn abuse_check(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
    ) -> Result<LimiterDecision, GateError>;

    async fn set_nx_ex(&self, key: &str, ttl: Duration) -> Result<bool, GateError>;

    async fn del(&self, key: &str) -> Result<(), GateError>;

    // Failure-only path (A8). abuse_peek is read-only and runs before a
    // credential or IP check. abuse_record appends one failed attempt and
    // evaluates the threshold; it never appends on the blocked path. A success
    // is cleared with clear_limit. abuse_check above keeps the count-every-call
    // semantics for senders and limiters that are not login-shaped.
    async fn abuse_peek(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
        mode: FailureMode,
    ) -> Result<GateDecision, GateError>;

    async fn abuse_record(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
        mode: FailureMode,
    ) -> Result<GateDecision, GateError>;

    // Read-only active-ban lookup; unlike abuse_check it MUST NOT append an
    // attempt event (W3a escalation calls this on every request before the
    // fixed-window increment, so appending would inflate the suspicious counter).
    async fn ban_status(&self, key: &str) -> Result<Option<BanStatus>, GateError>;

    // Removes a limit bucket (fixed count + attempts + block) — distinct from
    // del(key), which releases a dedup-claim. Used by admin un-ban to clear L1.
    async fn clear_limit(&self, key: &str) -> Result<(), GateError>;

    // Sets/overwrites an active ban on a bucket (block_until + scope). W3a
    // escalation uses this to create a Temp/Long ban under `quran-ban:{unit}`;
    // it survives restart via the existing snapshot/flush (W3b) and is lifted by
    // clear_limit (W3c). Idempotent: a Temp→Long upgrade overwrites in place.
    async fn set_block(
        &self,
        key: &str,
        scope: BlockScope,
        duration: Duration,
    ) -> Result<(), GateError>;

    // Appends a qualifying-blocked-window timestamp to a bucket's attempt history
    // (the L1-only escalation history), prunes entries older than `long_window`,
    // and returns how many qualifying windows fall inside the temp and long
    // windows. Unlike abuse_check this NEVER evaluates a threshold or sets a
    // block — the caller (escalation engine) decides Temp/Long from the counts.
    async fn record_qualifying(
        &self,
        key: &str,
        temp_window: Duration,
        long_window: Duration,
    ) -> Result<QualifyingCounts, GateError>;
}

// Counts returned by record_qualifying: how many qualifying blocked windows a
// unit accumulated inside the Temp and Long evaluation windows. Long is
// evaluated before Temp by the escalation engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QualifyingCounts {
    pub temp_count: u32,
    pub long_count: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BucketSnapshot {
    pub key: String,
    pub fixed_count: u64,
    pub fixed_expires_at: Epoch,
    pub block_until_at: Epoch,
    pub block_scope: BlockScope,
}

// Read-only view of an active block; returned by ban_status() and consumed by
// inspection/export. Carries scope + absolute expiry so callers never re-read
// the bucket mutex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BanStatus {
    pub scope: BlockScope,
    pub block_until_at: Epoch,
}

#[derive(Default)]
struct Bucket {
    fixed_count: u64,
    fixed_expires: Option<SystemTime>,
    attempts: VecDeque<Epoch>,
    block_until: Option<SystemTime>,
    block_scope: Option<BlockScope>,
}

#[derive(Default)]
pub struct InMemoryStore {
    limits: Mutex<HashMap<String, Bucket>>,
    claims: Mutex<HashMap<String, SystemTime>>,
    saturation: AtomicU64,
}

impl InMemoryStore {
    pub fn snapshot(&self) -> Vec<BucketSnapshot> {
        let now = now_epoch();
        let map = self.limits.lock().expect("rate-limit map poisoned");
        map.iter()
            .filter_map(|(key, b)| {
                let fixed_expires_at = match b.fixed_expires {
                    Some(t) => t
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
                    None => 0,
                };
                let block_until_at = match b.block_until {
                    Some(t) => t
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0),
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
                        block_scope: b.block_scope.unwrap_or(BlockScope::Temp),
                    })
                }
            })
            .collect()
    }

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
                b.block_scope = Some(s.block_scope);
            }
            if b.fixed_expires.is_some() || b.block_until.is_some() {
                map.insert(s.key, b);
            }
        }
    }

    pub fn prune(&self) {
        let now = SystemTime::now();
        let attempt_cutoff = now_epoch().saturating_sub(MAX_ATTEMPT_WINDOW.as_secs() as i64);
        {
            let mut map = self.limits.lock().expect("rate-limit map poisoned");
            map.retain(|_, b| {
                b.fixed_expires.is_some_and(|t| t > now)
                    || b.block_until.is_some_and(|t| t > now)
                    || b.attempts.back().is_some_and(|&t| t >= attempt_cutoff)
            });
        }
        // Dedup claims have no other eviction path: set_nx_ex only overwrites an
        // expired SAME-key claim and del removes one explicit key, so distinct
        // expired keys would accumulate without bound. An expired claim is
        // already logically released (set_nx_ex treats expires<=now as
        // claimable), so keep only expires>now — never an unexpired claim.
        {
            let mut claims = self.claims.lock().expect("claims map poisoned");
            claims.retain(|_, expires| *expires > now);
        }
    }

    /// Total dedup-claim entries held (live plus not-yet-pruned expired).
    /// After `prune()` this equals the live claim-set size.
    pub fn claims_count(&self) -> usize {
        self.claims.lock().expect("claims map poisoned").len()
    }

    /// Live buckets currently serving an active block (block_until > now).
    /// Counts every active-ban key class (IP, email, user-id); the
    /// QURAN_ACTIVE_BAN_MAX gate consumes this to decline new ban state.
    pub fn active_ban_count(&self) -> usize {
        let now = SystemTime::now();
        self.limits
            .lock()
            .expect("rate-limit map poisoned")
            .values()
            .filter(|b| b.block_until.is_some_and(|t| t > now))
            .count()
    }

    /// Monotonic count of times the active-ban set was at/over capacity and a
    /// new ban (or its durable row) had to be declined. Saturation only ever
    /// grows; it never evicts an active ban and never disables fixed limiting.
    pub fn saturation_count(&self) -> u64 {
        self.saturation.load(Ordering::Relaxed)
    }

    pub fn incr_saturation(&self) {
        self.saturation.fetch_add(1, Ordering::Relaxed);
    }

    /// Number of `quran-ban:{unit}` buckets currently held (with qualifying
    /// history and/or an active ban). The W3a escalation capacity gate consumes
    /// this to bound tracked identities at QURAN_ESCALATION_MAX_IDENTITIES.
    pub fn ban_unit_history_count(&self) -> usize {
        self.limits
            .lock()
            .expect("rate-limit map poisoned")
            .keys()
            .filter(|k| k.starts_with("quran-ban:"))
            .count()
    }

    /// Drops `quran-ban:{unit}` buckets that carry no active block AND whose
    /// newest qualifying attempt is older than `long_window` (stale history).
    /// Active bans (Temp or Long) are never removed — they are never capacity
    /// victims. Returns how many buckets were dropped. Called by the escalation
    /// engine only when at capacity, before declining a new tracked unit.
    pub fn prune_escalation_history(&self, long_window: Duration) -> usize {
        let now = SystemTime::now();
        let long_cutoff = now_epoch().saturating_sub(long_window.as_secs() as i64);
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let before = map.len();
        map.retain(|key, b| {
            if !key.starts_with("quran-ban:") {
                return true;
            }
            // Never drop an active ban (Temp or Long) — invariant: active bans
            // are never capacity victims.
            if b.block_until.is_some_and(|t| t > now) {
                return true;
            }
            // Keep only units with at least one qualifying attempt inside the
            // long window; fully-stale history is reclaimable capacity.
            b.attempts.back().is_some_and(|&t| t >= long_cutoff)
        });
        before - map.len()
    }
}

fn prune_failures(b: &mut Bucket, cfg: AbuseLimiterConfig, now_e: Epoch) {
    let cutoff = now_e.saturating_sub(cfg.block_range as i64);
    while b.attempts.front().is_some_and(|&t| t < cutoff) {
        b.attempts.pop_front();
    }
}

fn active_block_remaining(b: &Bucket, now: SystemTime) -> Option<u64> {
    b.block_until
        .filter(|&until| until > now)
        .map(|until| remaining_secs_with_future(Some(until), Duration::ZERO))
}

fn credential_decision(failures: u64, cfg: AbuseLimiterConfig) -> GateDecision {
    match credential_delay_secs(failures, cfg.temp_block_attempts as u64) {
        0 => GateDecision::Pass,
        secs => GateDecision::Delay { secs },
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
        if b.fixed_expires.is_none_or(|t| t <= now) {
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
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let b = map.entry(key.to_string()).or_default();

        let long_cutoff = now_e.saturating_sub(cfg.block_range as i64);
        while b.attempts.front().is_some_and(|&t| t < long_cutoff) {
            b.attempts.pop_front();
        }

        if let Some(until) = b.block_until {
            if until > now {
                // Blocked retries never append: a blocked identity must not
                // extend its own block (A8).
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

    async fn abuse_peek(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
        mode: FailureMode,
    ) -> Result<GateDecision, GateError> {
        let now = SystemTime::now();
        let now_e = now_epoch();
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let Some(b) = map.get_mut(key) else {
            return Ok(GateDecision::Pass);
        };
        prune_failures(b, cfg, now_e);
        match mode {
            FailureMode::Ip => match active_block_remaining(b, now) {
                Some(retry) => Ok(GateDecision::Blocked {
                    retry_after_secs: retry.min(IP_RETRY_CAP_SECS),
                }),
                None => Ok(GateDecision::Pass),
            },
            FailureMode::Credential => Ok(credential_decision(b.attempts.len() as u64, cfg)),
        }
    }

    async fn abuse_record(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
        mode: FailureMode,
    ) -> Result<GateDecision, GateError> {
        let now = SystemTime::now();
        let now_e = now_epoch();
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let b = map.entry(key.to_string()).or_default();
        if mode == FailureMode::Ip {
            if let Some(retry) = active_block_remaining(b, now) {
                // Blocked path: report the block, append nothing (A8).
                return Ok(GateDecision::Blocked {
                    retry_after_secs: retry.min(IP_RETRY_CAP_SECS),
                });
            }
        }
        prune_failures(b, cfg, now_e);
        b.attempts.push_back(now_e);
        let failures = b.attempts.len() as u64;
        match mode {
            FailureMode::Credential => Ok(credential_decision(failures, cfg)),
            FailureMode::Ip => {
                if failures < cfg.temp_block_attempts as u64 {
                    return Ok(GateDecision::Pass);
                }
                let secs = (cfg.temp_block_duration as u64).min(IP_RETRY_CAP_SECS);
                b.block_until = Some(now + Duration::from_secs(secs));
                b.block_scope = Some(BlockScope::Temp);
                Ok(GateDecision::Blocked {
                    retry_after_secs: secs,
                })
            }
        }
    }

    async fn set_nx_ex(&self, key: &str, ttl: Duration) -> Result<bool, GateError> {
        let now = SystemTime::now();
        let mut claims = self.claims.lock().expect("claims map poisoned");
        let claimable = claims.get(key).is_none_or(|&expires| expires <= now);
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

    async fn ban_status(&self, key: &str) -> Result<Option<BanStatus>, GateError> {
        let now = SystemTime::now();
        let map = self.limits.lock().expect("rate-limit map poisoned");
        let Some(b) = map.get(key) else {
            return Ok(None);
        };
        match b.block_until {
            Some(until) if until > now => {
                let block_until_at = until
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                Ok(Some(BanStatus {
                    scope: b.block_scope.unwrap_or(BlockScope::Temp),
                    block_until_at,
                }))
            }
            _ => Ok(None),
        }
    }

    async fn clear_limit(&self, key: &str) -> Result<(), GateError> {
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        map.remove(key);
        Ok(())
    }

    async fn set_block(
        &self,
        key: &str,
        scope: BlockScope,
        duration: Duration,
    ) -> Result<(), GateError> {
        let now = SystemTime::now();
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let b = map.entry(key.to_string()).or_default();
        b.block_until = Some(now + duration);
        b.block_scope = Some(scope);
        Ok(())
    }

    async fn record_qualifying(
        &self,
        key: &str,
        temp_window: Duration,
        long_window: Duration,
    ) -> Result<QualifyingCounts, GateError> {
        let now_e = now_epoch();
        let long_cutoff = now_e.saturating_sub(long_window.as_secs() as i64);
        let temp_cutoff = now_e.saturating_sub(temp_window.as_secs() as i64);
        let mut map = self.limits.lock().expect("rate-limit map poisoned");
        let b = map.entry(key.to_string()).or_default();
        while b.attempts.front().is_some_and(|&t| t < long_cutoff) {
            b.attempts.pop_front();
        }
        // One timestamp per qualifying window — the escalation engine calls this
        // at most once per fixed window (count == max+1 equality), so this never
        // appends on every 429. Cap at a sane per-unit ceiling as a guard.
        if (b.attempts.len() as u64) < long_window.as_secs() {
            b.attempts.push_back(now_e);
        }
        let temp_count = b.attempts.iter().filter(|&&t| t >= temp_cutoff).count() as u32;
        let long_count = b.attempts.len() as u32;
        Ok(QualifyingCounts {
            temp_count,
            long_count,
        })
    }
}

fn remaining_secs_with_future(t: Option<SystemTime>, fallback: Duration) -> u64 {
    match t {
        Some(t) => t
            .duration_since(SystemTime::now())
            .map(|d| d.as_secs())
            .unwrap_or(fallback.as_secs()),
        None => fallback.as_secs(),
    }
}

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
    async fn abuse_peek(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
        mode: FailureMode,
    ) -> Result<GateDecision, GateError> {
        (**self).abuse_peek(key, cfg, mode).await
    }
    async fn abuse_record(
        &self,
        key: &str,
        cfg: AbuseLimiterConfig,
        mode: FailureMode,
    ) -> Result<GateDecision, GateError> {
        (**self).abuse_record(key, cfg, mode).await
    }
    async fn set_nx_ex(&self, key: &str, ttl: Duration) -> Result<bool, GateError> {
        (**self).set_nx_ex(key, ttl).await
    }
    async fn del(&self, key: &str) -> Result<(), GateError> {
        (**self).del(key).await
    }
    async fn ban_status(&self, key: &str) -> Result<Option<BanStatus>, GateError> {
        (**self).ban_status(key).await
    }
    async fn clear_limit(&self, key: &str) -> Result<(), GateError> {
        (**self).clear_limit(key).await
    }
    async fn set_block(
        &self,
        key: &str,
        scope: BlockScope,
        duration: Duration,
    ) -> Result<(), GateError> {
        (**self).set_block(key, scope, duration).await
    }
    async fn record_qualifying(
        &self,
        key: &str,
        temp_window: Duration,
        long_window: Duration,
    ) -> Result<QualifyingCounts, GateError> {
        (**self)
            .record_qualifying(key, temp_window, long_window)
            .await
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
            LimiterDecision::Blocked {
                scope: BlockScope::Temp,
                ..
            }
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

    #[tokio::test]
    async fn snapshot_restore_preserves_long_scope() {
        let s = InMemoryStore::default();
        let future = now_epoch() + 3600;
        s.restore(vec![BucketSnapshot {
            key: "ip".to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at: future,
            block_scope: BlockScope::Long,
        }]);
        let snaps = s.snapshot();
        assert!(snaps
            .iter()
            .any(|s| { s.block_scope == BlockScope::Long && s.block_until_at > now_epoch() }));

        let s2 = InMemoryStore::default();
        s2.restore(snaps);
        match s2.abuse_check("ip", CFG).await.unwrap() {
            LimiterDecision::Blocked {
                scope: BlockScope::Long,
                ..
            } => (),
            other => panic!("expected Long block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn prune_drops_only_expired_claims() {
        let s = InMemoryStore::default();
        {
            let mut claims = s.claims.lock().unwrap();
            claims.insert(
                "live-1".to_string(),
                SystemTime::now() + Duration::from_secs(60),
            );
            claims.insert(
                "live-2".to_string(),
                SystemTime::now() + Duration::from_secs(60),
            );
            for i in 0..50 {
                claims.insert(
                    format!("expired-{i}"),
                    SystemTime::now() - Duration::from_secs(60),
                );
            }
        }
        assert_eq!(s.claims_count(), 52);
        s.prune();
        assert_eq!(s.claims_count(), 2);
    }

    #[tokio::test]
    async fn prune_never_releases_live_claim() {
        let s = InMemoryStore::default();
        assert!(s.set_nx_ex("live", Duration::from_secs(60)).await.unwrap());
        s.prune();
        // Still held: a second claim on the same unexpired key is refused.
        assert!(!s.set_nx_ex("live", Duration::from_secs(60)).await.unwrap());
    }

    #[tokio::test]
    async fn claims_count_returns_live_size_after_prune() {
        let s = InMemoryStore::default();
        let live = 7;
        for i in 0..live {
            assert!(s
                .set_nx_ex(&format!("live-{i}"), Duration::from_secs(60))
                .await
                .unwrap());
        }
        {
            let mut claims = s.claims.lock().unwrap();
            for i in 0..200 {
                claims.insert(
                    format!("expired-{i}"),
                    SystemTime::now() - Duration::from_secs(60),
                );
            }
        }
        assert_eq!(s.claims_count(), live + 200);
        s.prune();
        assert_eq!(s.claims_count(), live);
    }

    #[tokio::test]
    async fn ban_status_returns_active_block_without_appending() {
        let s = InMemoryStore::default();
        for _ in 0..3 {
            let _ = s.abuse_check("ip", CFG).await.unwrap();
        }
        // Now blocked. ban_status reads the active block side-effect-free.
        let before = {
            let map = s.limits.lock().unwrap();
            map.get("ip").map(|b| b.attempts.len()).unwrap()
        };
        let status = s.ban_status("ip").await.unwrap();
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.scope, BlockScope::Temp);
        assert!(status.block_until_at > now_epoch());
        // No attempt was appended by the read-only lookup.
        let after = {
            let map = s.limits.lock().unwrap();
            map.get("ip").map(|b| b.attempts.len()).unwrap()
        };
        assert_eq!(before, after, "ban_status must not append an attempt event");
    }

    #[tokio::test]
    async fn ban_status_none_when_no_active_block() {
        let s = InMemoryStore::default();
        assert!(s.ban_status("absent").await.unwrap().is_none());
        let _ = s
            .incr_expire("rate", Duration::from_secs(60))
            .await
            .unwrap();
        // Fixed-window bucket without a block is not a ban.
        assert!(s.ban_status("rate").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn clear_limit_removes_bucket_but_not_claims() {
        let s = InMemoryStore::default();
        for _ in 0..3 {
            let _ = s.abuse_check("ip", CFG).await.unwrap();
        }
        // A dedup claim coexists under a distinct key.
        assert!(s.set_nx_ex("dedup", Duration::from_secs(60)).await.unwrap());

        s.clear_limit("ip").await.unwrap();
        assert!(s.ban_status("ip").await.unwrap().is_none());
        let snaps = s.snapshot();
        assert!(!snaps.iter().any(|s| s.key == "ip"));

        // clear_limit is distinct from del: the claim survives it.
        assert_eq!(
            s.claims_count(),
            1,
            "clear_limit must not release a dedup claim"
        );
        // del removes the claim, not a bucket.
        s.del("dedup").await.unwrap();
        assert_eq!(s.claims_count(), 0);
    }

    #[tokio::test]
    async fn set_block_creates_ban_visible_to_ban_status() {
        let s = InMemoryStore::default();
        s.set_block(
            "quran-ban:203.0.113.5/32",
            BlockScope::Long,
            Duration::from_secs(3600),
        )
        .await
        .unwrap();
        let status = s.ban_status("quran-ban:203.0.113.5/32").await.unwrap();
        let status = status.expect("active ban expected");
        assert_eq!(status.scope, BlockScope::Long);
        assert!(status.block_until_at > now_epoch());
    }

    #[tokio::test]
    async fn set_block_upgrades_temp_to_long_in_place() {
        let s = InMemoryStore::default();
        s.set_block(
            "quran-ban:1.2.3.4/32",
            BlockScope::Temp,
            Duration::from_secs(60),
        )
        .await
        .unwrap();
        s.set_block(
            "quran-ban:1.2.3.4/32",
            BlockScope::Long,
            Duration::from_secs(86400),
        )
        .await
        .unwrap();
        let status = s.ban_status("quran-ban:1.2.3.4/32").await.unwrap().unwrap();
        assert_eq!(status.scope, BlockScope::Long);
    }

    #[tokio::test]
    async fn record_qualifying_counts_within_windows_and_neversets_block() {
        let s = InMemoryStore::default();
        let key = "quran-ban:203.0.113.5/32";
        // Three qualifying windows.
        for _ in 0..3 {
            s.record_qualifying(key, Duration::from_secs(3600), Duration::from_secs(86400))
                .await
                .unwrap();
        }
        let c = s
            .record_qualifying(key, Duration::from_secs(3600), Duration::from_secs(86400))
            .await
            .unwrap();
        assert_eq!(c.long_count, 4);
        assert_eq!(c.temp_count, 4);
        // record_qualifying never sets a block on its own.
        assert!(s.ban_status(key).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ban_unit_history_count_only_counts_quran_ban_keys() {
        let s = InMemoryStore::default();
        s.set_block(
            "quran-ban:1.2.3.4/32",
            BlockScope::Temp,
            Duration::from_secs(60),
        )
        .await
        .unwrap();
        let _ = s
            .record_qualifying(
                "quran-ban:5.6.7.8/32",
                Duration::from_secs(3600),
                Duration::from_secs(86400),
            )
            .await
            .unwrap();
        let _ = s
            .incr_expire("ratelimit:1.2.3.4:quran-v1", Duration::from_secs(60))
            .await;
        assert_eq!(s.ban_unit_history_count(), 2);
    }

    #[tokio::test]
    async fn prune_escalation_history_keeps_active_bans_drops_stale() {
        let s = InMemoryStore::default();
        // Active ban: must survive prune.
        s.set_block(
            "quran-ban:10.0.0.1/32",
            BlockScope::Long,
            Duration::from_secs(3600),
        )
        .await
        .unwrap();
        // Stale history (old attempt, no active block): reclaimable.
        {
            let mut map = s.limits.lock().unwrap();
            let b = map.entry("quran-ban:10.0.0.2/32".to_string()).or_default();
            b.attempts.push_back(now_epoch() - 100_000);
        }
        // Recent history (no active block): kept.
        let _ = s
            .record_qualifying(
                "quran-ban:10.0.0.3/32",
                Duration::from_secs(3600),
                Duration::from_secs(86400),
            )
            .await
            .unwrap();

        let dropped = s.prune_escalation_history(Duration::from_secs(86400));
        assert_eq!(dropped, 1);
        assert_eq!(s.ban_unit_history_count(), 2);
        // Active ban untouched.
        assert!(s
            .ban_status("quran-ban:10.0.0.1/32")
            .await
            .unwrap()
            .is_some());
        // Stale unit gone.
        let snaps = s.snapshot();
        assert!(!snaps.iter().any(|s| s.key == "quran-ban:10.0.0.2/32"));
    }

    // Login-shaped limits (auth ABUSE_LIMITER_CONFIG): 3 failures, 900 s window.
    const AUTH_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
        temp_block_attempts: 3,
        temp_block_range: 360,
        temp_block_duration: 3600,
        block_retry_limit: 5,
        block_range: 900,
        block_duration: 86400,
    };

    fn attempts_len(s: &InMemoryStore, key: &str) -> usize {
        s.limits
            .lock()
            .unwrap()
            .get(key)
            .map(|b| b.attempts.len())
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn credential_failures_delay_and_never_block() {
        let s = InMemoryStore::default();
        let mut seen = Vec::new();
        for _ in 0..10 {
            let d = s
                .abuse_record("login:a@x.io", AUTH_CFG, FailureMode::Credential)
                .await
                .unwrap();
            assert!(!matches!(d, GateDecision::Blocked { .. }));
            seen.push(d);
        }
        let expected = [
            GateDecision::Pass,
            GateDecision::Pass,
            GateDecision::Delay { secs: 1 },
            GateDecision::Delay { secs: 2 },
            GateDecision::Delay { secs: 4 },
            GateDecision::Delay { secs: 8 },
            GateDecision::Delay { secs: 8 },
            GateDecision::Delay { secs: 8 },
            GateDecision::Delay { secs: 8 },
            GateDecision::Delay { secs: 8 },
        ];
        assert_eq!(seen, expected);
        assert_eq!(
            s.abuse_peek("login:a@x.io", AUTH_CFG, FailureMode::Credential)
                .await
                .unwrap(),
            GateDecision::Delay { secs: 8 }
        );
    }

    #[tokio::test]
    async fn credential_peek_never_appends() {
        let s = InMemoryStore::default();
        for _ in 0..3 {
            s.abuse_record("totp:7", AUTH_CFG, FailureMode::Credential)
                .await
                .unwrap();
        }
        let before = attempts_len(&s, "totp:7");
        for _ in 0..20 {
            let _ = s
                .abuse_peek("totp:7", AUTH_CFG, FailureMode::Credential)
                .await
                .unwrap();
        }
        assert_eq!(attempts_len(&s, "totp:7"), before);
    }

    #[tokio::test]
    async fn credential_success_clear_resets_delay() {
        let s = InMemoryStore::default();
        for _ in 0..4 {
            s.abuse_record("reauth:9", AUTH_CFG, FailureMode::Credential)
                .await
                .unwrap();
        }
        s.clear_limit("reauth:9").await.unwrap();
        assert_eq!(
            s.abuse_peek("reauth:9", AUTH_CFG, FailureMode::Credential)
                .await
                .unwrap(),
            GateDecision::Pass
        );
    }

    #[tokio::test]
    async fn ip_mode_blocks_with_capped_retry_and_no_append_while_blocked() {
        let s = InMemoryStore::default();
        let key = "forgot_password_verify:203.0.113.9";
        let mut last = GateDecision::Pass;
        for _ in 0..3 {
            last = s
                .abuse_record(key, AUTH_CFG, FailureMode::Ip)
                .await
                .unwrap();
        }
        // temp_block_duration 3600 is capped to the 900 s IP ceiling.
        assert_eq!(
            last,
            GateDecision::Blocked {
                retry_after_secs: 900
            }
        );
        let until = s.limits.lock().unwrap().get(key).unwrap().block_until;
        let appended = attempts_len(&s, key);

        for _ in 0..5 {
            let peek = s.abuse_peek(key, AUTH_CFG, FailureMode::Ip).await.unwrap();
            assert!(
                matches!(peek, GateDecision::Blocked { retry_after_secs } if retry_after_secs <= 900)
            );
            let rec = s
                .abuse_record(key, AUTH_CFG, FailureMode::Ip)
                .await
                .unwrap();
            assert!(matches!(rec, GateDecision::Blocked { .. }));
        }
        // Blocked retries neither extend the block nor append attempts.
        assert_eq!(attempts_len(&s, key), appended);
        assert_eq!(
            s.limits.lock().unwrap().get(key).unwrap().block_until,
            until
        );
    }

    #[tokio::test]
    async fn ip_mode_has_no_long_tier() {
        let s = InMemoryStore::default();
        // block_retry_limit 5 across the whole window must not produce a 24 h block.
        let key = "forgot_password:198.51.100.4";
        for _ in 0..2 {
            assert_eq!(
                s.abuse_record(key, AUTH_CFG, FailureMode::Ip)
                    .await
                    .unwrap(),
                GateDecision::Pass
            );
        }
        assert!(matches!(
            s.abuse_record(key, AUTH_CFG, FailureMode::Ip)
                .await
                .unwrap(),
            GateDecision::Blocked {
                retry_after_secs: 900
            }
        ));
    }

    #[tokio::test]
    async fn abuse_check_blocked_path_does_not_append() {
        let s = InMemoryStore::default();
        for _ in 0..3 {
            let _ = s.abuse_check("ip-blocked", CFG).await.unwrap();
        }
        let appended = attempts_len(&s, "ip-blocked");
        for _ in 0..5 {
            assert!(matches!(
                s.abuse_check("ip-blocked", CFG).await.unwrap(),
                LimiterDecision::Blocked { .. }
            ));
        }
        assert_eq!(attempts_len(&s, "ip-blocked"), appended);
    }
}
