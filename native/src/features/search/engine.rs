//! The search page's engine (`search/page/engine.svelte.ts`): a debounced
//! run over the Arabic corpus plus every selected, on-device, left-to-right
//! translation, with per-section state, stale-run dropping, isolated
//! failure, load more and retry, plus the surah and nav suggestions.

use std::time::Duration;

use eq_core::{
    quran::meta::{QuranMeta, meta},
    search::{
        SearchHit,
        latin::contains_arabic_script,
        nav::{NavMatch, is_nav_candidate, match_nav, split_place_token},
        normalize::{DEFAULT_LIMIT, MAX_OFFSET, MIN_QUERY_LEN},
        recents::{SEARCH_BATCH_SIZE, partition_searchable},
        suggest::{MAX_SUGGESTIONS, suggest_surahs},
    },
};
use nsh_core::prelude::*;

use crate::{
    app::quran_store,
    stores::search::{CorpusError, SearchStore, search_arabic, search_translation},
};

/// The Arabic section's id (never a translation id).
pub const ARABIC: &str = "arabic";
const DEBOUNCE: Duration = Duration::from_millis(140);
/// Room for the full sajda list plus coordinate matches.
const NAV_LIMIT: usize = 18;

/// Where a section is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loading,
    More,
    Done,
    Error,
    /// Skipped, with a reason shown.
    Gated,
}

/// One source's results.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// `arabic` or a translation id.
    pub id: String,
    pub phase: Phase,
    pub hits: Vec<SearchHit>,
    pub total: usize,
    /// Fetched-through cursor.
    pub offset: usize,
}

impl Section {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            phase: Phase::Loading,
            hits: Vec::new(),
            total: 0,
            offset: 0,
        }
    }

    /// The Arabic corpus's section.
    pub fn is_arabic(&self) -> bool {
        self.id == ARABIC
    }

    /// More pages remain.
    pub fn can_load_more(&self) -> bool {
        self.phase == Phase::Done && self.offset < MAX_OFFSET && self.offset < self.total
    }
}

/// A surah suggestion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurahSuggestion {
    pub num: u16,
    pub score: f64,
}

/// The page's engine: `Copy` signals the page owns.
#[derive(Clone, Copy)]
pub struct Engine {
    pub input: Signal<String>,
    pub committed: Signal<String>,
    pub searching: Signal<bool>,
    pub sections: Signal<Vec<Section>>,
    pub surahs: Signal<Vec<SurahSuggestion>>,
    pub nav: Signal<Vec<NavMatch>>,
    typed: Signal<u64>,
    run_seq: Signal<u64>,
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

fn is_cached(id: &str) -> bool {
    untracked(|| quran_store().is_installed(id))
}

impl Engine {
    /// A fresh engine under the current owner.
    pub fn new() -> Self {
        Self {
            input: signal(String::new()),
            committed: signal(String::new()),
            searching: signal(false),
            sections: signal(Vec::new()),
            surahs: signal(Vec::new()),
            nav: signal(Vec::new()),
            typed: signal(0),
            run_seq: signal(0),
        }
    }

    /// Types `query`: commits it after the debounce.
    pub fn run(self, query: String) {
        self.input.set(query.clone());
        self.typed.update(|n| *n += 1);
        let token = self.typed.get_untracked();
        spawn(async move {
            timer(DEBOUNCE).await;
            if self.typed.get_untracked() != token {
                return;
            }
            self.commit(query);
        });
    }

    fn current(self, seq: u64) -> bool {
        self.run_seq.get_untracked() == seq
    }

    fn commit(self, raw: String) {
        let query = raw.trim().to_owned();
        self.run_seq.update(|n| *n += 1);
        let seq = self.run_seq.get_untracked();
        self.committed.set(query.clone());
        if chars(&query) >= MIN_QUERY_LEN {
            SearchStore::get().record(&query);
        }
        self.surahs.set(Vec::new());
        self.nav.set(Vec::new());
        if chars(&query) < MIN_QUERY_LEN {
            self.sections.set(Vec::new());
            self.searching.set(false);
            if is_nav_candidate(&query) {
                self.suggest(&query);
            }
            return;
        }
        self.searching.set(true);
        let arabic_only = contains_arabic_script(&query);
        let (searchable, _) = partition_searchable(&untracked(|| SearchStore::get().selection()));
        let mut sections = vec![Section::new(ARABIC)];
        let mut cached = Vec::new();
        for id in searchable {
            if !is_cached(&id) {
                continue;
            }
            let mut section = Section::new(&id);
            if arabic_only {
                section.phase = Phase::Gated;
            }
            sections.push(section);
            cached.push(id);
        }
        self.sections.set(sections);
        self.suggest(&query);
        spawn(async move {
            self.fetch_into(ARABIC.to_owned(), 0, seq, query.clone())
                .await;
            if !arabic_only {
                for batch in cached.chunks(SEARCH_BATCH_SIZE) {
                    for id in batch {
                        if !self.current(seq) {
                            return;
                        }
                        self.fetch_into(id.clone(), 0, seq, query.clone()).await;
                    }
                }
            }
            if self.current(seq) {
                self.searching.set(false);
            }
        });
    }

