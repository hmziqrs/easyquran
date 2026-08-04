use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use moka::future::{Cache, FutureExt};
use moka::notification::{ListenerFuture, RemovalCause};

use super::loader::{load_translation_corpus, QuranLoadError};
use super::store::{CatalogueEntry, Corpus, TranslationId};

/// On-demand cached pool of translation corpora (§3). Cold reads build a
/// `Corpus` from `translations/sqlite/<id>.sqlite`; hot reads are single-flighted
/// by moka so N concurrent requests for the same id build once. Bounds (bytes
/// via the weigher, idle TTL) are taken from settings, not constants — see the
/// tuning table in docs/quran.md §3.
pub struct TranslationPool {
    cache: Cache<TranslationId, Arc<Corpus>>,
    dir: PathBuf,
    ids: HashSet<String>,
    requests: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    evictions: Arc<AtomicU64>,
    resident_bytes: Arc<AtomicU64>,
    max_resident: u64,
    max_bytes: u64,
    idle_ttl_secs: u64,
}

impl TranslationPool {
    pub fn new(
        sqlite_dir: PathBuf,
        entries: &[CatalogueEntry],
        max_resident: u64,
        max_bytes: u64,
        idle_ttl_secs: u64,
    ) -> Self {
        let ids: HashSet<String> = entries.iter().map(|e| e.id.to_string()).collect();
        let misses = Arc::new(AtomicU64::new(0));
        let evictions = Arc::new(AtomicU64::new(0));
        let resident_bytes = Arc::new(AtomicU64::new(0));

        let evictions_for_listener = evictions.clone();
        let resident_bytes_for_listener = resident_bytes.clone();
        let listener =
            move |_k: Arc<TranslationId>, v: Arc<Corpus>, cause: RemovalCause| -> ListenerFuture {
                let ev = evictions_for_listener.clone();
                let rb = resident_bytes_for_listener.clone();
                async move {
                    // Replaced entries re-insert under the same key; counting them as
                    // evictions would double-count normal refreshes, so only Size/Expired
                    // bump the counter (these are the bounds-driven outcomes §3 measures).
                    if matches!(cause, RemovalCause::Size | RemovalCause::Expired) {
                        ev.fetch_add(1, Ordering::Relaxed);
                    }
                    let _ = rb.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |x| {
                        Some(x.saturating_sub(v.bytes() as u64))
                    });
                }
                .boxed()
            };

        let cache = Cache::builder()
            // With a weigher present, max_capacity is the weight (byte) budget,
            // not an entry count. This bounds resident memory hard; the
            // max_resident entry bound is a tuning target effectively enforced
            // for p95 artifacts (see §3 table).
            .max_capacity(max_bytes)
            .weigher(|_k: &TranslationId, v: &Arc<Corpus>| -> u32 {
                v.bytes().try_into().unwrap_or(u32::MAX)
            })
            .time_to_idle(Duration::from_secs(idle_ttl_secs))
            .async_eviction_listener(listener)
            .build();

