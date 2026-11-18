//! Search state (PARITY E1, E4): the device-local recents and the search
//! page's translation selection (persisted under the web's keys), plus the
//! in-memory search corpora.
//!
//! Corpora are built off the engine thread from full reads of the
//! on-device databases (`QuranStore::read_source`, read-only) and kept for
//! the run: one Arabic corpus for the source plan's match/display pair and
//! one per searched translation. Nothing is written back, nothing hashed.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use eq_core::{
    clock::{Clock, SystemClock},
    quran::{SourceRef, meta::AYAH_COUNT, read::DEFAULT_SOURCE_PLAN},
    search::{
        ArabicCorpus, SearchPage, TranslationCorpus,
        palette::Resume,
        recents::{RECENTS_KEY, Recents, SELECTION_KEY, decode_selection, encode_selection, uniq},
    },
    store::sessions,
};
use nsh_core::{prelude::*, storage::Storage};

use crate::app::quran_store;

/// Reads where "Continue reading" would resume (this tab's thread, else the
/// newest open one) and hands it to `then` on the engine thread; `None`
/// before the app store opens.
pub fn load_resume(then: impl FnOnce(Option<Resume>) + 'static) {
    let Some(sync) = crate::stores::sync::sync_store() else {
        then(None);
        return;
    };
    sync.run(
        |store| {
            let thread = match sessions::current(store)? {
                Some(t) => Some(t),
                None => sessions::latest(store)?,
            };
            Ok(thread)
        },
        move |out| then(out.ok().flatten().map(|t| resume_of(&t))),
    );
}

fn resume_of(t: &sessions::ReadingThread) -> Resume {
    let source_id = t
        .source_id
        .clone()
        .filter(|id| matches!(SourceRef::from_id(id), SourceRef::Translation(_)));
    Resume {
        thread: t.id.clone(),
        surah: u16::try_from(t.surah).unwrap_or(1),
        ayah: u16::try_from(t.ayah).unwrap_or(1),
        source_id,
    }
}

#[derive(Default, Clone, PartialEq)]
struct Held {
    loaded: bool,
    recents: Recents,
    selection: Vec<String>,
}

/// The search store: a `Copy` handle over the surface's state.
#[derive(Clone, Copy)]
pub struct SearchStore {
    held: Signal<Held>,
}

/// A query handed to the search page before it opens (the palette's
/// "search for…", a launch argument in debug builds).
#[derive(Default, Clone, PartialEq)]
struct Seed(Option<String>);

impl SearchStore {
    /// The surface's store; the first call loads the saved blobs.
    pub fn get() -> SearchStore {
        let held = global::<Held>();
        if !held.with_untracked(|h| h.loaded) {
            held.set(Held {
                loaded: true,
                recents: Recents::decode(Storage::get_item(RECENTS_KEY).as_deref()),
                selection: decode_selection(Storage::get_item(SELECTION_KEY).as_deref()),
            });
        }
        SearchStore { held }
    }

    /// Recent queries, newest first. Tracked.
    pub fn recents(self) -> Vec<String> {
        self.held.with(|h| h.recents.queries())
    }

    /// History is paused. Tracked.
    pub fn paused(self) -> bool {
        self.held.with(|h| h.recents.paused)
    }

    fn save_recents(self, recents: Recents) {
        if let Err(e) = Storage::set_item(RECENTS_KEY, recents.encode()) {
            log::warn!("search recents not saved: {e:?}");
        }
        self.held.update(|h| h.recents = recents);
    }

    /// Records an executed search (no-op while paused).
    pub fn record(self, q: &str) {
        let mut recents = self.held.with_untracked(|h| h.recents.clone());
        if recents.record(q, SystemClock.unix_ms()) {
            self.save_recents(recents);
        }
    }

    /// Clears the history.
    pub fn clear_recents(self) {
        let mut recents = self.held.with_untracked(|h| h.recents.clone());
        if recents.clear() {
            self.save_recents(recents);
        }
    }

    /// Pauses or resumes recording.
    pub fn set_paused(self, paused: bool) {
        let mut recents = self.held.with_untracked(|h| h.recents.clone());
        if recents.paused == paused {
            return;
        }
        recents.paused = paused;
        self.save_recents(recents);
    }

    /// The translations the search page searches. Tracked.
    pub fn selection(self) -> Vec<String> {
        self.held.with(|h| h.selection.clone())
    }

    /// Replaces the selection (deduplicated).
    pub fn set_selection(self, ids: &[String]) {
        let next = uniq(ids);
        if self.held.with_untracked(|h| h.selection == next) {
            return;
        }
        if let Err(e) = Storage::set_item(SELECTION_KEY, encode_selection(&next)) {
            log::warn!("search selection not saved: {e:?}");
        }
        self.held.update(|h| h.selection = next);
    }

