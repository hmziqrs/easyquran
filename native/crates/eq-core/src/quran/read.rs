//! Reading a reader scope for (scope, Arabic source, translation sources)
//! (mirror of `source-plan.ts`, `script-availability.ts` and the worker's
//! local-first, API-fallback ladder). [`OpenSources`] keeps opened
//! databases (it is `Send + Sync`, so the app can hand an `Arc` of it to
//! its blocking pool); local reads are blocking, online reads async.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{
    db::{ArabicDb, DbError, TranslationDb},
    meta::{QuranMeta, RangeKind},
    online::{ApiAvailability, OnlineError, QuranApi},
    store::ArtifactStore,
    types::{ArabicSource, RangeText, SourceRef},
    view::{RangeGroup, group_range},
};
use crate::{net::Transport, routes::ReaderScope};

/// Build environment of the Quran data (`QuranDataEnvironment`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataEnvironment {
    Local,
    #[default]
    Production,
}

/// Production-gated scripts (`PRODUCTION_DISABLED_SOURCES`).
pub const PRODUCTION_DISABLED_SOURCES: [ArabicSource; 2] =
    [ArabicSource::IndoPak, ArabicSource::Tajweed];

/// `isScriptEnabled`.
pub fn is_script_enabled(source: ArabicSource, env: DataEnvironment) -> bool {
    env != DataEnvironment::Production || !PRODUCTION_DISABLED_SOURCES.contains(&source)
}

/// `enabledScriptIds`.
pub fn enabled_scripts(env: DataEnvironment) -> Vec<ArabicSource> {
    ArabicSource::ALL
        .into_iter()
        .filter(|s| is_script_enabled(*s, env))
        .collect()
}

/// Which source reads and which searches (`QuranSourcePlan`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePlan {
    pub reader: ArabicSource,
    pub search_match: ArabicSource,
    pub search_display: ArabicSource,
}

/// `DEFAULT_QURAN_SOURCE_PLAN`: annotated Uthmani reads, plain Tanzil
/// Uthmani searches.
pub const DEFAULT_SOURCE_PLAN: SourcePlan = SourcePlan {
    reader: ArabicSource::AnnotatedUthmani,
    search_match: ArabicSource::Uthmani,
    search_display: ArabicSource::Uthmani,
};

impl SourcePlan {
    /// `plannedSourceIds`, deduplicated in order.
    pub fn planned(&self) -> Vec<ArabicSource> {
        let mut out = Vec::new();
        for s in [self.reader, self.search_match, self.search_display] {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out
    }
}

/// The global index range `(from, to)` a reader scope covers; `None` for an
/// out-of-range number.
pub fn scope_range(meta: &QuranMeta, scope: &ReaderScope) -> Option<(u32, u32)> {
    match *scope {
        ReaderScope::Surah(n) => meta.surah(n).map(|s| (s.start_global, s.end_global())),
        ReaderScope::Page(n) => meta
            .range(RangeKind::Page, n)
            .map(|r| (r.start_global, r.end_global)),
        ReaderScope::Juz(n) => meta
            .range(RangeKind::Juz, u16::from(n))
            .map(|r| (r.start_global, r.end_global)),
        ReaderScope::Hizb(n) => meta.hizb_range(n).map(|r| (r.start_global, r.end_global)),
    }
}

/// Most translation databases kept open (`TRANSLATION_DB_CAP`-style LRU).
pub const TRANSLATION_DB_CAP: usize = 8;

/// Opened databases, shared across threads.
#[derive(Debug, Default)]
pub struct OpenSources {
    arabic: Mutex<HashMap<ArabicSource, Arc<Mutex<ArabicDb>>>>,
    translations: Mutex<Vec<(String, Arc<Mutex<TranslationDb>>)>>,
}

/// Where a read was served from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServedBy {
    Local,
    Api,
}

/// A failed read of one source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("not on device and no online fallback for {0}")]
    NotAvailable(String),
    #[error("{0}")]
    Db(#[from] DbError),
    #[error("{0}")]
    Online(#[from] OnlineError),
    #[error("out of range")]
    OutOfRange,
}

impl OpenSources {
    /// Empty.
    pub fn new() -> Self {
        Self::default()
    }

    fn arabic(
        &self,
        store: &ArtifactStore,
        source: ArabicSource,
        meta: &QuranMeta,
    ) -> Result<Option<Arc<Mutex<ArabicDb>>>, DbError> {
        let mut map = self.arabic.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(db) = map.get(&source) {
            return Ok(Some(db.clone()));
        }
        let Some(path) = store.path_for(source.id()) else {
            return Ok(None);
        };
        let db = Arc::new(Mutex::new(ArabicDb::open(source, &path, meta)?));
        map.insert(source, db.clone());
        Ok(Some(db))
    }

    fn translation(
        &self,
        store: &ArtifactStore,
        id: &str,
    ) -> Result<Option<Arc<Mutex<TranslationDb>>>, DbError> {
        let mut list = self.translations.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = list.iter().position(|(k, _)| k == id) {
            let hit = list.remove(i);
            let db = hit.1.clone();
            list.push(hit);
            return Ok(Some(db));
        }
        let Some(path) = store.path_for(id) else {
            return Ok(None);
        };
        let db = Arc::new(Mutex::new(TranslationDb::open(id, &path)?));
        while list.len() >= TRANSLATION_DB_CAP {
            list.remove(0);
        }
        list.push((id.to_owned(), db.clone()));
        Ok(Some(db))
    }

    /// Drops an opened database (before deleting its file).
    pub fn forget(&self, id: &str) {
        if let Some(source) = ArabicSource::from_id(id) {
            self.arabic
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&source);
        }
        self.translations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(k, _)| k != id);
    }

    /// Reads `from..=to` of a source from the device; `Ok(None)` when the
    /// artifact is not installed. Blocking.
    pub fn read_local(
        &self,
        store: &ArtifactStore,
        meta: &QuranMeta,
        source_id: &str,
        from: u32,
        to: u32,
    ) -> Result<Option<RangeText>, DbError> {
        match SourceRef::from_id(source_id) {
            SourceRef::Arabic(source) => match self.arabic(store, source, meta)? {
                Some(db) => db
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .read_range(from, to)
                    .map(Some),
                None => Ok(None),
            },
            SourceRef::Translation(id) => match self.translation(store, &id)? {
                Some(db) => db
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .read_range(from, to)
                    .map(Some),
                None => Ok(None),
            },
        }
    }
}

