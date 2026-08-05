use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use moka::notification::RemovalCause;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use crate::quran::loader::{load_translation_corpus, QuranLoadError};
use crate::quran::store::{CatalogueEntry, Corpus, TranslationId};

/// Bounds concurrent cold Corpus builds so the byte ceiling holds during the build phase: moka's
/// weigher caps resident entries, not in-flight builds, so unbounded parallel distinct-id loads
/// could transiently allocate far past `max_resident_bytes`. Near-serial (2) closes that gap.
const BUILD_CONCURRENCY: usize = 2;

/// Length-prefix a byte field into the digest so streamed fields cannot collide across
/// variable-length boundaries (a true function of the bytes, not their concatenation).
fn digest_field(hasher: &mut Sha256, b: &[u8]) {
    hasher.update((b.len() as u64).to_le_bytes());
    hasher.update(b);
}

#[derive(Default)]
struct PoolMetrics {
    builds: AtomicU64,
    evictions: AtomicU64,
    resident_bytes: AtomicU64,
    lookups: AtomicU64,
}

/// Observable pool state for tuning the §3 bounds on evidence.
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub resident_count: u64,
    pub resident_bytes: u64,
    pub builds: u64,
    pub evictions: u64,
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
    catalogue_digest: Box<str>,
}

impl TranslationPool {
    /// Bounds come from settings (not constants) so they tune without recompiling the rules.
    /// moka enforces a single capacity dimension: with a weigher set, `max_resident_bytes` is
    /// the hard RAM ceiling and `max_resident_translations` is an advisory count target
    /// (observable via [`PoolStats::resident_count`]); one moka cache cannot bind both at once.
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
        let mut sorted: Vec<&CatalogueEntry> = catalogue.iter().collect();
        sorted.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        let mut hasher = Sha256::new();
        for e in &sorted {
            digest_field(&mut hasher, e.id.as_bytes());
            digest_field(&mut hasher, e.language.as_bytes());
            digest_field(&mut hasher, e.language_code.as_bytes());
            digest_field(&mut hasher, e.direction.as_bytes());
            digest_field(&mut hasher, e.name.as_bytes());
            match e.translator.as_deref() {
                Some(t) => digest_field(&mut hasher, t.as_bytes()),
                None => digest_field(&mut hasher, &[]), // length-0 marks absence distinctly
            }
            digest_field(&mut hasher, e.path.as_bytes());
            digest_field(&mut hasher, &e.size_bytes.to_le_bytes());
        }
        let catalogue_digest: Box<str> = hex::encode(&hasher.finalize()[..8]).into_boxed_str();
        let build_sem = Arc::new(Semaphore::new(BUILD_CONCURRENCY));
        let metrics = Arc::new(PoolMetrics::default());
        let metrics_for_listener = metrics.clone();

        let cache = Cache::builder()
            .max_capacity(max_resident_bytes)
            .weigher(|_k: &TranslationId, v: &Arc<Corpus>| -> u32 {
                u32::try_from(v.bytes()).unwrap_or(u32::MAX)
            })
            .time_to_idle(idle_ttl)
            .async_eviction_listener(move |_key, v: Arc<Corpus>, _cause: RemovalCause| {
                let m = metrics_for_listener.clone();
                Box::pin(async move {
                    m.evictions.fetch_add(1, Ordering::Relaxed);
                    m.resident_bytes
                        .fetch_sub(v.bytes() as u64, Ordering::Relaxed);
                })
            })
            .build();

        let _ = max_resident_translations;

        Self {
            cache,
            dir: translations_dir,
            entries,
            id_whitelist,
            metrics,
            build_sem,
            catalogue_digest,
        }
    }

    pub fn catalogue(&self) -> &HashMap<String, CatalogueEntry> {
        &self.entries
    }

    pub fn catalogue_digest(&self) -> &str {
        &self.catalogue_digest
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
            // Gate the build so the byte ceiling holds while a corpus is materialized (moka's
            // weigher binds residents, not in-flight builds). Cached hits never reach here, so the
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
        self.cache.try_get_with(id.clone(), init).await
    }

    pub async fn stats(&self) -> PoolStats {
        self.cache.run_pending_tasks().await;
        let lookups = self.metrics.lookups.load(Ordering::Relaxed);
        let builds = self.metrics.builds.load(Ordering::Relaxed);
        PoolStats {
            resident_count: self.cache.entry_count(),
            resident_bytes: self.metrics.resident_bytes.load(Ordering::Relaxed),
            builds,
            evictions: self.metrics.evictions.load(Ordering::Relaxed),
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
}