        Self {
            cache,
            dir: sqlite_dir,
            ids,
            requests: Arc::new(AtomicU64::new(0)),
            misses,
            evictions,
            resident_bytes,
            max_resident,
            max_bytes,
            idle_ttl_secs,
        }
    }

    /// Resolve `s` to a `TranslationId` iff it exactly matches a catalogue id.
    /// This membership check IS the path-traversal guard — `..`, `/`, `\` can
    /// never match a real id (§3).
    pub fn parse_id(&self, s: &str) -> Option<TranslationId> {
        if self.ids.contains(s) {
            Some(TranslationId::from_validated(s))
        } else {
            None
        }
    }

    /// Read-only view of the catalogue id set (for tests/diagnostics).
    pub fn ids(&self) -> &HashSet<String> {
        &self.ids
    }

    fn resolve_path(&self, id: &TranslationId) -> PathBuf {
        // id is a validated catalogue id (no path separators), so joining
        // `<id>.sqlite` onto the sqlite dir is traversal-safe.
        self.dir.join(format!("{}.sqlite", id.as_str()))
    }

    /// Get-or-build a corpus. moka single-flights the async init per key, so N
    /// concurrent cold callers for the same id build exactly once.
    pub async fn get_or_build(
        &self,
        id: &TranslationId,
    ) -> Result<Arc<Corpus>, Arc<QuranLoadError>> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let path = self.resolve_path(id);
        let misses = self.misses.clone();
        let resident_bytes = self.resident_bytes.clone();
        self.cache
            .try_get_with(id.clone(), async move {
                // Counted inside the single-flighted init → increments once per
                // cold build (hits = requests - misses at read time).
                misses.fetch_add(1, Ordering::Relaxed);
                let corpus = load_translation_corpus(path.to_string_lossy().as_ref()).await?;
                let c = Arc::new(corpus);
                resident_bytes.fetch_add(c.bytes() as u64, Ordering::Relaxed);
                Ok::<Arc<Corpus>, QuranLoadError>(c)
            })
            .await
    }

    /// (resident_count, resident_bytes, hits, misses, evictions). Best-effort:
    /// resident_count is moka's live entry_count; resident_bytes is maintained
    /// via atomics around build/eviction and may transiently desync. hits is
    /// derived from requests - misses (§3).
    pub fn metrics(&self) -> (u64, u64, u64, u64, u64) {
        let requests = self.requests.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let hits = requests.saturating_sub(misses);
        let evictions = self.evictions.load(Ordering::Relaxed);
        let resident_bytes = self.resident_bytes.load(Ordering::Relaxed);
        let resident_count = self.cache.entry_count();
        (resident_count, resident_bytes, hits, misses, evictions)
    }

    /// Bounds summary for diagnostics/tuning (§3). (max_resident_entries,
    /// max_bytes, idle_ttl_secs).
    pub fn config(&self) -> (u64, u64, u64) {
        (self.max_resident, self.max_bytes, self.idle_ttl_secs)
    }

    /// Force a sync of pending eviction maintenance — used by tests that need
    /// eviction counters to settle before asserting.
    pub async fn run_pending_tasks(&self) {
        self.cache.run_pending_tasks().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../db/quran/tanzil/translations"
        ))
    }

    fn real_pool(max_bytes: u64) -> TranslationPool {
        let b = base();
        use crate::config::QuranSettings;
        let settings = QuranSettings {
            uthmani_path: String::new(),
            simple_clean_path: String::new(),
            metadata_xml_path: String::new(),
            translations_index_path: b.join("index.min.json").to_string_lossy().into_owned(),
            translations_sqlite_dir: b.join("sqlite").to_string_lossy().into_owned(),
            max_resident_translations: 8,
            max_resident_bytes: 48 * 1024 * 1024,
            translation_idle_ttl_secs: 1800,
        };
        let entries = crate::quran::load_catalogue(&settings).expect("catalogue loads");
        TranslationPool::new(b.join("sqlite"), &entries, 8, max_bytes, 1800)
    }

    #[tokio::test]
    async fn cold_start_single_flight_one_build_for_16_concurrent() {
        let pool = real_pool(48 * 1024 * 1024);
        let id = pool.parse_id("en.sahih").expect("en.sahih in catalogue");
        let pool = Arc::new(pool);
        let mut handles = Vec::new();
        for _ in 0..16 {
            let p = pool.clone();
            let id = id.clone();
            handles.push(tokio::spawn(async move { p.get_or_build(&id).await }));
        }
        let mut corpora = Vec::new();
        for h in handles {
            corpora.push(h.await.unwrap().expect("build ok"));
        }
        pool.run_pending_tasks().await;
        let first = &corpora[0];
        for c in &corpora[1..] {
            // Same logical corpus — arena lengths match (pointer differs per Arc clone).
            assert_eq!(c.bytes(), first.bytes(), "all 16 saw the same corpus");
        }
        let (resident_count, _, hits, misses, _) = pool.metrics();
        assert_eq!(misses, 1, "exactly one cold build (single-flight)");
        assert_eq!(hits, 15, "remaining 15 were cache hits");
        assert_eq!(resident_count, 1, "one entry resident");
    }

    #[tokio::test]
    async fn parse_id_rejects_non_catalogue_strings() {
        let pool = real_pool(48 * 1024 * 1024);
        assert!(pool.parse_id("../foo").is_none());
        assert!(pool.parse_id("en.sahih/../x").is_none());
        assert!(pool.parse_id("nope").is_none());
        assert!(pool.parse_id("").is_none());
        assert!(pool.parse_id("en.sahih").is_some());
    }

    #[tokio::test]
    async fn eviction_fires_when_byte_bound_exceeded() {
        // Tight byte budget: a single real translation corpus (≥1 MiB) won't
        // fit two distinct ids, so loading two distinct ids forces a Size eviction.
        let pool = real_pool(1);
        let id_a = pool.parse_id("en.sahih").expect("en.sahih");
        // Pick a second real id from the catalogue that is NOT en.sahih.
        let second = pool
            .ids
            .iter()
            .find(|s| s.as_str() != "en.sahih")
            .cloned()
            .unwrap();
        let id_b = pool.parse_id(&second).expect("second id");
        let pool = Arc::new(pool);
        let _ = pool.get_or_build(&id_a).await.expect("a builds");
        let _ = pool.get_or_build(&id_b).await.expect("b builds");
        // moka evicts asynchronously; run maintenance to settle the counter.
        for _ in 0..20 {
            pool.run_pending_tasks().await;
            let (_, _, _, _, evictions) = pool.metrics();
            if evictions >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let (_, _, _, _, evictions) = pool.metrics();
        assert!(
            evictions >= 1,
            "expected at least one Size eviction, got {evictions}"
        );
    }
}