/// One source's read result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRead {
    pub source_id: String,
    pub result: Result<(RangeText, ServedBy), ReadError>,
}

/// What a reader scope shows: the Arabic groups plus each translation's
/// verses (same order as requested).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeRead {
    pub from: u32,
    pub to: u32,
    pub arabic: Result<(Vec<RangeGroup>, ServedBy), ReadError>,
    pub translations: Vec<SourceRead>,
}

/// Local-first, API-fallback read of one source (the worker ladder). The
/// local step is blocking; UI callers run [`OpenSources::read_local`] off
/// the UI thread and call [`QuranApi::read_range_guarded`] themselves.
#[allow(clippy::too_many_arguments)]
pub async fn read_source(
    sources: &OpenSources,
    store: &ArtifactStore,
    meta: &QuranMeta,
    api: Option<(
        &QuranApi,
        &dyn Transport,
        &std::cell::RefCell<ApiAvailability>,
        i64,
    )>,
    source_id: &str,
    from: u32,
    to: u32,
) -> Result<(RangeText, ServedBy), ReadError> {
    if let Some(text) = sources.read_local(store, meta, source_id, from, to)? {
        return Ok((text, ServedBy::Local));
    }
    let online = match SourceRef::from_id(source_id) {
        SourceRef::Arabic(s) => super::online::api_serves(s),
        SourceRef::Translation(_) => true,
    };
    let Some((client, transport, availability, now)) = api.filter(|_| online) else {
        return Err(ReadError::NotAvailable(source_id.to_owned()));
    };
    let text = client
        .read_range_guarded(transport, availability, now, source_id, from, to, meta)
        .await?;
    Ok((text, ServedBy::Api))
}

/// Reads a whole reader scope for an Arabic source and translations.
pub async fn read_scope(
    sources: &OpenSources,
    store: &ArtifactStore,
    meta: &QuranMeta,
    api: Option<(
        &QuranApi,
        &dyn Transport,
        &std::cell::RefCell<ApiAvailability>,
        i64,
    )>,
    scope: &ReaderScope,
    arabic: ArabicSource,
    translations: &[String],
) -> Result<ScopeRead, ReadError> {
    let (from, to) = scope_range(meta, scope).ok_or(ReadError::OutOfRange)?;
    let arabic = match read_source(sources, store, meta, api, arabic.id(), from, to).await {
        Ok((text, by)) => group_range(&text.ayahs, &text.normalizations, meta)
            .map(|g| (g, by))
            .map_err(|m| ReadError::Db(DbError::Decode(m))),
        Err(e) => Err(e),
    };
    let mut lanes = Vec::with_capacity(translations.len());
    for id in translations {
        lanes.push(SourceRead {
            source_id: id.clone(),
            result: read_source(sources, store, meta, api, id, from, to).await,
        });
    }
    Ok(ScopeRead {
        from,
        to,
        arabic,
        translations: lanes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quran::meta::meta;

    #[test]
    fn scopes_resolve_to_global_ranges() {
        let m = meta();
        assert_eq!(scope_range(m, &ReaderScope::Surah(1)), Some((1, 7)));
        assert_eq!(scope_range(m, &ReaderScope::Page(1)), Some((1, 7)));
        assert_eq!(scope_range(m, &ReaderScope::Juz(30)).unwrap().1, 6236);
        assert_eq!(scope_range(m, &ReaderScope::Hizb(1)).unwrap().0, 1);
        assert_eq!(scope_range(m, &ReaderScope::Surah(115)), None);
        assert_eq!(scope_range(m, &ReaderScope::Page(605)), None);
    }

    #[test]
    fn plan_and_gates() {
        assert_eq!(
            DEFAULT_SOURCE_PLAN.planned(),
            vec![ArabicSource::AnnotatedUthmani, ArabicSource::Uthmani]
        );
        assert!(!is_script_enabled(
            ArabicSource::Tajweed,
            DataEnvironment::Production
        ));
        assert!(is_script_enabled(
            ArabicSource::Tajweed,
            DataEnvironment::Local
        ));
        assert_eq!(enabled_scripts(DataEnvironment::Production).len(), 3);
    }

    #[test]
    fn open_sources_is_shareable() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OpenSources>();
    }
}
