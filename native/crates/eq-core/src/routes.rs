//! Reader URL grammar (PARITY A2): the reader's targets as plain types, and
//! later the parser/printer for `/{surah}`, `/page/n`, `/juz/n`, `/hizb/n`,
//! `/t/{lang}/{translator}/…` and `?ayah=` that deep links and share links
//! use (mirror of `web/src/lib/data/quran.ts` `surah*For`). Parsing lands
//! with A2; only the types live here now.

use serde::{Deserialize, Serialize};

/// A translation source as the URL names it: `/t/{lang}/{translator}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TranslationRef {
    pub lang: String,
    pub translator: String,
}

/// Which slice of the mushaf a reader shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReaderScope {
    /// Surah 1..=114.
    Surah(u16),
    /// Mushaf page 1..=604.
    Page(u16),
    /// Juz 1..=30.
    Juz(u8),
    /// Hizb 1..=60.
    Hizb(u8),
}

/// One reader route: scope, optional translation, optional ayah anchor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReaderTarget {
    pub scope: ReaderScope,
    pub translation: Option<TranslationRef>,
    pub ayah: Option<u16>,
}

impl ReaderTarget {
    /// The Arabic-only reader of `scope`, at its start.
    pub fn arabic(scope: ReaderScope) -> Self {
        Self {
            scope,
            translation: None,
            ayah: None,
        }
    }
}
