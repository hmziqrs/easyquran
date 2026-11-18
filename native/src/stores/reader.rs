//! Reader preferences (PARITY C4, C8): Arabic size, face, script, mode and
//! the translation size and family. The web's `reader-core.svelte.ts` +
//! `reader-settings.svelte.ts` + `reader-persistence.svelte.ts`: the same
//! persisted blob (`easyquran.reader`, schema v6, device-local) with the
//! same bounds and steps. Across devices the web syncs these values as the
//! settings document's `reading` section (`settings.reading.*` registers,
//! J9); the settings store reads [`ReaderStore::reading_doc`] and applies
//! incoming values with [`ReaderStore::apply_reading_doc`].
//!
//! One owner: a surface-wide signal (`global`), loaded from storage on the
//! first [`ReaderStore::get`], so the reader, its settings sheet and the
//! settings page write the same value from bodies, listeners and tasks.
//!
//! The web has no line-height setting (the Arabic line is fixed at 2.15,
//! `ReadingAyah.svelte`), so none is stored here.

use eq_core::quran::{
    catalogue::DEFAULT_READER_SOURCE,
    read::{DataEnvironment, is_script_enabled},
    types::ArabicSource,
};
use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use serde_json::{Value, json};

use crate::ui::arabic::{ArabicFont, ArabicStyle, QuranScript};

/// The key the web keeps the reader blob under (`STORAGE_KEY`).
pub const STORAGE_KEY: &str = "easyquran.reader";
/// `READER_SCHEMA_VERSION`.
pub const SCHEMA_VERSION: u32 = 6;

pub const ARABIC_FONT_MIN: f32 = 22.0;
pub const ARABIC_FONT_MAX: f32 = 56.0;
pub const ARABIC_FONT_STEP: f32 = 3.0;
pub const TRANSLATION_FONT_MIN: f32 = 13.0;
pub const TRANSLATION_FONT_MAX: f32 = 28.0;
pub const TRANSLATION_FONT_STEP: f32 = 1.0;
/// The Arabic line height (`.arabic-text { line-height: 2.15 }`).
pub const ARABIC_LINE_HEIGHT: f32 = 2.15;

pub use eq_core::routes::ReaderMode;

/// The translation typeface family (`TranslationFamily`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TranslationFamily {
    #[default]
    Sans,
    Serif,
}

impl TranslationFamily {
    pub fn id(self) -> &'static str {
        match self {
            TranslationFamily::Sans => "sans",
            TranslationFamily::Serif => "serif",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "sans" => Some(TranslationFamily::Sans),
            "serif" => Some(TranslationFamily::Serif),
            _ => None,
        }
    }
}

/// The persisted reader settings (`Persisted`, the fields v6 keeps).
#[derive(Clone, Debug, PartialEq)]
pub struct ReaderPrefs {
    /// Last surah opened (`current`, 1..=114).
    pub current: u16,
    pub font_size: f32,
    pub arabic_font: ArabicFont,
    pub arabic_script: ArabicSource,
    pub translation_size: f32,
    pub translation_family: TranslationFamily,
    pub mode: ReaderMode,
}

impl Default for ReaderPrefs {
    fn default() -> Self {
        Self {
            current: 1,
            font_size: 33.0,
            arabic_font: ArabicFont::KfgqpcHafs,
            arabic_script: DEFAULT_READER_SOURCE,
            translation_size: 17.0,
            translation_family: TranslationFamily::Sans,
            mode: ReaderMode::Verse,
        }
    }
}

fn num_in(v: &Value, min: f32, max: f32) -> Option<f32> {
    #[allow(clippy::cast_possible_truncation)]
    let n = v.as_f64()? as f32;
    (n.is_finite() && n >= min && n <= max).then_some(n)
}

/// The data environment the script gate reads (`clientQuranEnvironment`):
/// development builds and a local Quran directory are `Local`.
pub fn data_environment() -> DataEnvironment {
    if cfg!(debug_assertions) || crate::services::files::dev_quran_dir().is_some() {
        DataEnvironment::Local
    } else {
        DataEnvironment::Production
    }
}

