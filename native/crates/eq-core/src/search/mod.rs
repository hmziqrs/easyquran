//! Search (PARITY E1-E4): Arabic + translation normalisation, transliteration, ranking, highlights, go-to-verse parsing, palette sources, recents.
//!
//! Map of the web sources mirrored:
//! - [`normalize`]: `quran/search/normalize.ts`
//! - [`latin`]: `quran/search/normalize-latin.ts`
//! - [`translit`]: `quran/search/translit.ts`, `palette/surah-aliases.ts`
//! - [`alignment`]: `quran/search/alignment.ts`
//! - [`corpus`]: `quran/search/corpus.ts`, `types.ts`
//! - [`translation`]: `quran/search/translation-corpus.ts`, `ranking.ts`
//! - [`highlights`]: `quran/search/highlights.ts`
//! - [`nav`]: `search/nav/*`
//! - [`suggest`]: `palette/scoring.ts`, `page/surah-suggest.ts`
//! - [`palette`]: `search/palette/**`
//! - [`recents`]: `stores/search-{recents,selection}.svelte.ts`,
//!   `page/{selection,query-url}.ts`
//!
//! Normalisation runs on index copies and queries only: every hit carries
//! its text verbatim from the database, with highlight byte ranges into it.
//! Nothing here hashes or writes Quran data.

pub mod alignment;
pub mod corpus;
pub mod highlights;
pub mod latin;
pub mod nav;
pub mod normalize;
pub mod palette;
pub mod recents;
pub mod suggest;
pub mod translation;
pub mod translit;

pub use corpus::{ArabicCorpus, Highlight, HitKind, SearchHit, SearchPage};
pub use translation::TranslationCorpus;

#[cfg(test)]
mod tests;
