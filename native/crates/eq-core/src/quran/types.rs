//! Shared Quran value types (mirror of `web/src/lib/data/quran-types.ts`).
//! Wire names are the web's (`serde` renames), so the same JSON the read API
//! and the web worker speak decodes here unchanged.

use serde::{Deserialize, Serialize};

/// The five Arabic sources (`QuranSourceId`). The string form is the id the
/// web, the read API and thread rows use; it never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArabicSource {
    #[serde(rename = "uthmani")]
    Uthmani,
    #[serde(rename = "uthmani-annotated")]
    AnnotatedUthmani,
    #[serde(rename = "simple-clean")]
    SimpleClean,
    #[serde(rename = "indopak")]
    IndoPak,
    #[serde(rename = "tajweed")]
    Tajweed,
}

impl ArabicSource {
    /// Every Arabic source, in the web's registry order.
    pub const ALL: [ArabicSource; 5] = [
        ArabicSource::Uthmani,
        ArabicSource::AnnotatedUthmani,
        ArabicSource::SimpleClean,
        ArabicSource::IndoPak,
        ArabicSource::Tajweed,
    ];

    /// The source id (`"uthmani"`, `"uthmani-annotated"`, ...).
    pub fn id(self) -> &'static str {
        match self {
            ArabicSource::Uthmani => "uthmani",
            ArabicSource::AnnotatedUthmani => "uthmani-annotated",
            ArabicSource::SimpleClean => "simple-clean",
            ArabicSource::IndoPak => "indopak",
            ArabicSource::Tajweed => "tajweed",
        }
    }

    /// The source a source id names, `None` for a translation id.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Its script.
    pub fn script(self) -> QuranScript {
        match self {
            ArabicSource::Uthmani | ArabicSource::AnnotatedUthmani => QuranScript::Uthmani,
            ArabicSource::SimpleClean => QuranScript::SimpleClean,
            ArabicSource::IndoPak => QuranScript::IndoPak,
            ArabicSource::Tajweed => QuranScript::Tajweed,
        }
    }
}

/// Any reader source: one Arabic script or one translation, by id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceRef {
    Arabic(ArabicSource),
    Translation(String),
}

impl SourceRef {
    /// Classifies a source id the way `isArabicSourceId` does.
    pub fn from_id(id: &str) -> Self {
        match ArabicSource::from_id(id) {
            Some(source) => SourceRef::Arabic(source),
            None => SourceRef::Translation(id.to_owned()),
        }
    }

    /// The id.
    pub fn id(&self) -> &str {
        match self {
            SourceRef::Arabic(source) => source.id(),
            SourceRef::Translation(id) => id,
        }
    }
}

/// `QuranScript`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuranScript {
    Uthmani,
    SimpleClean,
    #[serde(rename = "indopak")]
    IndoPak,
    Tajweed,
    Translation,
}

/// What opens a surah (`OpenerKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenerKind {
    /// Surah 1: the bismillah is verse 1.
    Verse,
    /// Every other surah but 9: a bismillah header above verse 1.
    Header,
    /// Surah 9: none.
    None,
}

/// How a source stores a surah's opener (`OpenerPackaging`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenerPackaging {
    NumberedAyah,
    EmbeddedPrefix,
    ChapterFlag,
    SeparateRow,
    Absent,
}

/// One surah's opener partition for one source (`SurahNormalization`).
/// Scalar indexes count Unicode scalar values (Rust `char`s), as the web's
/// `Array.from(text)` does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurahNormalization {
    pub surah: u16,
    pub source_id: String,
    pub script: QuranScript,
    pub source_profile: String,
    pub packaging: OpenerPackaging,
    pub opener_kind: OpenerKind,
    pub opener_text: Option<String>,
    pub opener_end_scalar: u32,
    pub body_start_scalar: u32,
}

impl SurahNormalization {
    /// The normalization every translation surah carries
    /// (`translationNormalization` in the web worker).
    pub fn translation(source_id: &str, surah: u16) -> Self {
        Self {
            surah,
            source_id: source_id.to_owned(),
            script: QuranScript::Translation,
            source_profile: format!("translation:{source_id}"),
            packaging: OpenerPackaging::Absent,
            opener_kind: OpenerKind::None,
            opener_text: None,
            opener_end_scalar: 0,
            body_start_scalar: 0,
        }
    }
}

/// One verse row, text verbatim from the database (`Ayah`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ayah {
    /// `"{surah}:{ayah}"`.
    pub key: String,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    pub text: String,
}

/// A contiguous run of verses plus the normalization of every surah in it
/// (`QuranRangeText`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RangeText {
    pub ayahs: Vec<Ayah>,
    pub normalizations: Vec<SurahNormalization>,
}

/// One whole surah of one source (`QuranSurahText`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurahText {
    pub source_id: String,
    pub script: QuranScript,
    pub verses: Vec<String>,
    pub normalization: SurahNormalization,
}

/// `"{surah}:{ayah}"` (`verseKey`).
pub fn verse_key(surah: u16, ayah: u16) -> String {
    format!("{surah}:{ayah}")
}

/// Parses `"{surah}:{ayah}"`; `None` for anything else (the web's
/// `parseKey` returns a `{1,1}` sentinel instead; callers here decide).
pub fn parse_verse_key(key: &str) -> Option<(u16, u16)> {
    let (s, a) = key.split_once(':')?;
    if s.is_empty() || a.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !a.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((s.parse().ok()?, a.parse().ok()?))
}

/// Arabic-Indic digits (`toArabicDigits`).
pub fn to_arabic_digits(n: impl ToString) -> String {
    map_digits(&n.to_string(), '\u{0660}')
}

/// Extended Arabic-Indic digits, as IndoPak mushafs print them
/// (`toEasternDigits`).
pub fn to_eastern_digits(n: impl ToString) -> String {
    map_digits(&n.to_string(), '\u{06F0}')
}

fn map_digits(s: &str, zero: char) -> String {
    s.chars()
        .map(|c| match c.to_digit(10) {
            Some(d) => char::from_u32(zero as u32 + d).unwrap_or(c),
            None => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verse_keys_round_trip_and_reject_malformed() {
        assert_eq!(verse_key(2, 255), "2:255");
        assert_eq!(parse_verse_key("2:255"), Some((2, 255)));
        for bad in ["not-a-key", "2", "2:", ":5", "a:1"] {
            assert_eq!(parse_verse_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn digits() {
        assert_eq!(to_arabic_digits(0), "٠");
        assert_eq!(to_arabic_digits(123), "١٢٣");
        assert_eq!(to_eastern_digits(101), "۱۰۱");
    }

    #[test]
    fn source_ids_round_trip() {
        for s in ArabicSource::ALL {
            assert_eq!(ArabicSource::from_id(s.id()), Some(s));
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.id())
            );
        }
        assert_eq!(
            SourceRef::from_id("en.sahih"),
            SourceRef::Translation("en.sahih".into())
        );
    }
}
