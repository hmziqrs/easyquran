//! The Arabic search corpus (`quran/search/corpus.ts`): one unit per
//! surah opener (bismillah header) and per verse, in mushaf order, each a
//! normalised match surface aligned onto the verbatim display text.
//! Matching is a contiguous find over the normalised copy; hits carry the
//! display text verbatim plus byte-offset highlight ranges into it.

use crate::quran::{OpenerKind, RangeText, SurahNormalization, view};

use super::{
    alignment::align_search_text,
    normalize::{char_occurrences, clamp_page, is_eligible_query, normalize_arabic},
};

/// A highlighted byte range `start..end` of a hit's verbatim text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
}

/// What a hit is (`SearchHitKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitKind {
    /// A verse.
    Ayah,
    /// A surah's bismillah header; anchors at its first verse.
    Opener,
}

/// One search hit: a verse (or opener) and its verbatim text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub kind: HitKind,
    pub surah: u16,
    /// The verse it opens at (1 for an opener).
    pub ayah: u16,
    pub global_index: u32,
    /// Verbatim from the database.
    pub text: String,
    pub highlights: Vec<Highlight>,
}

impl SearchHit {
    /// Unique key: `S:A`, or `opener:S`.
    pub fn key(&self) -> String {
        match self.kind {
            HitKind::Ayah => format!("{}:{}", self.surah, self.ayah),
            HitKind::Opener => format!("opener:{}", self.surah),
        }
    }
}

/// One page of hits (`CanonicalCorpusSearchResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchPage {
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub results: Vec<SearchHit>,
}

/// A corpus that cannot be built from the rows it was given.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("[quran-search] {0}")]
pub struct CorpusError(pub String);

#[derive(Debug, Clone)]
struct Unit {
    kind: HitKind,
    surah: u16,
    ayah: u16,
    global: u32,
    norm: String,
    starts: Vec<u32>,
    ends: Vec<u32>,
    display: String,
    display_base: u32,
}

/// The Arabic corpus of one match/display source pair.
#[derive(Debug, Clone, Default)]
pub struct ArabicCorpus {
    units: Vec<Unit>,
}

fn normalization(rows: &RangeText, surah: u16) -> Option<&SurahNormalization> {
    rows.normalizations.iter().find(|n| n.surah == surah)
}

impl ArabicCorpus {
    /// `buildCanonicalSearchCorpus`: `matching` and `display` are full
    /// reads (`1..=6236`) of the match and display sources.
    pub fn build(matching: &RangeText, display: &RangeText) -> Result<Self, CorpusError> {
        if matching.ayahs.len() != display.ayahs.len() {
            return Err(CorpusError("source row counts differ".into()));
        }
        let mut units = Vec::with_capacity(matching.ayahs.len() + 114);
        for (m, d) in matching.ayahs.iter().zip(&display.ayahs) {
            if m.global_index != d.global_index || m.surah != d.surah || m.ayah != d.ayah {
                return Err(CorpusError(format!(
                    "source coordinate mismatch at {}",
                    m.global_index
                )));
            }
            let mn = normalization(matching, m.surah).ok_or_else(|| {
                CorpusError(format!("missing normalization for surah {}", m.surah))
            })?;
            let dn = normalization(display, d.surah).ok_or_else(|| {
                CorpusError(format!("missing normalization for surah {}", d.surah))
            })?;
            if m.ayah == 1 && mn.opener_kind == OpenerKind::Header {
                let (Some(mo), Some(dopen)) = (view::header_raw(mn), view::header_raw(dn)) else {
                    return Err(CorpusError(format!(
                        "missing opener text for surah {}",
                        m.surah
                    )));
                };
                let a = align_search_text(mo, dopen);
                units.push(Unit {
                    kind: HitKind::Opener,
                    surah: m.surah,
                    ayah: 1,
                    global: m.global_index,
                    norm: a.norm,
                    starts: a.starts,
                    ends: a.ends,
                    display: dopen.to_owned(),
                    display_base: 0,
                });
            }
            let match_body = view::body_text(&m.text, m.ayah, mn);
            let display_body = view::body_text(&d.text, d.ayah, dn);
            let a = align_search_text(match_body, display_body);
            // The body is a suffix of the verbatim row: its spans shift by
            // the bytes cut before it.
            let display_base = (d.text.len() - display_body.len()) as u32;
            units.push(Unit {
                kind: HitKind::Ayah,
                surah: m.surah,
                ayah: m.ayah,
                global: m.global_index,
                norm: a.norm,
                starts: a.starts,
                ends: a.ends,
                display: d.text.clone(),
                display_base,
            });
        }
        Ok(Self { units })
    }

    /// Units (openers and verses).
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// No units.
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// `searchCanonicalCorpus`: mushaf order, paged.
    pub fn search(&self, query: &str, limit: usize, offset: usize) -> SearchPage {
        let normalized = normalize_arabic(query);
        let (limit, offset) = clamp_page(limit, offset);
        if !is_eligible_query(&normalized) {
            return SearchPage {
                total: 0,
                limit,
                offset,
                results: Vec::new(),
            };
        }
        let matching: Vec<&Unit> = self
            .units
            .iter()
            .filter(|u| u.norm.contains(&normalized))
            .collect();
        SearchPage {
            total: matching.len(),
            limit,
            offset,
            results: matching
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|u| hit_for(u, &normalized))
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
            (start < end).then(|| Highlight {
                start: (unit.display_base + start) as usize,
                end: (unit.display_base + end) as usize,
            })
        })
        .collect();
    SearchHit {
        kind: unit.kind,
        surah: unit.surah,
        ayah: unit.ayah,
        global_index: unit.global,
        text: unit.display.clone(),
        highlights,
    }
}
