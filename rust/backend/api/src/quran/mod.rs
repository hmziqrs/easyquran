pub mod loader;
pub mod normalize;
pub mod search;
pub mod store;
pub mod translation_pool;
pub mod view;

pub use loader::{load_catalogue, load_quran_store, load_translation_corpus, QuranLoadError};
pub use normalize::normalize_arabic;
pub use search::highlight;
pub use store::{
    range_containing, AyahView, Bismillah, CatalogueEntry, Corpus, HizbQuarter, Juz, Manzil,
    Navigation, Page, Place, QuranMeta, QuranStore, Range, Ruku, Sajda, SajdaKind, Script,
    SourceId, SuraMeta, TranslationId, RESPONSE_CAP, SURA_COUNT, VERSE_COUNT,
};
pub use translation_pool::{PoolStats, TranslationPool};
pub use view::{
    normalization as surah_normalization, normalization_translation, surah_text as surah_text_view,
    surah_text_translation, OpenerKindDto, OpenerPackagingDto, QuranSurahTextDto,
    SurahNormalizationDto, ViewError,
};
