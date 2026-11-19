//! Translation search (`quran/search/translation-corpus.ts` + `ranking.ts`):
//! a Latin-normalised copy of each verse; contiguous find, then relevance
//! ranking (term frequency, whole-phrase boost, early-verse bias, ties in
//! mushaf order) before paging.

use crate::quran::RangeText;

use super::{
    corpus::{Highlight, HitKind, SearchHit, SearchPage},
    latin::{is_eligible_latin_query, normalize_latin, normalize_latin_with_map},
    normalize::{char_occurrences, clamp_page},
};

#[derive(Debug, Clone)]
struct Unit {
    surah: u16,
    ayah: u16,
    global: u32,
    norm: String,
    starts: Vec<u32>,
    ends: Vec<u32>,
    text: String,
}

/// One translation's corpus.
#[derive(Debug, Clone, Default)]
pub struct TranslationCorpus {
    pub source_id: String,
    units: Vec<Unit>,
}

/// Rows out of mushaf order.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("[translation-search] expected contiguous globalIndex {expected}, received {received}")]
pub struct TranslationCorpusError {
    pub expected: u32,
    pub received: u32,
}

const TERM_FREQUENCY_WEIGHT: f64 = 3.0;
const EXACT_PHRASE_WEIGHT: f64 = 2.0;
const EARLY_VERSE_WEIGHT: f64 = 0.5;

fn is_word_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// `scoreUnit`: occurrences counted exactly as the highlight loop does.
fn score(norm: &str, query: &str, global: u32, corpus_size: usize) -> f64 {
    let mut occurrences = 0usize;
    let mut exact = false;
    for (at, _) in norm.match_indices(query) {
        occurrences += 1;
        let before = norm[..at].chars().next_back();
        let after = norm[at + query.len()..].chars().next();
        if !is_word_char(before) && !is_word_char(after) {
            exact = true;
        }
    }
    let early = if corpus_size > 0 {
        (corpus_size as f64 - f64::from(global)) / corpus_size as f64
    } else {
        0.0
    };
    TERM_FREQUENCY_WEIGHT * occurrences as f64
        + if exact { EXACT_PHRASE_WEIGHT } else { 0.0 }
        + EARLY_VERSE_WEIGHT * early
}

impl TranslationCorpus {
    /// `buildTranslationSearchCorpus`: `rows` is a full read, contiguous
    /// from global index 1.
    pub fn build(source_id: &str, rows: &RangeText) -> Result<Self, TranslationCorpusError> {
        let mut units = Vec::with_capacity(rows.ayahs.len());
        for (expected, row) in (1u32..).zip(&rows.ayahs) {
            if row.global_index != expected {
                return Err(TranslationCorpusError {
                    expected,
                    received: row.global_index,
                });
            }
            let map = normalize_latin_with_map(&row.text);
            units.push(Unit {
                surah: row.surah,
                ayah: row.ayah,
                global: row.global_index,
                norm: map.normalized,
                starts: map.starts,
                ends: map.ends,
                text: row.text.clone(),
            });
        }
        Ok(Self {
            source_id: source_id.to_owned(),
            units,
        })
    }

    /// Verses.
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// No verses.
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// `searchTranslationCorpus`: ranked, then paged.
    pub fn search(&self, query: &str, limit: usize, offset: usize) -> SearchPage {
        let normalized = normalize_latin(query);
        let (limit, offset) = clamp_page(limit, offset);
        if !is_eligible_latin_query(&normalized) {
            return SearchPage {
                total: 0,
                limit,
                offset,
                results: Vec::new(),
            };
        }
        let size = self.units.len();
        let mut ranked: Vec<(f64, &Unit)> = self
            .units
            .iter()
            .filter(|u| u.norm.contains(&normalized))
            .map(|u| (score(&u.norm, &normalized, u.global, size), u))
            .collect();
        // Stable: equal scores keep mushaf order.
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.global.cmp(&b.1.global)));
        SearchPage {
            total: ranked.len(),
            limit,
            offset,
            results: ranked
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|(_, u)| hit_for(u, &normalized))
                .collect(),
        }
    }
}

fn hit_for(unit: &Unit, normalized: &str) -> SearchHit {
    let highlights = char_occurrences(&unit.norm, normalized)
        .into_iter()
        .filter_map(|(first, last)| {
            let start = *unit.starts.get(first)?;
            let end = *unit.ends.get(last)?;
            (start < end).then_some(Highlight {
                start: start as usize,
                end: end as usize,
            })
        })
        .collect();
    SearchHit {
        kind: HitKind::Ayah,
        surah: unit.surah,
        ayah: unit.ayah,
        global_index: unit.global,
        text: unit.text.clone(),
        highlights,
    }
}
