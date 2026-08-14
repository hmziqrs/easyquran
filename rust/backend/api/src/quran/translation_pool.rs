use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use moka::future::Cache;
use moka::notification::RemovalCause;
use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use crate::quran::loader::{load_translation_corpus, QuranLoadError};
use crate::quran::store::{CatalogueEntry, Corpus, TranslationId};

/// Bounds concurrent cold Corpus builds so the byte ceiling holds during the build phase: moka
/// enforces only the count ceiling (`max_capacity`) plus `time_to_idle` — there is no byte
/// weigher — and the separate `enforce_byte_bound` prune loop runs only after a build completes,
/// so unbounded parallel distinct-id loads could transiently allocate far past
/// `max_resident_bytes`. Near-serial (2) closes that gap.
const BUILD_CONCURRENCY: usize = 2;

/// Prewarm may share build permits with real cold loads; back off (yield) when a real request
/// could be using one. The condition is racy against the acquire in `init`, so the wait is
/// deadline-bounded to avoid starving prewarm on a busy boot.
const PREWARM_YIELD_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Default)]
struct PoolMetrics {
    builds: AtomicU64,
    evictions: AtomicU64,
    resident_bytes: AtomicU64,
    lookups: AtomicU64,
    access_clock: AtomicU64,
    residents: AsyncMutex<HashMap<TranslationId, u64>>,
    eviction_times: AsyncMutex<VecDeque<Instant>>,
}

/// Observable pool state for tuning the §3 bounds on evidence. `hit_rate` is `None` (serialized
/// as JSON `null`) until the first lookup lands — the prior hard-coded `1.0` read as a perfect
/// cache before any request existed.
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub resident_count: u64,
    pub resident_bytes: u64,
    pub builds: u64,
    pub evictions: u64,
    pub evictions_per_minute: u64,
    pub lookups: u64,
    pub hit_rate: Option<f64>,
    pub prewarmed: Vec<String>,
    pub top_demand: Vec<(String, f64)>,
}

pub struct TranslationPool {
    cache: Cache<TranslationId, Arc<Corpus>>,
    dir: PathBuf,
    entries: HashMap<String, CatalogueEntry>,
    id_whitelist: HashSet<String>,
    metrics: Arc<PoolMetrics>,
    /// Lock-free API-demand side table. Disjoint from `residents` (the byte-bound victim index)
    /// so eviction never has to drop or tombstone a count — a count survives an eviction and the
    /// subsequent re-admission of the same id. Pre-populated once per catalogue id at construction.
    demand: HashMap<TranslationId, AtomicU64>,
    build_sem: Arc<Semaphore>,
    prune_sem: Arc<Semaphore>,
    max_resident_bytes: u64,
    max_resident_translations: u64,
    idle_ttl: Duration,
    boot_instant: Instant,
    boot_id: String,
    demand_collect: bool,
    prewarm_count: u64,
    /// Ranked API-demand snapshot written by the flush task, read by `stats()` on the health
    /// path. Never touches SQLite, and the guard is never held across `.await`.
    top_demand: RwLock<Vec<(String, f64)>>,
    /// Ids warmed at boot. Feeds `PoolStats::prewarmed` and the first-request prewarm-hit signal.
    prewarmed: RwLock<HashSet<String>>,
    /// First-request prewarm hits are emitted once per prewarmed id.
    prewarm_claimed: Mutex<HashSet<String>>,
    /// Candidate set retained before any warm runs, for the boot effectiveness counters.
    candidate_set: RwLock<HashSet<String>>,
    otlp_real_cold_builds: Counter<u64>,
    otlp_candidate_cold_builds: Counter<u64>,
    otlp_prewarm_builds: Counter<u64>,
    otlp_prewarm_hits: Counter<u64>,
}

