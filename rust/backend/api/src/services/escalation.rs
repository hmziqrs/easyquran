//! W3a ban escalation: promote a suspicious, sustained rate-limit abuser from
//! fixed-window 429s to a durable Temp/Long ban. Default-OFF; a no-op until
//! `QURAN_BAN_ESCALATION_ENABLED=true` AND the hard-gate checklist passes.
//!
//! State split:
//! - **suspicious counter** (per fixed window, per unit): L1-only, here. Reset
//!   when the outer limiter signals `count == 1`. Bounded by
//!   `max_tracked_identities` via LRU prune.
//! - **qualifying-window history** (timestamps per unit): L1-only, in the gate
//!   store bucket under `quran-ban:{unit}`. Cleared by the W3c admin un-ban
//!   (`clear_limit`), so a lifted ban cannot immediately re-trigger.
//! - **active ban** (scope + expiry): L2-persistent via the store snapshot/flush
//!   (W3b); lifted by `clear_limit` (W3c).
//!
//! The store is the authority for bans/history; this struct owns only the
//! transient suspicious counter. A non-external identity, an allowlisted IP, or
//! a disabled flag short-circuits every method to a no-op.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use ipnet::IpNet;
use rux_request_gate::{BlockScope, Escalation, InMemoryStore, RateLimitStore, RequestIdentity};
use tracing::warn;

use crate::config::settings::EscalationConfig;
use crate::modules::admin_bans_v1::dto::BanUnit;
use crate::modules::quran_v1::error::QuranErrorClass;

type Epoch = i64;