    /// Adds or removes `id`.
    pub fn toggle(self, id: &str) {
        let mut ids = self.held.with_untracked(|h| h.selection.clone());
        match ids.iter().position(|x| x == id) {
            Some(at) => {
                ids.remove(at);
            }
            None => ids.push(id.to_owned()),
        }
        self.set_selection(&ids);
    }

    /// Drops `id`.
    pub fn remove(self, id: &str) {
        let ids: Vec<String> = self
            .held
            .with_untracked(|h| h.selection.iter().filter(|x| *x != id).cloned().collect());
        self.set_selection(&ids);
    }

    /// Hands the search page a query to run as it opens.
    pub fn seed(q: impl Into<String>) {
        global::<Seed>().set(Seed(Some(q.into())));
    }

    /// The seeded query, once.
    pub fn take_seed() -> Option<String> {
        let seed = global::<Seed>();
        let q = seed.with_untracked(|s| s.0.clone());
        if q.is_some() {
            seed.set(Seed(None));
        }
        q
    }
}

/// Why a section could not be searched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusError {
    /// The database is not on the device (the web's worker needs it cached).
    NotOnDevice(String),
    /// Reading or building failed.
    Failed(String),
}

static ARABIC: Mutex<Option<Arc<ArabicCorpus>>> = Mutex::new(None);

fn translations() -> &'static Mutex<HashMap<String, Arc<TranslationCorpus>>> {
    static MAP: std::sync::OnceLock<Mutex<HashMap<String, Arc<TranslationCorpus>>>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(Default::default)
}

/// Drops a corpus (its database was deleted).
pub fn forget_corpus(id: &str) {
    translations()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(id);
    let plan = DEFAULT_SOURCE_PLAN;
    if id == plan.search_match.id() || id == plan.search_display.id() {
        *ARABIC.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

async fn read_all(id: &str) -> Result<eq_core::quran::RangeText, CorpusError> {
    if !quran_store().is_installed(id) {
        return Err(CorpusError::NotOnDevice(id.to_owned()));
    }
    quran_store()
        .read_source(id, 1, AYAH_COUNT)
        .await
        .map(|(text, _)| text)
        .map_err(|e| CorpusError::Failed(e.to_string()))
}

async fn arabic_corpus() -> Result<Arc<ArabicCorpus>, CorpusError> {
    if let Some(c) = ARABIC.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(c);
    }
    let plan = DEFAULT_SOURCE_PLAN;
    let matching = read_all(plan.search_match.id()).await?;
    let display = if plan.search_display == plan.search_match {
        None
    } else {
        Some(read_all(plan.search_display.id()).await?)
    };
    let corpus =
        blocking(move || ArabicCorpus::build(&matching, display.as_ref().unwrap_or(&matching)))
            .await
            .map_err(|e| CorpusError::Failed(e.to_string()))?
            .map_err(|e| CorpusError::Failed(e.to_string()))?;
    let corpus = Arc::new(corpus);
    *ARABIC.lock().unwrap_or_else(|e| e.into_inner()) = Some(corpus.clone());
    Ok(corpus)
}

async fn translation_corpus(id: &str) -> Result<Arc<TranslationCorpus>, CorpusError> {
    if let Some(c) = translations()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        .cloned()
    {
        return Ok(c);
    }
    let rows = read_all(id).await?;
    let owned = id.to_owned();
    let corpus = blocking(move || TranslationCorpus::build(&owned, &rows))
        .await
        .map_err(|e| CorpusError::Failed(e.to_string()))?
        .map_err(|e| CorpusError::Failed(e.to_string()))?;
    let corpus = Arc::new(corpus);
    translations()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id.to_owned(), corpus.clone());
    Ok(corpus)
}

/// One page of the Arabic search. The corpus builds on first use.
pub async fn search_arabic(
    query: String,
    limit: usize,
    offset: usize,
) -> Result<SearchPage, CorpusError> {
    let corpus = arabic_corpus().await?;
    blocking(move || corpus.search(&query, limit, offset))
        .await
        .map_err(|e| CorpusError::Failed(e.to_string()))
}

/// One page of translation `id`'s search, ranked.
pub async fn search_translation(
    id: String,
    query: String,
    limit: usize,
    offset: usize,
) -> Result<SearchPage, CorpusError> {
    let corpus = translation_corpus(&id).await?;
    blocking(move || corpus.search(&query, limit, offset))
        .await
        .map_err(|e| CorpusError::Failed(e.to_string()))
}
