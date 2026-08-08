use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use moka::future::Cache;
use moka::notification::RemovalCause;
use tokio::sync::{Mutex, Semaphore};

use crate::quran::loader::{load_translation_corpus, QuranLoadError};
use crate::quran::store::{CatalogueEntry, Corpus, TranslationId};

/// Bounds concurrent cold Corpus builds so the byte ceiling holds during the build phase: moka
/// enforces only the count ceiling (`max_capacity`) plus `time_to_idle` — there is no byte
/// weigher — and the separate `enforce_byte_bound` prune loop runs only after a build completes,
/// so unbounded parallel distinct-id loads could transiently allocate far past
/// `max_resident_bytes`. Near-serial (2) closes that gap.
const BUILD_CONCURRENCY: usize = 2;

#[derive(Default)]
struct PoolMetrics {
    builds: AtomicU64,
    evictions: AtomicU64,
    resident_bytes: AtomicU64,
    lookups: AtomicU64,
    access_clock: AtomicU64,
    residents: Mutex<HashMap<TranslationId, u64>>,
    eviction_times: Mutex<VecDeque<Instant>>,
}

/// Observable pool state for tuning the §3 bounds on evidence.
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub resident_count: u64,
    pub resident_bytes: u64,
    pub builds: u64,
    pub evictions: u64,
    pub evictions_per_minute: u64,
    pub lookups: u64,
    pub hit_rate: f64,
}

pub struct TranslationPool {
    cache: Cache<TranslationId, Arc<Corpus>>,
    dir: PathBuf,
    entries: HashMap<String, CatalogueEntry>,
    id_whitelist: HashSet<String>,
    metrics: Arc<PoolMetrics>,
    build_sem: Arc<Semaphore>,
    prune_sem: Arc<Semaphore>,
    max_resident_bytes: u64,
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

        Self {
            cache,
            dir: translations_dir,
            entries,
            id_whitelist,
            metrics,
            build_sem,
            prune_sem,
            max_resident_bytes,
        }
    }

    pub fn catalogue(&self) -> &HashMap<String, CatalogueEntry> {
        &self.entries
    }

    /// Whitelist parse — the membership check IS the path-traversal guard.
    pub fn parse_id(&self, s: &str) -> Option<TranslationId> {
        TranslationId::parse(s, &self.id_whitelist)
    }

    fn path_for(&self, id: &TranslationId) -> Option<PathBuf> {
        // entry.path is catalogue-relative ("sqlite/<id>.sqlite"); join against the dir root.
        Some(self.dir.join(self.entries.get(id.as_str())?.path.as_ref()))
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
        let corpus = self.cache.try_get_with(id.clone(), init).await?;
        let tick = self.metrics.access_clock.fetch_add(1, Ordering::Relaxed) + 1;
        self.metrics.residents.lock().await.insert(id.clone(), tick);
        self.enforce_byte_bound().await;
        Ok(corpus)
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
            let victim = self
                .metrics
                .residents
                .lock()
                .await
                .iter()
                .min_by_key(|(_id, last_used)| **last_used)
                .map(|(id, _last_used)| id.clone());
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
        PoolStats {
            resident_count: self.cache.entry_count(),
            resident_bytes: self.metrics.resident_bytes.load(Ordering::Relaxed),
            builds,
            evictions: self.metrics.evictions.load(Ordering::Relaxed),
            evictions_per_minute,
            lookups,
            hit_rate: if lookups == 0 {
                1.0
            } else {
                1.0 - ((builds as f64 / lookups as f64).min(1.0))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quran::load_catalogue;

    fn settings() -> crate::config::QuranSettings {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran/tanzil");
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
        assert!(stats.hit_rate > 0.0, "{stats:?}");
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
}