fn now_epoch() -> Epoch {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// Per-unit current-fixed-window suspicious-4xx counter. Transient: reset when the
// outer limiter reports count == 1 (start of a fresh fixed window).
#[derive(Clone, Copy, Default)]
struct SuspiciousWindow {
    window_epoch: Epoch,
    count: u32,
}

pub struct EscalationEngine {
    cfg: EscalationConfig,
    store: Arc<InMemoryStore>,
    suspicious: Mutex<HashMap<String, SuspiciousWindow>>,
}

impl EscalationEngine {
    pub fn new(cfg: EscalationConfig, store: Arc<InMemoryStore>) -> Self {
        Self {
            cfg,
            store,
            suspicious: Mutex::new(HashMap::new()),
        }
    }

    /// True when the feature is live. The layer also holds `None` when disabled,
    /// so this is defense-in-depth: even an attached engine no-ops when off.
    #[allow(dead_code)]
    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn external_ip(identity: &RequestIdentity) -> Option<IpAddr> {
        match identity {
            RequestIdentity::External(ip) => Some(*ip),
            // Internal service + absent identity: never an IP, never escalates.
            _ => None,
        }
    }

    // Match the RAW client address (before BanUnit normalization) against the
    // allowlist. An allowlisted unit never escalates and never appears in export.
    fn allowlisted(&self, ip: IpAddr) -> bool {
        self.cfg
            .allowlist
            .iter()
            .any(|net: &IpNet| net.contains(&ip))
    }

    fn ban_key(&self, unit_str: &str) -> String {
        format!("{}:{unit_str}", self.cfg.key_prefix)
    }

    fn suspicious_count_for(&self, unit_str: &str) -> u32 {
        self.suspicious
            .lock()
            .expect("suspicious map poisoned")
            .get(unit_str)
            .map(|w| w.count)
            .unwrap_or(0)
    }

    // Bound the transient suspicious map. Drops entries with the oldest
    // window_epoch (LRU by window start); never touches the store, so an active
    // ban in the store is unaffected (active bans are never capacity victims).
    fn prune_suspicious_capacity(map: &mut HashMap<String, SuspiciousWindow>, max: usize) {
        if map.len() <= max {
            return;
        }
        let mut entries: Vec<(String, Epoch)> = map
            .iter()
            .map(|(k, v)| (k.clone(), v.window_epoch))
            .collect();
        entries.sort_by_key(|(_, e)| *e);
        let to_remove = map.len().saturating_sub(max);
        for (k, _) in entries.into_iter().take(to_remove) {
            map.remove(&k);
        }
    }
}

#[async_trait]
impl Escalation for EscalationEngine {
    // active_ban is async (awaits the read-only store lookup); observe_allowed
    // is sync (no await, avoids the &Response Send boundary).
    async fn active_ban(&self, identity: &RequestIdentity) -> Option<u64> {
        if !self.cfg.enabled {
            return None;
        }
        let ip = Self::external_ip(identity)?;
        if self.allowlisted(ip) {
            return None;
        }
        let unit = BanUnit::from_ip(ip)?;
        let ban_key = self.ban_key(&unit.canonical());
        // Side-effect-free read-only lookup: NEVER appends a qualifying event
        // (this runs on every request before the fixed-window increment).
        match self.store.ban_status(&ban_key).await {
            Ok(Some(status)) => {
                let now = now_epoch();
                if status.block_until_at > now {
                    Some((status.block_until_at - now) as u64)
                } else {
                    None
                }
            }
            Ok(None) => None,
            // Best-effort: a store error must not 5xx the caller; proceed to the
            // fixed-window limiter, which has its own fail-closed semantics.
            Err(e) => {
                warn!(error = %e, "ban_status read failed (best-effort, no short-circuit)");
                None
            }
        }
    }

    fn observe_allowed(
        &self,
        identity: &RequestIdentity,
        count: u64,
        response: &axum::response::Response,
    ) {
        if !self.cfg.enabled {
            return;
        }
        let ip = match Self::external_ip(identity) {
            Some(ip) => ip,
            None => return,
        };
        if self.allowlisted(ip) {
            return;
        }
        let unit = match BanUnit::from_ip(ip) {
            Some(u) => u,
            None => return,
        };
        let unit_str = unit.canonical();
        // Read the typed classification extension — never JSON/message text.
        let class = response.extensions().get::<QuranErrorClass>();
        let now = now_epoch();
        let mut map = self.suspicious.lock().expect("suspicious map poisoned");
        if map.len() > self.cfg.max_tracked_identities {
            Self::prune_suspicious_capacity(&mut map, self.cfg.max_tracked_identities);
        }
        let entry = map.entry(unit_str).or_insert(SuspiciousWindow {
            window_epoch: now,
            count: 0,
        });
        // count == 1 is the one-event-per-window reset primitive: the first
        // request of a fresh fixed window restarts the suspicious counter.
        if count == 1 {
            entry.window_epoch = now;
            entry.count = 0;
        }
        if class.is_some() {
            entry.count = entry.count.saturating_add(1);
        }
    }

    async fn on_first_block(&self, identity: &RequestIdentity) -> Option<u64> {
        if !self.cfg.enabled {
            return None;
        }
        let ip = Self::external_ip(identity)?;
        if self.allowlisted(ip) {
            return None;
        }
        let unit = BanUnit::from_ip(ip)?;
        let unit_str = unit.canonical();
        let ban_key = self.ban_key(&unit_str);

        // A window qualifies ONLY when the suspicious counter (built during
        // count 1..max of this same window) meets its threshold. Raw volume
        // alone — a flood with no suspicious shape — never creates a ban.
        if self.suspicious_count_for(&unit_str) < self.cfg.suspicious_4xx_per_window {
            return None;
        }

        let long_window = Duration::from_secs(self.cfg.long_window_secs);
        let temp_window = Duration::from_secs(self.cfg.temp_window_secs);

        // Tracking capacity: bound distinct tracked units. Prune reclaimable
        // history first; active bans (Temp or Long) are never dropped. If still
        // full, decline new history/ban creation, keep fixed limiting, saturate.
        if self.store.ban_unit_history_count() >= self.cfg.max_tracked_identities {
            self.store.prune_escalation_history(long_window);
            if self.store.ban_unit_history_count() >= self.cfg.max_tracked_identities {
                self.store.incr_saturation();
                warn!(
                    unit = %unit_str,
                    "escalation history at capacity; declining new ban, fixed limiting continues"
                );
                return None;
            }
        }

        // Record the qualifying event and read the counts. This appends at most
        // once per fixed window (this method runs only at count == max+1).
        let counts = match self
            .store
            .record_qualifying(&ban_key, temp_window, long_window)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "record_qualifying failed (best-effort)");
                return None;
            }
        };

        // Active-ban capacity (QURAN_ACTIVE_BAN_MAX, reused from W3b). Never
        // exceed; an active ban set at capacity keeps fixed limiting and signals
        // saturation. History was already recorded above so a later, freer
        // capacity can still act on it.
        if self.store.active_ban_count() >= self.cfg.max_active_bans {
            self.store.incr_saturation();
            warn!(
                unit = %unit_str,
                "active-ban set at capacity; declining new ban creation, fixed limiting continues"
            );
            return None;
        }

        // Evaluate Long BEFORE Temp: once the long threshold is met, upgrade to
        // (or create) Long regardless of the temp threshold.
        if counts.long_count >= self.cfg.long_after {
            let dur = Duration::from_secs(self.cfg.long_duration_secs);
            if let Err(e) = self.store.set_block(&ban_key, BlockScope::Long, dur).await {
                warn!(error = %e, "set_block Long failed (best-effort)");
                return None;
            }
            tracing::info!(unit = %unit_str, scope = "Long", "escalated: qualifying-window threshold met");
            return Some(self.cfg.long_duration_secs);
        }
        if counts.temp_count >= self.cfg.temp_after {
            let dur = Duration::from_secs(self.cfg.temp_duration_secs);
            if let Err(e) = self.store.set_block(&ban_key, BlockScope::Temp, dur).await {
                warn!(error = %e, "set_block Temp failed (best-effort)");
                return None;
            }
            tracing::info!(unit = %unit_str, scope = "Temp", "escalated: qualifying-window threshold met");
            return Some(self.cfg.temp_duration_secs);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn cfg_with(temp_after: u32, long_after: u32, suspicious: u32) -> EscalationConfig {
        EscalationConfig {
            enabled: true,
            key_prefix: "quran-ban",
            temp_after,
            temp_window_secs: 3600,
            temp_duration_secs: 3600,
            long_after,
            long_window_secs: 86400,
            long_duration_secs: 604800,
            suspicious_4xx_per_window: suspicious,
            max_tracked_identities: 10_000,
            max_active_bans: 2_000,
            allowlist: Vec::new(),
        }
    }

    fn ip(a: u8, b: u8, c: u8, d: u8) -> RequestIdentity {
        RequestIdentity::External(IpAddr::V4(Ipv4Addr::new(a, b, c, d)))
    }

    fn suspicious_response() -> axum::response::Response {
        let mut resp = axum::response::Response::new(axum::body::Body::empty());
        resp.extensions_mut().insert(QuranErrorClass::UnknownRoute);
        resp
    }

    fn clean_response() -> axum::response::Response {
        axum::response::Response::new(axum::body::Body::empty())
    }

    // Drive one full fixed window to its first block (count == max+1), optionally
    // flooding it with suspicious allowed responses first. Returns whether a ban
    // was created at the qualifying block.
    async fn run_window_to_block(
        eng: &EscalationEngine,
        identity: &RequestIdentity,
        max: u64,
        suspicious_responses: u32,
    ) -> Option<u64> {
        // count 1..=max are allowed: reset happens at count==1, suspicious events
        // accumulate across the window.
        for c in 1..=max {
            let resp = if c as u32 <= suspicious_responses {
                suspicious_response()
            } else {
                clean_response()
            };
            eng.observe_allowed(identity, c, &resp);
        }
        // count == max+1 is the one qualifying-block evaluation per window.
        eng.on_first_block(identity).await
    }

    #[tokio::test]
    async fn disabled_engine_never_bans_or_observes() {
        let store = Arc::new(InMemoryStore::default());
        let mut cfg = cfg_with(5, 20, 1);
        cfg.enabled = false;
        let eng = EscalationEngine::new(cfg, store.clone());

        let id = ip(203, 0, 113, 5);
        // Flood of suspicious allowed responses + a qualifying block.
        for c in 1..=5u64 {
            eng.observe_allowed(&id, c, &suspicious_response());
        }
        assert!(eng.on_first_block(&id).await.is_none());
        assert!(eng.active_ban(&id).await.is_none());
        assert!(store.ban_unit_history_count() == 0);
    }

    #[tokio::test]
    async fn volume_without_suspicious_shape_never_bans() {
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 20, 20), store.clone());
        let id = ip(198, 51, 100, 7);

        // Many qualifying windows, but NONE carry a suspicious classification.
        for _ in 0..30 {
            let created = run_window_to_block(&eng, &id, 600, 0).await;
            assert!(created.is_none(), "raw volume must never create a ban");
        }
        assert!(eng.active_ban(&id).await.is_none());
        assert_eq!(store.ban_unit_history_count(), 0);
    }

    #[tokio::test]
    async fn qualifying_windows_reach_temp_then_long() {
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 20, 20), store.clone());
        let id = ip(203, 0, 113, 10);
        let ban_key = "quran-ban:203.0.113.10/32";

        // 5 qualifying windows → Temp.
        for _ in 0..5 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        let status = store
            .ban_status(ban_key)
            .await
            .unwrap()
            .expect("Temp ban expected after 5 qualifying windows");
        assert_eq!(status.scope, BlockScope::Temp);

        // Continue accumulating qualifying windows (on_first_block keeps
        // recording history; a Temp ban is overwritten until the Long threshold
        // is met). Once 20 qualifying windows land inside the long window,
        // record_block_event upgrades to Long. This mirrors the real path where
        // a Temp expires (ban_status -> None) and later qualifying windows push
        // the durable history to the Long threshold.
        for _ in 5..20 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        let status2 = store
            .ban_status(ban_key)
            .await
            .unwrap()
            .expect("ban expected after long threshold");
        assert_eq!(
            status2.scope,
            BlockScope::Long,
            "Long must win once the long threshold is met"
        );
    }

    #[tokio::test]
    async fn long_wins_when_both_thresholds_match_in_one_window() {
        // If temp_after == long_after, the Long branch is evaluated first and
        // must win (no Temp is ever created).
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 5, 20), store.clone());
        let id = ip(203, 0, 113, 20);

        for _ in 0..5 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        let status = store
            .ban_status("quran-ban:203.0.113.20/32")
            .await
            .unwrap()
            .expect("ban expected");
        assert_eq!(status.scope, BlockScope::Long);
    }

    #[tokio::test]
    async fn active_ban_blocks_request_one_after_window_rollover() {
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 20, 20), store.clone());
        let id = ip(203, 0, 113, 30);
        let ban_key = "quran-ban:203.0.113.30/32".to_string();

        // Create a Temp ban.
        for _ in 0..5 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        assert!(eng.active_ban(&id).await.is_some());

        // Simulate a fixed-window rollover: the next request would be count == 1
        // of a new window, but active_ban is checked BEFORE the increment and
        // must still short-circuit.
        eng.observe_allowed(&id, 1, &clean_response()); // new-window reset
        assert!(
            eng.active_ban(&id).await.is_some(),
            "active ban must hold after fixed-window rollover"
        );
        assert!(store.ban_status(&ban_key).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn active_ban_survives_restart_via_snapshot_restore() {
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 20, 20), store.clone());
        let id = ip(203, 0, 113, 40);
        for _ in 0..5 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        // Snapshot → fresh store → restore. The ban (block_until + scope) is in
        // the snapshot; the suspicious counter is L1-only and does not carry over.
        let snaps = store.snapshot();
        let store2 = Arc::new(InMemoryStore::default());
        store2.restore(snaps);
        let eng2 = EscalationEngine::new(cfg_with(5, 20, 20), store2.clone());
        assert!(
            eng2.active_ban(&id).await.is_some(),
            "ban must survive restart via L2 snapshot/restore"
        );
    }

    #[tokio::test]
    async fn observe_allowed_never_appends_qualifying_history() {
        // observe_allowed builds the suspicious counter but NEVER appends a
        // qualifying-window timestamp — only on_first_block does. A flood of
        // allowed suspicious responses without a qualifying block leaves no
        // history, so raw volume (even if suspicious-shaped) below the
        // qualifying block cannot accumulate a ban on its own.
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 20, 5), store.clone());
        let id = ip(203, 0, 113, 50);

        for c in 1..=600u64 {
            eng.observe_allowed(&id, c, &suspicious_response());
        }
        assert_eq!(
            store.ban_unit_history_count(),
            0,
            "observe_allowed must not create qualifying history"
        );
        // One qualifying block records exactly one event.
        eng.on_first_block(&id).await;
        assert_eq!(store.ban_unit_history_count(), 1);
    }

    #[tokio::test]
    async fn allowlisted_identity_never_bans() {
        let mut cfg = cfg_with(5, 20, 1);
        cfg.allowlist = vec!["203.0.113.0/24".parse().unwrap()];
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg, store.clone());
        let id = ip(203, 0, 113, 77); // inside the allowlisted /24

        for _ in 0..50 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        assert!(
            eng.active_ban(&id).await.is_none(),
            "allowlisted IP never bans"
        );
        assert_eq!(store.ban_unit_history_count(), 0);
    }

    #[tokio::test]
    async fn internal_identity_never_bans() {
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(5, 20, 1), store.clone());
        let id = RequestIdentity::InternalService(rux_request_gate::InternalServiceId::WebSsr);

        for _ in 0..50 {
            run_window_to_block(&eng, &id, 600, 20).await;
        }
        assert!(
            eng.active_ban(&id).await.is_none(),
            "internal identity never bans"
        );
        assert_eq!(store.ban_unit_history_count(), 0);
    }

    #[tokio::test]
    async fn capacity_saturation_preserves_active_bans_and_fixed_limiting() {
        // Tiny capacity: at most 2 tracked units, 2 active bans.
        let mut cfg = cfg_with(1, 100, 1);
        cfg.max_tracked_identities = 2;
        cfg.max_active_bans = 2;
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg, store.clone());

        // Fill the two tracked-unit slots with active bans.
        let a = ip(10, 0, 0, 1);
        let b = ip(10, 0, 0, 2);
        run_window_to_block(&eng, &a, 10, 1).await;
        run_window_to_block(&eng, &b, 10, 1).await;
        assert!(eng.active_ban(&a).await.is_some());
        assert!(eng.active_ban(&b).await.is_some());

        // A third unit at/over capacity: no new ban, fixed limiting continues.
        let c = ip(10, 0, 0, 3);
        let created = run_window_to_block(&eng, &c, 10, 1).await;
        assert!(created.is_none(), "capacity must decline a new ban");

        // Active bans are never capacity victims: both survive.
        assert!(eng.active_ban(&a).await.is_some());
        assert!(eng.active_ban(&b).await.is_some());
        // Saturation counter advanced.
        assert!(store.saturation_count() >= 1);
    }

    #[tokio::test]
    async fn suspicious_counter_resets_each_window() {
        // If 19 suspicious events (< threshold 20) arrive in a window, no ban.
        // A subsequent window with a fresh reset and 20+ events qualifies.
        let store = Arc::new(InMemoryStore::default());
        let eng = EscalationEngine::new(cfg_with(1, 100, 20), store.clone());
        let id = ip(203, 0, 113, 90);

        // Window 1: under threshold → no qualifying.
        let r = run_window_to_block(&eng, &id, 600, 19).await;
        assert!(r.is_none());
        // Window 2: count==1 resets the counter; 20 events qualify and (with
        // temp_after=1) immediately reach a Temp ban.
        let r2 = run_window_to_block(&eng, &id, 600, 20).await;
        assert!(
            r2.is_some(),
            "fresh window with threshold suspicious events qualifies"
        );
    }
}
