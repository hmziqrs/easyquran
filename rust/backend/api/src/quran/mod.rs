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