    async fn fetch_into(self, id: String, offset: usize, seq: u64, query: String) {
        let result = if id == ARABIC {
            search_arabic(query, DEFAULT_LIMIT, offset).await
        } else {
            search_translation(id.clone(), query, DEFAULT_LIMIT, offset).await
        };
        if !self.current(seq) {
            return;
        }
        match result {
            Ok(page) => self.apply(&id, page.results, page.total, offset),
            Err(error) => {
                if let CorpusError::Failed(message) = &error {
                    log::warn!("search {id}: {message}");
                }
                self.fail(&id);
            }
        }
    }

    fn patch(self, id: &str, f: impl FnOnce(&mut Section)) {
        self.sections.update(|list| {
            if let Some(s) = list.iter_mut().find(|s| s.id == id) {
                f(s);
            }
        });
    }

    fn apply(self, id: &str, hits: Vec<SearchHit>, total: usize, offset: usize) {
        self.patch(id, |s| {
            let fetched = hits.len();
            s.hits.extend(hits);
            s.total = total;
            s.offset = s.offset.max(offset + fetched);
            s.phase = Phase::Done;
        });
    }

    fn fail(self, id: &str) {
        self.patch(id, |s| {
            if s.phase != Phase::Gated {
                s.phase = Phase::Error;
            }
        });
    }

    fn suggest(self, query: &str) {
        let m: &QuranMeta = meta();
        let placed = split_place_token(query);
        let surahs: Vec<SurahSuggestion> = match placed.place {
            Some(place) if placed.text.is_empty() => m
                .surahs()
                .iter()
                .filter(|s| s.place == place)
                .take(MAX_SUGGESTIONS)
                .map(|s| SurahSuggestion {
                    num: s.num,
                    score: 0.0,
                })
                .collect(),
            place => suggest_surahs(m, &placed.text, MAX_SUGGESTIONS)
                .into_iter()
                .filter(|s| place.is_none_or(|p| s.surah.place == p))
                .map(|s| SurahSuggestion {
                    num: s.surah.num,
                    score: s.score,
                })
                .collect(),
        };
        self.surahs.set(surahs);
        self.nav.set(match_nav(m, query, NAV_LIMIT));
    }

    /// The next page of `id`.
    pub fn load_more(self, id: &str) {
        let Some(section) = self
            .sections
            .with_untracked(|l| l.iter().find(|s| s.id == id).cloned())
        else {
            return;
        };
        if !section.can_load_more() {
            return;
        }
        let seq = self.run_seq.get_untracked();
        let query = self.committed.get_untracked();
        self.patch(id, |s| s.phase = Phase::More);
        let id = id.to_owned();
        spawn(async move {
            self.fetch_into(id, section.offset.min(MAX_OFFSET), seq, query)
                .await;
        });
    }

    /// Retries a failed section.
    pub fn retry(self, id: &str) {
        let Some(section) = self
            .sections
            .with_untracked(|l| l.iter().find(|s| s.id == id).cloned())
        else {
            return;
        };
        if section.phase != Phase::Error {
            return;
        }
        let seq = self.run_seq.get_untracked();
        let query = self.committed.get_untracked();
        let phase = if section.hits.is_empty() {
            Phase::Loading
        } else {
            Phase::More
        };
        self.patch(id, |s| s.phase = phase);
        let id = id.to_owned();
        spawn(async move {
            self.fetch_into(id, section.offset, seq, query).await;
        });
    }

    /// Runs newly cached selected translations once
    /// (`onArtifactsChanged`).
    pub fn artifacts_changed(self) {
        let query = self.committed.get_untracked();
        if chars(&query) < MIN_QUERY_LEN || contains_arabic_script(&query) {
            return;
        }
        let seq = self.run_seq.get_untracked();
        let (searchable, _) = partition_searchable(&untracked(|| SearchStore::get().selection()));
        for id in searchable {
            let known = self
                .sections
                .with_untracked(|l| l.iter().any(|s| s.id == id));
            if known || !is_cached(&id) {
                continue;
            }
            self.sections.update(|l| l.push(Section::new(&id)));
            let q = query.clone();
            spawn(async move {
                self.fetch_into(id, 0, seq, q).await;
            });
        }
    }
}
