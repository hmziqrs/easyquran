//! Quran content API — in-memory store + boot loader (Phase 0, §4).
//!
//! [`store`] holds the immutable content types; [`loader`] is the only place
//! that reads SQLite for Quran content (it owns its own read-only connections,
//! independent of `sea_db`). Public handlers under `modules/quran_v1/` consume
//! the store purely in-memory and contain no sqlx/SQLite reference (§10).

pub mod loader;
pub mod normalize;
pub mod search;
pub mod store;

pub use loader::{load_quran_store, QuranLoadError};
pub use normalize::normalize_arabic;
pub use search::highlight;
pub use store::{
    range_containing, AyahView, Bismillah, Corpus, HizbQuarter, Juz, Manzil, Navigation, Page,
    Place, QuranMeta, QuranStore, Range, Ruku, Sajda, SajdaKind, Script, SourceDigests, SuraMeta,
    RESPONSE_CAP, SEARCH_VERSION, SURA_COUNT, VERSE_COUNT,
};