impl ReaderPrefs {
    /// `decodeReader` over a parsed blob: each valid field wins, the rest
    /// keep their defaults; a future schema reads as defaults.
    pub fn from_value(raw: &Value) -> Self {
        let mut out = Self::default();
        let Some(o) = raw.as_object() else {
            return out;
        };
        if o.get("v").and_then(Value::as_u64).is_some_and(|v| v == 0) {
            return out;
        }
        if let Some(n) = o.get("current").and_then(|v| num_in(v, 1.0, 114.0)) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                out.current = n as u16;
            }
        }
        out.apply_reading(raw);
        out
    }

    /// Applies the `reading` fields of a settings document (or the blob,
    /// which uses the same names); invalid values are ignored.
    fn apply_reading(&mut self, doc: &Value) {
        if let Some(n) = doc
            .get("fontSize")
            .and_then(|v| num_in(v, ARABIC_FONT_MIN, ARABIC_FONT_MAX))
        {
            self.font_size = n;
        }
        if let Some(f) = doc
            .get("arabicFont")
            .and_then(Value::as_str)
            .and_then(ArabicFont::from_id)
        {
            self.arabic_font = f;
        }
        if let Some(s) = doc
            .get("arabicScript")
            .and_then(Value::as_str)
            .and_then(ArabicSource::from_id)
            .filter(|s| is_script_enabled(*s, data_environment()))
        {
            self.arabic_script = s;
        }
        if let Some(n) = doc
            .get("translationSize")
            .and_then(|v| num_in(v, TRANSLATION_FONT_MIN, TRANSLATION_FONT_MAX))
        {
            self.translation_size = n;
        }
        if let Some(f) = doc
            .get("translationFamily")
            .and_then(Value::as_str)
            .and_then(TranslationFamily::from_id)
        {
            self.translation_family = f;
        }
        if let Some(m) = doc
            .get("mode")
            .and_then(Value::as_str)
            .and_then(ReaderMode::parse)
        {
            self.mode = m;
        }
    }

    /// The blob as the web writes it.
    pub fn to_value(&self) -> Value {
        json!({
            "v": SCHEMA_VERSION,
            "current": self.current,
            "fontSize": self.font_size,
            "arabicFont": self.arabic_font.id(),
            "arabicScript": self.arabic_script.id(),
            "translationSize": self.translation_size,
            "translationFamily": self.translation_family.id(),
            "mode": self.mode.as_str(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
struct Held {
    loaded: bool,
    prefs: ReaderPrefs,
}

/// The reader settings store: a `Copy` handle over the surface's signal.
/// Reads are tracked; every write persists at once (`writeNow`).
#[derive(Clone, Copy)]
pub struct ReaderStore {
    held: Signal<Held>,
}

impl ReaderStore {
    /// The surface's store; the first call loads the saved blob.
    pub fn get() -> ReaderStore {
        let held = global::<Held>();
        if !held.with_untracked(|h| h.loaded) {
            let mut prefs = Storage::get_item(STORAGE_KEY)
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .map(|v| ReaderPrefs::from_value(&v))
                .unwrap_or_default();
            dev_overrides(&mut prefs);
            held.set(Held {
                loaded: true,
                prefs,
            });
        }
        ReaderStore { held }
    }

    /// Every setting. Tracked.
    pub fn prefs(self) -> ReaderPrefs {
        self.held.with(|h| h.prefs.clone())
    }

    fn read<T>(self, f: impl FnOnce(&ReaderPrefs) -> T) -> T {
        self.held.with(|h| f(&h.prefs))
    }

    fn write(self, f: impl FnOnce(&mut ReaderPrefs)) {
        let mut next = self.held.with_untracked(|h| h.prefs.clone());
        f(&mut next);
        let same = self.held.with_untracked(|h| h.prefs == next);
        if same {
            return;
        }
        if let Err(e) = Storage::set_item(STORAGE_KEY, next.to_value().to_string()) {
            log::warn!("reader prefs not saved: {e:?}");
        }
        self.held.set(Held {
            loaded: true,
            prefs: next,
        });
    }

    pub fn font_size(self) -> f32 {
        self.read(|p| p.font_size)
    }
    pub fn arabic_font(self) -> ArabicFont {
        self.read(|p| p.arabic_font)
    }
    pub fn script(self) -> ArabicSource {
        self.read(|p| p.arabic_script)
    }
    pub fn mode(self) -> ReaderMode {
        self.read(|p| p.mode)
    }
    pub fn translation_size(self) -> f32 {
        self.read(|p| p.translation_size)
    }
    pub fn translation_family(self) -> TranslationFamily {
        self.read(|p| p.translation_family)
    }

    /// How the Arabic is set for `script` (a row's own edition: a range
    /// read keeps the script it was read in).
    pub fn style_for(self, script: QuranScript) -> ArabicStyle {
        self.read(|p| ArabicStyle {
            script,
            font: p.arabic_font,
            size: p.font_size,
            line_height: ARABIC_LINE_HEIGHT,
        })
    }

    /// A+ (`bigger`).
    pub fn bigger(self) {
        self.write(|p| p.font_size = (p.font_size + ARABIC_FONT_STEP).min(ARABIC_FONT_MAX));
    }
    /// A− (`smaller`).
    pub fn smaller(self) {
        self.write(|p| p.font_size = (p.font_size - ARABIC_FONT_STEP).max(ARABIC_FONT_MIN));
    }
    pub fn can_grow(self) -> bool {
        self.read(|p| p.font_size < ARABIC_FONT_MAX)
    }
    pub fn can_shrink(self) -> bool {
        self.read(|p| p.font_size > ARABIC_FONT_MIN)
    }
    pub fn set_arabic_font(self, font: ArabicFont) {
        self.write(|p| p.arabic_font = font);
    }
    /// `setArabicScript`: a script the build gates off is refused.
    pub fn set_script(self, script: ArabicSource) {
        if !is_script_enabled(script, data_environment()) {
            return;
        }
        self.write(|p| p.arabic_script = script);
    }
    pub fn set_mode(self, mode: ReaderMode) {
        self.write(|p| p.mode = mode);
    }
    pub fn grow_translation(self) {
        self.write(|p| {
            p.translation_size =
                (p.translation_size + TRANSLATION_FONT_STEP).min(TRANSLATION_FONT_MAX);
        });
    }
    pub fn shrink_translation(self) {
        self.write(|p| {
            p.translation_size =
                (p.translation_size - TRANSLATION_FONT_STEP).max(TRANSLATION_FONT_MIN);
        });
    }
    pub fn set_translation_family(self, family: TranslationFamily) {
        self.write(|p| p.translation_family = family);
    }
    /// `setCurrent`: the surah last opened.
    pub fn set_current(self, surah: u16) {
        self.write(|p| p.current = surah);
    }

    /// The settings document's `reading` section (`toSettingsDoc`), the
    /// values the `settings.reading.*` registers hold.
    pub fn reading_doc(self) -> Value {
        self.held.with_untracked(|h| {
            let p = &h.prefs;
            json!({
                "fontSize": p.font_size,
                "mode": p.mode.as_str(),
                "arabicFont": p.arabic_font.id(),
                "arabicScript": p.arabic_script.id(),
                "translationSize": p.translation_size,
                "translationFamily": p.translation_family.id(),
            })
        })
    }

    /// Applies a synced `reading` section (`applySyncedSettings`).
    pub fn apply_reading_doc(self, doc: &Value) {
        self.write(|p| p.apply_reading(doc));
    }
}

/// Debug builds: `EQ_READER_SCRIPT` (a source id) and `EQ_READER_MODE`
/// (`verse`/`reading`) override the loaded values for screenshots; never
/// written back unless the user changes a setting.
fn dev_overrides(prefs: &mut ReaderPrefs) {
    if !cfg!(debug_assertions) {
        return;
    }
    if let Some(s) = std::env::var("EQ_READER_SCRIPT")
        .ok()
        .and_then(|id| ArabicSource::from_id(&id))
    {
        prefs.arabic_script = s;
    }
    if let Some(m) = std::env::var("EQ_READER_MODE")
        .ok()
        .and_then(|m| ReaderMode::parse(&m))
    {
        prefs.mode = m;
    }
}