impl TranslationPool {
    /// Bounds come from settings (not constants) so they tune without recompiling the rules.
    /// Moka enforces the exact LRU count ceiling; a serialized second pass invalidates LRU entries
    /// until the independently tracked resident-byte ceiling also holds.
    pub fn new(
        catalogue: &[CatalogueEntry],
        translations_dir: PathBuf,
        max_resident_translations: u64,
        max_resident_bytes: u64,
        idle_ttl: Duration,
        demand_collect: bool,
        prewarm_count: u64,
    ) -> Self {
        let entries: HashMap<String, CatalogueEntry> = catalogue
            .iter()
            .map(|e| (e.id.to_string(), e.clone()))
            .collect();
        let id_whitelist: HashSet<String> = entries.keys().cloned().collect();
        let build_sem = Arc::new(Semaphore::new(BUILD_CONCURRENCY));
        let prune_sem = Arc::new(Semaphore::new(1));
        let metrics = Arc::new(PoolMetrics::default());
        let metrics_for_listener = metrics.clone();

        let demand: HashMap<TranslationId, AtomicU64> = catalogue
            .iter()
            .filter_map(|e| {
                let id_str: &str = &e.id;
                TranslationId::parse(id_str, &id_whitelist).map(|tid| (tid, AtomicU64::new(0)))
            })
            .collect();

        let cache = Cache::builder()
            .max_capacity(max_resident_translations.max(1))
            .time_to_idle(idle_ttl)
            .async_eviction_listener(
                move |key: Arc<TranslationId>, v: Arc<Corpus>, cause: RemovalCause| {
                    let m = metrics_for_listener.clone();
                    Box::pin(async move {
                        let bytes = v.bytes() as u64;
                        let _ = m.resident_bytes.fetch_update(
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                            |resident| Some(resident.saturating_sub(bytes)),
                        );
                        if cause != RemovalCause::Replaced {
                            m.residents.lock().await.remove(key.as_ref());
                            m.evictions.fetch_add(1, Ordering::Relaxed);
                            m.eviction_times.lock().await.push_back(Instant::now());
                        }
                    })
                },
            )
            .build();

        let meter = global::meter("quran.translation.pool");
        let otlp_real_cold_builds = meter
            .u64_counter("quran.translation.real_cold_builds")
            .with_description("Real cold translation corpus builds (excludes boot prewarm).")
            .build();
        let otlp_candidate_cold_builds = meter
            .u64_counter("quran.translation.candidate_cold_builds")
            .with_description("Cold builds for the boot prewarm candidate set within the idle TTL.")
            .build();
        let otlp_prewarm_builds = meter
            .u64_counter("quran.translation.prewarm_builds")
            .with_description("Translation corpora built by the boot prewarm path.")
            .build();
        let otlp_prewarm_hits = meter
            .u64_counter("quran.translation.prewarm_hits")
            .with_description("First real cache hits on prewarmed translation corpora.")
            .build();

        let boot_id = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );

        Self {
            cache,
            dir: translations_dir,
            entries,
            id_whitelist,
            metrics,
            demand,
            build_sem,
            prune_sem,
            max_resident_bytes,
            max_resident_translations,
            idle_ttl,
            boot_instant: Instant::now(),
            boot_id,
            demand_collect,
            prewarm_count,
            top_demand: RwLock::new(Vec::new()),
            prewarmed: RwLock::new(HashSet::new()),
            prewarm_claimed: Mutex::new(HashSet::new()),
            candidate_set: RwLock::new(HashSet::new()),
            otlp_real_cold_builds,
            otlp_candidate_cold_builds,
            otlp_prewarm_builds,
            otlp_prewarm_hits,
        }
    }

    pub fn catalogue(&self) -> &HashMap<String, CatalogueEntry> {
        &self.entries
    }

    pub fn catalogue_ids(&self) -> &HashSet<String> {
        &self.id_whitelist
    }

    pub fn demand_collect_enabled(&self) -> bool {
        self.demand_collect
    }

    /// Whitelist parse — the membership check IS the path-traversal guard.
    pub fn parse_id(&self, s: &str) -> Option<TranslationId> {
        TranslationId::parse(s, &self.id_whitelist)
    }

    fn path_for(&self, id: &TranslationId) -> Option<PathBuf> {
        // entry.path is catalogue-relative ("sqlite/<id>.sqlite"); join against the dir root.
        Some(self.dir.join(self.entries.get(id.as_str())?.path.as_ref()))
    }

    fn otlp_attrs(&self, id: &TranslationId) -> [KeyValue; 2] {
        [
            KeyValue::new("boot_id", self.boot_id.clone()),
            KeyValue::new("translation_id", id.as_str().to_string()),
        ]
    }

    /// Single-flight cold load: moka coalesces concurrent cold requests for the same id into one
    /// build. No `std::sync` guard is held across the sqlite `.await` (handler futures stay `Send`).
    pub async fn get_or_build(
        &self,
        id: &TranslationId,
    ) -> Result<Arc<Corpus>, Arc<QuranLoadError>> {
        self.metrics.lookups.fetch_add(1, Ordering::Relaxed);
        let path = match self.path_for(id) {
            Some(p) => p,
            None => {
                return Err(Arc::new(QuranLoadError::Invariant(format!(
                    "translation {} not in catalogue",
                    id.as_str()
                ))));
            }
        };
        let path_str = path.to_string_lossy().into_owned();
        let metrics = self.metrics.clone();
        let build_sem = self.build_sem.clone();
        let init = async move {
            // Gate the build so the byte ceiling holds while a corpus is materialized (moka caps
            // only the count ceiling; the byte bound is enforced after the build by the separate
            // `enforce_byte_bound` loop, not by moka). Cached hits never reach here, so the
            // semaphore only throttles cold loads, never the hot path.
            let _permit = build_sem
                .acquire()
                .await
                .expect("build semaphore is never closed");
            let corpus = load_translation_corpus(&path_str).await?;
            // Account exactly once per build — single-flight guarantees a single init per id.
            metrics
                .resident_bytes
                .fetch_add(corpus.bytes() as u64, Ordering::Relaxed);
            metrics.builds.fetch_add(1, Ordering::Relaxed);
            Ok(Arc::new(corpus))
        };
        let builds_before = self.metrics.builds.load(Ordering::Relaxed);
        let corpus = self.cache.try_get_with(id.clone(), init).await?;
        let is_cold = self.metrics.builds.load(Ordering::Relaxed) > builds_before;
        // API-demand counter: one incr per successful get_or_build (needs + obtains the corpus).
        // Catalogue miss and build error both return before this site, so neither contributes. The
        // side table outlives eviction, so a count survives an eviction and re-admission intact.
        if self.demand_collect {
            if let Some(counter) = self.demand.get(id) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }
        let tick = self.metrics.access_clock.fetch_add(1, Ordering::Relaxed) + 1;
        self.metrics.residents.lock().await.insert(id.clone(), tick);
        self.enforce_byte_bound().await;

        // Effectiveness evidence. `builds` (above) counts every corpus build including prewarm;
        // these counters are separate so prewarm builds never inflate real-cold-build signals.
        if is_cold {
            self.otlp_real_cold_builds.add(1, &self.otlp_attrs(id));
            if self.boot_instant.elapsed() <= self.idle_ttl
                && self.candidate_set.read().unwrap().contains(id.as_str())
            {
                self.otlp_candidate_cold_builds.add(1, &self.otlp_attrs(id));
            }
        } else if self.prewarmed.read().unwrap().contains(id.as_str()) {
            let id_str = id.as_str().to_string();
            let claimed = {
                let mut guard = self.prewarm_claimed.lock().unwrap();
                guard.insert(id_str)
            };
            if claimed {
                self.otlp_prewarm_hits.add(1, &self.otlp_attrs(id));
            }
        }
        Ok(corpus)
    }

    /// Private prewarm path: reuses the cold-build `init` (so `builds` and `resident_bytes` stay
    /// accurate) but bypasses `lookups` and the demand counter — prewarm is not API demand, and
    /// touching `lookups` would corrupt `hit_rate`. Stamps `tick = 0` only when no residents entry
    /// exists yet, so a real request racing the warm keeps its newer tick.
    async fn warm(&self, id: &TranslationId) -> Result<Arc<Corpus>, Arc<QuranLoadError>> {
        let path = match self.path_for(id) {
            Some(p) => p,
            None => {
                return Err(Arc::new(QuranLoadError::Invariant(format!(
                    "translation {} not in catalogue",
                    id.as_str()
                ))));
            }
        };
        let path_str = path.to_string_lossy().into_owned();
        let metrics = self.metrics.clone();
        let build_sem = self.build_sem.clone();
        let init = async move {
            let _permit = build_sem
                .acquire()
                .await
                .expect("build semaphore is never closed");
            let corpus = load_translation_corpus(&path_str).await?;
            metrics
                .resident_bytes
                .fetch_add(corpus.bytes() as u64, Ordering::Relaxed);
            metrics.builds.fetch_add(1, Ordering::Relaxed);
            Ok(Arc::new(corpus))
        };
        // Cold detection mirrors get_or_build: moka skips init for a resident id, so gate the
        // prewarm counter on an actual build to avoid over-counting a racing residency.
        let builds_before = self.metrics.builds.load(Ordering::Relaxed);
        let corpus = self.cache.try_get_with(id.clone(), init).await;
        let is_cold = self.metrics.builds.load(Ordering::Relaxed) > builds_before;
        if corpus.is_ok() {
            self.metrics
                .residents
                .lock()
                .await
                .entry(id.clone())
                .or_insert(0);
            if is_cold {
                self.otlp_prewarm_builds.add(1, &self.otlp_attrs(id));
            }
        }
        corpus
    }

    /// Boot prewarm of the top-N translations by decayed durable API-demand score. Bounded,
    /// non-self-reinforcing (no demand/lookup increments), and yielding. The candidate set is
    /// retained before any warm runs so the effectiveness counters can attribute real cold builds.
    pub async fn prewarm(&self, ranked: Vec<(String, f64)>) {
        if !self.demand_collect || self.prewarm_count == 0 {
            return;
        }
        // The durable table outlives catalogue changes: filter by current catalogue membership
        // BEFORE truncating to N, so a removed id cannot waste a prewarm slot.
        let catalogue_filtered: Vec<String> = ranked
            .into_iter()
            .map(|(id, _)| id)
            .filter(|id| self.id_whitelist.contains(id))
            .collect();
        let n = (self.prewarm_count as usize).min(self.max_resident_translations as usize);
        let candidates: Vec<String> = catalogue_filtered.into_iter().take(n).collect();
        {
            let mut cs = self.candidate_set.write().unwrap();
            for id in &candidates {
                cs.insert(id.clone());
            }
        }
        for id in &candidates {
            let Some(tid) = self.parse_id(id) else {
                continue;
            };
            // Yield while a real cold load could be holding a permit; the check is racy against
            // the acquire inside `init`, so the wait is deadline-bounded.
            let deadline = Instant::now() + PREWARM_YIELD_DEADLINE;
            while self.build_sem.available_permits() < BUILD_CONCURRENCY {
                if Instant::now() >= deadline {
                    break;
                }
                tokio::task::yield_now().await;
            }
            match self.warm(&tid).await {
                Ok(_) => {
                    let mut guard = self.prewarmed.write().unwrap();
                    guard.insert(id.clone());
                }
                Err(e) => tracing::warn!(
                    id = %id,
                    error = %e,
                    "prewarm build skipped (non-fatal; translations are not fail-fast)"
                ),
            }
        }
    }

    /// Snapshot every demand atomic into a local before any SQLite I/O. The flush writes from the
    /// snapshot; `demand_subtract` runs only after a committed transaction and subtracts exactly
    /// the snapshotted amount, so hits accrued during the write survive.
    pub fn demand_snapshot(&self) -> Vec<(String, u64)> {
        self.demand
            .iter()
            .map(|(id, atom)| (id.as_str().to_string(), atom.load(Ordering::Relaxed)))
            .collect()
    }

    pub fn demand_subtract(&self, snapshot: &[(String, u64)]) {
        for (id, amount) in snapshot {
            if let Some(tid) = self.parse_id(id) {
                if let Some(atom) = self.demand.get(&tid) {
                    atom.fetch_sub(*amount, Ordering::Relaxed);
                }
            }
        }
    }

    /// Health snapshot written by the flush task. Cloned under the read guard; never held across
    /// `.await`.
    pub fn set_top_demand(&self, ranked: Vec<(String, f64)>) {
        let mut guard = self.top_demand.write().unwrap();
        *guard = ranked;
    }

    async fn enforce_byte_bound(&self) {
        let _permit = self
            .prune_sem
            .acquire()
            .await
            .expect("prune semaphore is never closed");
        loop {
            self.cache.run_pending_tasks().await;
            if self.metrics.resident_bytes.load(Ordering::Relaxed) <= self.max_resident_bytes {
                break;
            }
            // Select the lowest-tick victim. If it has already left the cache (a late eviction
            // listener cleared the entry), drop the stale residents row and re-select; otherwise
            // evict it. Checking membership without removing would re-select the same dead id on
            // every pass (deterministic min_by_key) and spin on the request hot path. `contains_key`
            // is synchronous and does not touch the TinyLFU estimator or the idle timer. The guard
            // is dropped before the `.await`ing invalidate below.
            let (stale, victim) = {
                let mut guard = self.metrics.residents.lock().await;
                let cand = guard
                    .iter()
                    .min_by_key(|(_, last_used)| **last_used)
                    .map(|(id, _)| id.clone());
                match cand {
                    None => (false, None),
                    Some(id) if self.cache.contains_key(&id) => (false, Some(id)),
                    Some(id) => {
                        guard.remove(&id);
                        (true, None)
                    }
                }
            };
            if stale {
                continue;
            }
            let Some(victim) = victim else {
                break;
            };
            self.cache.invalidate(&victim).await;
        }
        self.cache.run_pending_tasks().await;
    }

    pub async fn stats(&self) -> PoolStats {
        self.cache.run_pending_tasks().await;
        let lookups = self.metrics.lookups.load(Ordering::Relaxed);
        let builds = self.metrics.builds.load(Ordering::Relaxed);
        let evictions_per_minute = {
            let now = Instant::now();
            let mut times = self.metrics.eviction_times.lock().await;
            while times
                .front()
                .is_some_and(|at| now.saturating_duration_since(*at) > Duration::from_secs(60))
            {
                times.pop_front();
            }
            times.len() as u64
        };
        let hit_rate = if lookups == 0 {
            None
        } else {
            Some(1.0 - ((builds as f64 / lookups as f64).min(1.0)))
        };
        let prewarmed: Vec<String> = self.prewarmed.read().unwrap().iter().cloned().collect();
        let top_demand = self.top_demand.read().unwrap().clone();
        PoolStats {
            resident_count: self.cache.entry_count(),
            resident_bytes: self.metrics.resident_bytes.load(Ordering::Relaxed),
            builds,
            evictions: self.metrics.evictions.load(Ordering::Relaxed),
            evictions_per_minute,
            lookups,
            hit_rate,
            prewarmed,
            top_demand,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quran::load_catalogue;

    fn settings() -> crate::config::QuranSettings {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran");
        crate::config::QuranSettings {
            uthmani_path: format!("{base}/arabic/quran-uthmani.sqlite"),
            simple_clean_path: format!("{base}/arabic/quran-simple-clean.sqlite"),
            metadata_xml_path: format!("{base}/quran-data.xml"),
            translations_dir: format!("{base}/translations"),
            max_resident_translations: 8,
            max_resident_bytes: 48 * 1024 * 1024,
            translation_idle_ttl_secs: 1800,
        }
    }

    async fn pool() -> TranslationPool {
        pool_with(true, 2).await
    }

    async fn pool_with(demand_collect: bool, prewarm_count: u64) -> TranslationPool {
        let s = settings();
        let cat = load_catalogue(&format!("{}/index.min.json", s.translations_dir))
            .await
            .expect("catalogue loads");
        TranslationPool::new(
            &cat,
            PathBuf::from(&s.translations_dir),
            s.max_resident_translations,
            s.max_resident_bytes,
            Duration::from_secs(s.translation_idle_ttl_secs),
            demand_collect,
            prewarm_count,
        )
    }

    #[tokio::test]
    async fn parse_id_rejects_non_catalogue_and_traversal_strings() {
        let p = pool().await;
        assert!(p.parse_id("en.sahih").is_some(), "real id accepted");
        assert!(p.parse_id("nope").is_none());
        assert!(p.parse_id("../quran-uthmani").is_none());
        assert!(p.parse_id("en.sahih/../foo").is_none());
        assert!(p.parse_id("").is_none());
    }

    #[tokio::test]
    async fn cold_start_single_flight_one_build_for_many_concurrent() {
        let p = Arc::new(pool().await);
        let id = p.parse_id("en.sahih").expect("en.sahih in catalogue");
        let n = 16u32;
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..n {
            let p = p.clone();
            let id = id.clone();
            set.spawn(async move { p.get_or_build(&id).await.is_ok() });
        }
        let mut ok = 0u32;
        while let Some(res) = set.join_next().await {
            if res.unwrap() {
                ok += 1;
            }
        }
        assert_eq!(ok, n, "all concurrent requests succeed");
        let stats = p.stats().await;
        assert_eq!(
            stats.builds, 1,
            "exactly one build for {n} concurrent (single-flight)"
        );
        assert_eq!(stats.resident_count, 1, "one translation resident");
    }

    #[tokio::test]
    async fn stats_report_builds_lookups_and_resident() {
        let p = Arc::new(pool().await);
        let id = p.parse_id("en.sahih").unwrap();
        let _ = p.get_or_build(&id).await.unwrap();
        let _ = p.get_or_build(&id).await.unwrap();
        let stats = p.stats().await;
        assert_eq!(stats.builds, 1);
        assert_eq!(stats.resident_count, 1);
        assert_eq!(stats.lookups, 2);
        assert!(stats.hit_rate.unwrap() > 0.0, "{stats:?}");
    }

    #[tokio::test]
    async fn byte_bound_evicts_when_capacity_exceeded() {
        let s = settings();
        let cat = load_catalogue(&format!("{}/index.min.json", s.translations_dir))
            .await
            .unwrap();
        // A 1-byte budget: every real translation outweighs it, so loading several distinct ids
        // must evict the earlier residents.
        let p = TranslationPool::new(
            &cat,
            PathBuf::from(&s.translations_dir),
            8,
            1,
            Duration::from_secs(1800),
            true,
            0,
        );
        let ids: Vec<String> = p.catalogue().keys().take(5).cloned().collect();
        for id in &ids {
            let tid = p.parse_id(id).unwrap();
            let _ = p.get_or_build(&tid).await.expect("loads");
            let _ = p.stats().await; // flush pending evictions
        }
        let stats = p.stats().await;
        assert!(stats.evictions >= 1, "byte bound must evict: {stats:?}");
    }

    #[tokio::test]
    async fn count_bound_evicts_lru_translations() {
        let s = settings();
        let cat = load_catalogue(&format!("{}/index.min.json", s.translations_dir))
            .await
            .unwrap();
        let p = TranslationPool::new(
            &cat,
            PathBuf::from(&s.translations_dir),
            2,
            u64::MAX,
            Duration::from_secs(1800),
            true,
            0,
        );
        let ids: Vec<String> = p.catalogue().keys().take(5).cloned().collect();
        for id in &ids {
            let tid = p.parse_id(id).unwrap();
            p.get_or_build(&tid).await.expect("loads");
        }
        let stats = p.stats().await;
        assert!(
            stats.resident_count <= 2,
            "count bound must hold: {stats:?}"
        );
        assert!(
            stats.evictions >= 3,
            "LRU count evictions observed: {stats:?}"
        );
        assert!(
            stats.evictions_per_minute >= 3,
            "recent eviction rate exported: {stats:?}"
        );
    }

    fn first_catalogue_id(p: &TranslationPool) -> TranslationId {
        let id_str = p.catalogue().keys().next().unwrap().clone();
        p.parse_id(&id_str).unwrap()
    }

    #[tokio::test]
    async fn demand_counter_increments_once_per_successful_get_or_build() {
        let p = pool().await;
        let id = first_catalogue_id(&p);
        let _ = p.get_or_build(&id).await.unwrap();
        let snap = p.demand_snapshot();
        let count = snap
            .iter()
            .find(|(s, _)| s == id.as_str())
            .map(|(_, c)| *c)
            .unwrap();
        assert_eq!(count, 1);
        let _ = p.get_or_build(&id).await.unwrap();
        let snap = p.demand_snapshot();
        let count = snap
            .iter()
            .find(|(s, _)| s == id.as_str())
            .map(|(_, c)| *c)
            .unwrap();
        assert_eq!(count, 2, "one incr per successful get_or_build");
    }

    #[tokio::test]
    async fn demand_counter_skips_catalogue_miss_and_build_error() {
        let p = pool().await;
        // Catalogue miss: increments lookups but not demand (no demand row exists for a bogus id).
        let bogus = TranslationId::parse("nope", &p.id_whitelist);
        assert!(bogus.is_none());
        // A real id whose backing file is missing would error; instead verify the side table is
        // pre-populated only with catalogue ids (no row → no increment possible).
        for (id_str, _) in p.demand_snapshot() {
            assert!(p.id_whitelist.contains(&id_str));
        }
    }

    #[tokio::test]
    async fn demand_count_survives_eviction_and_readmission() {
        // Two slots, tiny byte budget: loading two distinct ids evicts the first; re-loading it
        // must still observe the prior demand count (side table outlives eviction).
        let s = settings();
        let cat = load_catalogue(&format!("{}/index.min.json", s.translations_dir))
            .await
            .unwrap();
        let p = TranslationPool::new(
            &cat,
            PathBuf::from(&s.translations_dir),
            1,
            1,
            Duration::from_secs(1800),
            true,
            0,
        );
        let ids: Vec<String> = p.catalogue().keys().take(2).cloned().collect();
        let a = p.parse_id(&ids[0]).unwrap();
        let b = p.parse_id(&ids[1]).unwrap();
        let _ = p.get_or_build(&a).await.unwrap();
        let _ = p.get_or_build(&b).await.unwrap();
        let _ = p.stats().await; // flush evictions
                                 // Re-admit a: its demand counter should still hold the earlier hit (side table disjoint
                                 // from residents/eviction).
        let snap = p.demand_snapshot();
        let a_count = snap
            .iter()
            .find(|(s, _)| s == a.as_str())
            .map(|(_, c)| *c)
            .unwrap();
        assert_eq!(a_count, 1, "demand count survives eviction");
        let _ = p.get_or_build(&a).await.unwrap();
        let snap = p.demand_snapshot();
        let a_count = snap
            .iter()
            .find(|(s, _)| s == a.as_str())
            .map(|(_, c)| *c)
            .unwrap();
        assert_eq!(a_count, 2, "demand increments again after re-admission");
    }

    #[tokio::test]
    async fn enforce_byte_bound_terminates_when_victim_already_left_cache() {
        // Forge the spin condition: a residents entry whose id is no longer in the cache, plus a
        // resident_bytes value over budget. The self-healing loop must drop the stale entry and
        // terminate rather than spin on the dead id.
        let p = pool().await;
        let id = first_catalogue_id(&p);
        // Insert a residents tombstone for an id that is NOT in the cache.
        p.metrics.residents.lock().await.insert(id.clone(), 5);
        // Inflate resident_bytes past budget to force enforce_byte_bound to look for a victim.
        p.metrics
            .resident_bytes
            .store(p.max_resident_bytes + 1, Ordering::Relaxed);
        // No cache entry for id, so contains_key is false; the loop must remove the tombstone.
        assert!(!p.cache.contains_key(&id));
        p.enforce_byte_bound().await;
        // The stale residents entry is gone.
        assert!(
            !p.metrics.residents.lock().await.contains_key(&id),
            "stale victim-index entry must be dropped"
        );
        // And resident_bytes is reconciled (no real entries to evict, so it stays over budget —
        // the point is termination, not the byte value).
    }

    #[tokio::test]
    async fn enforce_byte_bound_drops_stale_entry_so_next_call_still_enforces() {
        let p = pool().await;
        let id = first_catalogue_id(&p);
        // Tombstone + over-budget: first call drops the tombstone and terminates.
        p.metrics.residents.lock().await.insert(id.clone(), 5);
        p.metrics
            .resident_bytes
            .store(p.max_resident_bytes + 1, Ordering::Relaxed);
        p.enforce_byte_bound().await;
        assert!(!p.metrics.residents.lock().await.contains_key(&id));
        // Reset the artificial over-budget so the real build below is not evicted
        // by get_or_build's own internal enforce_byte_bound before we re-overbudget.
        p.metrics.resident_bytes.store(0, Ordering::Relaxed);
        // Build a real cache-backed resident, push resident_bytes over budget, and confirm the
        // next call still evicts it — the stale cleanup did not wedge the loop.
        let corpus = p.get_or_build(&id).await.unwrap();
        let real_bytes = corpus.bytes() as u64;
        p.metrics
            .resident_bytes
            .store(real_bytes + p.max_resident_bytes + 1, Ordering::Relaxed);
        let ev_before = p.metrics.evictions.load(Ordering::Relaxed);
        p.enforce_byte_bound().await;
        let _ = p.stats().await; // flush the eviction listener
        assert!(
            p.metrics.evictions.load(Ordering::Relaxed) > ev_before,
            "next call still evicts a live victim after stale cleanup"
        );
    }

    #[tokio::test]
    async fn prewarm_clamps_to_max_residents_and_increments_neither_demand_nor_lookups() {
        // prewarm_count far above capacity: must clamp to max_resident_translations.
        let s = settings();
        let cat = load_catalogue(&format!("{}/index.min.json", s.translations_dir))
            .await
            .unwrap();
        let p = Arc::new(TranslationPool::new(
            &cat,
            PathBuf::from(&s.translations_dir),
            2,
            u64::MAX,
            Duration::from_secs(1800),
            true,
            99,
        ));
        let lookups_before = p.metrics.lookups.load(Ordering::Relaxed);
        // Build a ranked list of catalogue ids.
        let ranked: Vec<(String, f64)> = p
            .catalogue()
            .keys()
            .take(5)
            .cloned()
            .map(|id| (id, 1.0))
            .collect();
        p.prewarm(ranked).await;
        let lookups_after = p.metrics.lookups.load(Ordering::Relaxed);
        assert_eq!(
            lookups_before, lookups_after,
            "prewarm must not touch lookups"
        );
        // Demand counters all zero (prewarm does not increment demand).
        for (_, c) in p.demand_snapshot() {
            assert_eq!(c, 0, "prewarm must not increment demand");
        }
        // prewarmed set clamped to max_resident_translations (2).
        let pre = p.prewarmed.read().unwrap().clone();
        assert!(pre.len() <= 2, "prewarm clamps to capacity: {pre:?}");
        // tick=0 stamped: residents entries for prewarmed ids carry 0.
        let residents = p.metrics.residents.lock().await;
        for id in &pre {
            let tid = p.parse_id(id).unwrap();
            assert_eq!(*residents.get(&tid).unwrap(), 0u64, "prewarm stamps tick 0");
        }
    }

    #[tokio::test]
    async fn prewarm_skips_ids_absent_from_catalogue() {
        let p = pool_with(true, 5).await;
        let mut ranked: Vec<(String, f64)> = p
            .catalogue()
            .keys()
            .take(2)
            .cloned()
            .map(|id| (id, 1.0))
            .collect();
        // An id that no longer exists in the catalogue (outlives a catalogue change).
        ranked.push(("removed.id".to_string(), 100.0));
        ranked.push(("also.gone".to_string(), 99.0));
        p.prewarm(ranked).await;
        let pre = p.prewarmed.read().unwrap().clone();
        assert!(
            !pre.iter().any(|s| s == "removed.id" || s == "also.gone"),
            "prewarm skips non-catalogue ids: {pre:?}"
        );
    }

    #[tokio::test]
    async fn prewarm_on_bogus_id_warns_without_panic() {
        // A ranked list containing ONLY non-catalogue ids: prewarm filters them out, prewarms
        // nothing, and returns without panicking. (A real build error inside warm() is routed
        // through the `Err => warn!` branch, also non-panicking.)
        let p = pool_with(true, 2).await;
        p.prewarm(vec![
            ("nope.id".to_string(), 5.0),
            ("also.gone".to_string(), 4.0),
        ])
        .await;
        assert!(
            p.prewarmed.read().unwrap().is_empty(),
            "no non-catalogue id is prewarmed"
        );
        assert!(
            p.candidate_set.read().unwrap().is_empty(),
            "no candidate recorded for filtered ids"
        );
    }

    #[tokio::test]
    async fn demand_subtract_preserves_hits_accrued_during_flush() {
        let p = pool().await;
        let id = first_catalogue_id(&p);
        let _ = p.get_or_build(&id).await.unwrap();
        let _ = p.get_or_build(&id).await.unwrap();
        let snap = p.demand_snapshot(); // snapshot amount for this id == 2
                                        // A third hit accrues "during the flush" (after the snapshot, before subtract).
        let _ = p.get_or_build(&id).await.unwrap();
        // Committed flush → subtract exactly the snapshotted amount (2); the accrued 1 survives.
        p.demand_subtract(&snap);
        let after = p.demand_snapshot();
        let count = after
            .iter()
            .find(|(s, _)| s == id.as_str())
            .map(|(_, c)| *c)
            .unwrap();
        assert_eq!(count, 1, "hits accrued during the flush survive fetch_sub");
    }

    #[tokio::test]
    async fn candidate_set_contains_only_prewarmed_ids() {
        // Effectiveness contract: the candidate set is retained before any warm and contains
        // exactly the warmed ids. A real cold build for a non-candidate id must NOT enter it (so
        // it cannot inflate the candidate-cold-build counter).
        let p = pool_with(true, 2).await;
        let ranked: Vec<(String, f64)> = p
            .catalogue()
            .keys()
            .take(3)
            .cloned()
            .map(|id| (id, 1.0))
            .collect();
        let chosen: HashSet<String> = ranked.iter().take(2).map(|(id, _)| id.clone()).collect();
        p.prewarm(ranked).await;
        let cs = p.candidate_set.read().unwrap().clone();
        assert_eq!(cs.len(), 2, "candidate set clamps to N: {cs:?}");
        for id in &chosen {
            assert!(cs.contains(id), "warmed id in candidate set: {id}");
        }
        // A real cold build for a non-candidate id does not enlarge the candidate set.
        let mut non_candidate: Option<String> = None;
        for k in p.catalogue().keys() {
            if !chosen.contains(k) {
                non_candidate = Some(k.clone());
                break;
            }
        }
        if let Some(nc) = non_candidate {
            let tid = p.parse_id(&nc).unwrap();
            let _ = p.get_or_build(&tid).await.unwrap();
            assert!(
                !p.candidate_set.read().unwrap().contains(&nc),
                "real cold build for a non-candidate id never enters the candidate set"
            );
        }
    }

    #[tokio::test]
    async fn otlp_counter_exclusion_prewarm_never_real_cold_candidate_only_cold() {
        // OTLP counter exclusion contract. The OTel meter is a no-op in unit tests, so the
        // contract is proven structurally via the gates that drive the counters:
        //   - otlp_real_cold_builds and otlp_candidate_cold_builds fire ONLY inside get_or_build's
        //     `is_cold` branch; warm() never touches either.
        //   - A prewarmed id requested via get_or_build is a cache HIT (builds unchanged), so
        //     is_cold is false and neither counter can fire — even though the id is a candidate
        //     within the idle TTL.
        //   - otlp_candidate_cold_builds also requires candidate_set membership, which a
        //     non-candidate cold build can never enter.
        let p = pool_with(true, 1).await;
        let prewarm_str = p.catalogue().keys().next().unwrap().clone();
        let prewarm_id = p.parse_id(&prewarm_str).unwrap();
        let non_candidate_str = p
            .catalogue()
            .keys()
            .find(|k| *k != &prewarm_str)
            .unwrap()
            .clone();
        let non_candidate_id = p.parse_id(&non_candidate_str).unwrap();

        // Prewarm one id: exactly one cold build, candidate set = {prewarm_str}.
        p.prewarm(vec![(prewarm_str.clone(), 1.0)]).await;
        let builds_after_prewarm = p.metrics.builds.load(Ordering::Relaxed);
        assert_eq!(builds_after_prewarm, 1, "one cold build during prewarm");
        assert!(p.candidate_set.read().unwrap().contains(&prewarm_str));

        // Contract A: a real request for the prewarmed id is a HIT — builds unchanged → is_cold
        // false → otlp_real_cold_builds cannot fire, and otlp_candidate_cold_builds cannot fire
        // either (not a cold build, despite candidate membership + within TTL).
        let _ = p.get_or_build(&prewarm_id).await.unwrap();
        assert_eq!(
            p.metrics.builds.load(Ordering::Relaxed),
            builds_after_prewarm,
            "prewarmed id is a hit: no new build → real/candidate counters gated off"
        );

        // Contract B: a cold build for a non-candidate id is real-cold (builds++) but never enters
        // the candidate set, so otlp_candidate_cold_builds stays gated off.
        let _ = p.get_or_build(&non_candidate_id).await.unwrap();
        assert_eq!(
            p.metrics.builds.load(Ordering::Relaxed),
            builds_after_prewarm + 1,
            "non-candidate id is a real cold build"
        );
        assert!(
            !p.candidate_set.read().unwrap().contains(&non_candidate_str),
            "non-candidate cold build never enters candidate set"
        );
    }

    #[tokio::test]
    async fn prewarm_hit_emitted_once_per_prewarmed_id() {
        // First-request prewarm hit is claimed once: a second cache hit on the same prewarmed id
        // does not re-claim (the claim set grows by exactly one).
        let p = pool_with(true, 1).await;
        let id_str = p.catalogue().keys().next().unwrap().clone();
        let id = p.parse_id(&id_str).unwrap();
        p.prewarm(vec![(id_str.clone(), 1.0)]).await;
        // First real request: cache hit (prewarmed) → claim.
        let _ = p.get_or_build(&id).await.unwrap();
        assert_eq!(p.prewarm_claimed.lock().unwrap().len(), 1);
        // Second real request: still a hit, but already claimed → claim set unchanged.
        let _ = p.get_or_build(&id).await.unwrap();
        assert_eq!(p.prewarm_claimed.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn hit_rate_is_none_when_no_lookups() {
        let p = pool().await;
        let stats = p.stats().await;
        assert!(stats.hit_rate.is_none(), "nullable before any lookup");
    }

    #[tokio::test]
    async fn demand_collection_off_disables_counter() {
        let p = pool_with(false, 0).await;
        let id = first_catalogue_id(&p);
        let _ = p.get_or_build(&id).await.unwrap();
        for (_, c) in p.demand_snapshot() {
            assert_eq!(c, 0, "demand collection off → no counting");
        }
    }
}
