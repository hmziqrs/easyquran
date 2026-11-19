//! The baked artifact catalogue (PARITY B1, B2): the five Arabic source
//! profiles and the 378 translation sources, each an immutable SQLite file
//! identified by its **id** (never a hash). Bytes, size and path come from
//! this baked map, never from remote metadata.
//!
//! Single source: the translation rows, flags and autonyms are read from the
//! web's own files (`include_str!`, parsed once). The Arabic profiles are
//! TS-only constants in `web/src/lib/quran/view/source-profiles.ts`, so they
//! are transcribed here; a test greps that file for every id, path and size
//! so drift fails the build's tests.

use std::{collections::HashMap, sync::OnceLock};

use serde::{Deserialize, Serialize};

use super::types::{ArabicSource, OpenerPackaging, QuranScript, SourceRef};

/// The baked translation rows (`[id, language, languageCode, direction,
/// name, translator, filePath, sizeBytes]`).
const TRANSLATIONS_JSON: &str = include_str!("../../../../../web/src/lib/data/translations.json");
/// The web catalogue module: flags and autonyms are parsed out of it.
const CATALOGUE_TS: &str = include_str!("../../../../../web/src/lib/quran/catalogue.ts");

/// Where translation artifacts live under the artifact base
/// (`TRANSLATION_ARTIFACT_PREFIX`).
pub const TRANSLATION_ARTIFACT_PREFIX: &str = "tanzil/translations/";

/// Every artifact has exactly this many verse rows (`QURAN_ROW_COUNT`).
pub const QURAN_ROW_COUNT: u32 = 6236;

/// Which family an artifact belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Arabic,
    Translation,
}

/// One downloadable artifact: id, kind, its path under the artifact base,
/// and its exact size.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Artifact {
    pub id: String,
    pub kind: ArtifactKind,
    /// Relative to the artifact base (`tanzil/arabic/quran-uthmani.sqlite`).
    pub path: String,
    pub size_bytes: u64,
}

/// One Arabic source profile (`QuranSourceProfile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArabicProfile {
    pub source: ArabicSource,
    /// The profile id (`tanzil-uthmani-581cc540`): wire `sourceProfile`.
    pub profile_id: &'static str,
    /// Path inside the monorepo (`db/quran/arabic/...`).
    pub repository_path: &'static str,
    /// Path under the artifact base.
    pub r2_path: &'static str,
    pub size_bytes: u64,
    pub canonical_row_count: u32,
    /// The variant adapter: separate-row openers taken from the DB's own 1:1
    /// (`VARIANT_QURAN_DATABASE`); `false` is the Tanzil adapter.
    pub variant: bool,
    pub reference_opener_surah: u16,
}

impl ArabicProfile {
    /// The script.
    pub fn script(&self) -> QuranScript {
        self.source.script()
    }

    /// How surah `surah` (1..=114) packages its opener
    /// (`TANZIL_PACKAGING` / `VARIANT_PACKAGING`).
    pub fn packaging(&self, surah: u16) -> OpenerPackaging {
        if surah == 1 {
            return OpenerPackaging::NumberedAyah;
        }
        if surah == 9 || surah == 0 {
            return OpenerPackaging::Absent;
        }
        if self.variant {
            OpenerPackaging::SeparateRow
        } else {
            OpenerPackaging::EmbeddedPrefix
        }
    }

    /// The artifact.
    pub fn artifact(&self) -> Artifact {
        Artifact {
            id: self.source.id().to_owned(),
            kind: ArtifactKind::Arabic,
            path: self.r2_path.to_owned(),
            size_bytes: self.size_bytes,
        }
    }
}

/// The five profiles, in the web's registry order.
pub const ARABIC_PROFILES: [ArabicProfile; 5] = [
    ArabicProfile {
        source: ArabicSource::Uthmani,
        profile_id: "tanzil-uthmani-581cc540",
        repository_path: "db/quran/arabic/quran-uthmani.sqlite",
        r2_path: "tanzil/arabic/quran-uthmani.sqlite",
        size_bytes: 1_593_344,
        canonical_row_count: 6236,
        variant: false,
        reference_opener_surah: 1,
    },
    ArabicProfile {
        source: ArabicSource::AnnotatedUthmani,
        profile_id: "uthmani-annotated-qcom-words-v1",
        repository_path: "db/quran/arabic/quran-uthmani-annotated.sqlite",
        r2_path: "tanzil/arabic/quran-uthmani-annotated.sqlite",
        size_bytes: 1_605_632,
        canonical_row_count: 6236,
        variant: true,
        reference_opener_surah: 1,
    },
    ArabicProfile {
        source: ArabicSource::SimpleClean,
        profile_id: "tanzil-simple-clean-a0c52760",
        repository_path: "db/quran/arabic/quran-simple-clean.sqlite",
        r2_path: "tanzil/arabic/quran-simple-clean.sqlite",
        size_bytes: 929_792,
        canonical_row_count: 6236,
        variant: false,
        reference_opener_surah: 1,
    },
    ArabicProfile {
        source: ArabicSource::IndoPak,
        profile_id: "indopak-naveed-7d3c21e0",
        repository_path: "db/quran/arabic/quran-indopak.sqlite",
        r2_path: "tanzil/arabic/quran-indopak.sqlite",
        size_bytes: 1_634_304,
        canonical_row_count: 6236,
        variant: true,
        reference_opener_surah: 1,
    },
    ArabicProfile {
        source: ArabicSource::Tajweed,
        profile_id: "tajweed-daralislam-5b9f48d2",
        repository_path: "db/quran/arabic/quran-tajweed.sqlite",
        r2_path: "tanzil/arabic/quran-tajweed.sqlite",
        size_bytes: 2_015_232,
        canonical_row_count: 6236,
        variant: true,
        reference_opener_surah: 1,
    },
];

/// The profile of `source`.
pub fn arabic_profile(source: ArabicSource) -> &'static ArabicProfile {
    ARABIC_PROFILES
        .iter()
        .find(|p| p.source == source)
        .expect("every ArabicSource has a profile")
}

/// The Arabic source the reader opens by default and the app bundles
/// (`DEFAULT_QURAN_SOURCE_PLAN.reader`).
pub const DEFAULT_READER_SOURCE: ArabicSource = ArabicSource::AnnotatedUthmani;

/// Text direction of a translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Ltr,
    Rtl,
}

/// One translation source (`BakedTranslationMetadata`): the attribution the
/// web shows (name, translator, language) plus its artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationEntry {
    pub id: String,
    pub language: String,
    pub language_code: String,
    pub direction: Direction,
    /// Catalogue short name (the lane label).
    pub name: String,
    pub translator: Option<String>,
    /// Relative to the translations root (`sqlite/en.sahih.sqlite`).
    pub file_path: String,
    pub size_bytes: u64,
}

impl TranslationEntry {
    /// Path under the artifact base (`tanzil/translations/<filePath>`).
    pub fn artifact_path(&self) -> String {
        format!("{TRANSLATION_ARTIFACT_PREFIX}{}", self.file_path)
    }

    /// The artifact.
    pub fn artifact(&self) -> Artifact {
        Artifact {
            id: self.id.clone(),
            kind: ArtifactKind::Translation,
            path: self.artifact_path(),
            size_bytes: self.size_bytes,
        }
    }

    /// Where the id says the work came from.
    pub fn provenance(&self) -> Provenance {
        provenance_of(&self.id)
    }

    /// Commentary (tafsir), not a reading translation.
    pub fn is_tafsir(&self) -> bool {
        super::view::text_marks::is_tafsir_translation(&self.id)
    }
}

/// Provenance of a translation (`translationSourceOf`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provenance {
    Qul,
    QuranEnc,
    Tanzil,
}

/// Provenance by id prefix.
pub fn provenance_of(id: &str) -> Provenance {
    if id.starts_with("qul.") {
        return Provenance::Qul;
    }
    if id.starts_with("quranenc.") {
        return Provenance::QuranEnc;
    }
    Provenance::Tanzil
}

/// Flag + country for a language code (`LanguageFlag`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageFlag {
    pub flag: String,
    /// Feeds search only; empty for stateless languages.
    pub country: String,
}

struct Catalogue {
    translations: Vec<TranslationEntry>,
    by_id: HashMap<String, usize>,
    flags: HashMap<String, LanguageFlag>,
    autonyms: HashMap<String, Option<String>>,
}

fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let translations = decode_translations(TRANSLATIONS_JSON)
            .unwrap_or_else(|e| panic!("[translations] baked catalogue invalid: {e}"));
        let mut by_id = HashMap::with_capacity(translations.len());
        for (i, entry) in translations.iter().enumerate() {
            if by_id.insert(entry.id.clone(), i).is_some() {
                panic!("[translations] duplicate id: {}", entry.id);
            }
        }
        Catalogue {
            translations,
            by_id,
            flags: parse_flags(CATALOGUE_TS),
            autonyms: parse_autonyms(CATALOGUE_TS),
        }
    })
}

/// Decodes the tuple rows exactly as `translations.ts` validates them.
fn decode_translations(json: &str) -> Result<Vec<TranslationEntry>, String> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    rows.iter()
        .enumerate()
        .map(|(i, row)| decode_row(row, i + 1))
        .collect()
}

fn decode_row(row: &serde_json::Value, n: usize) -> Result<TranslationEntry, String> {
    let fields = row
        .as_array()
        .filter(|a| a.len() == 8)
        .ok_or(format!("row {n}: expected 8 fields"))?;
    let text = |i: usize, label: &str| -> Result<String, String> {
        fields[i]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or(format!("row {n}: bad {label}"))
    };
    let direction = match fields[3].as_str() {
        Some("ltr") => Direction::Ltr,
        Some("rtl") => Direction::Rtl,
        _ => return Err(format!("row {n}: bad direction")),
    };
    let translator = match &fields[5] {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s.clone()),
        _ => return Err(format!("row {n}: bad translator")),
    };
    let size_bytes = fields[7]
        .as_u64()
        .filter(|s| *s > 0)
        .ok_or(format!("row {n}: bad sizeBytes"))?;
    Ok(TranslationEntry {
        id: text(0, "id")?,
        language: text(1, "language")?,
        language_code: text(2, "languageCode")?,
        direction,
        name: text(4, "name")?,
        translator,
        file_path: text(6, "filePath")?,
        size_bytes,
    })
}

/// Every translation source, in the baked order.
pub fn translations() -> &'static [TranslationEntry] {
    &catalogue().translations
}

/// The translation with id `id`.
pub fn translation(id: &str) -> Option<&'static TranslationEntry> {
    let c = catalogue();
    c.by_id.get(id).map(|&i| &c.translations[i])
}

/// The artifact of any source id (Arabic or translation); `None` when the
/// id is unknown to this build (listed as unavailable, never guessed).
pub fn artifact(id: &str) -> Option<Artifact> {
    match SourceRef::from_id(id) {
        SourceRef::Arabic(source) => Some(arabic_profile(source).artifact()),
        SourceRef::Translation(id) => translation(&id).map(TranslationEntry::artifact),
    }
}

/// Every artifact: the five Arabic sources, then the translations.
pub fn all_artifacts() -> Vec<Artifact> {
    ARABIC_PROFILES
        .iter()
        .map(ArabicProfile::artifact)
        .chain(translations().iter().map(TranslationEntry::artifact))
        .collect()
}

const GLOBE: &str = "\u{1F310}";

/// Flag + country for a language code; the globe and no country for codes
/// without one (`flagFor`).
pub fn flag_for(language_code: &str) -> LanguageFlag {
    catalogue()
        .flags
        .get(language_code)
        .cloned()
        .unwrap_or_else(|| LanguageFlag {
            flag: GLOBE.to_owned(),
            country: String::new(),
        })
}

/// Native autonym of a language code; `None` when the English name is the
/// native one or the code is unknown (`nativeNameFor`).
pub fn native_name_for(language_code: &str) -> Option<&'static str> {
    catalogue()
        .autonyms
        .get(language_code)
        .and_then(|v| v.as_deref())
}

/// The catalogue name of a source id (`peekTranslationName`).
pub fn peek_translation_name(source_id: &str) -> Option<&'static str> {
    translation(source_id).map(|e| e.name.as_str())
}

/// The text of the TS object literal `const <name> ... = Object.freeze({ ... });`.
fn ts_block<'a>(ts: &'a str, name: &str) -> &'a str {
    let start = ts
        .find(&format!("export const {name}"))
        .unwrap_or_else(|| panic!("[catalogue] {name} missing from catalogue.ts"));
    let rest = &ts[start..];
    let open = rest.find("({").map_or(0, |i| i + 2);
    let close = rest.find("});").unwrap_or(rest.len());
    &rest[open..close]
}

/// Unescapes a TS string literal body: `\u{XXXX}`, `\uXXXX`, `\"`, `\\`.
fn unescape_ts(body: &str) -> String {
    let mut out = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = if chars.peek() == Some(&'{') {
                    chars.next();
                    chars.by_ref().take_while(|&h| h != '}').collect()
                } else {
                    chars.by_ref().take(4).collect()
                };
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn parse_flags(ts: &str) -> HashMap<String, LanguageFlag> {
    let re = regex::Regex::new(
        r#"(?m)^\s*([a-z]+):\s*\{\s*flag:\s*(GLOBE|"([^"]*)"),\s*country:\s*"([^"]*)"\s*\}"#,
    )
    .expect("flag pattern");
    re.captures_iter(ts_block(ts, "LANGUAGE_FLAGS"))
        .map(|c| {
            let flag = match c.get(3) {
                Some(lit) => unescape_ts(lit.as_str()),
                None => GLOBE.to_owned(),
            };
            (
                c[1].to_owned(),
                LanguageFlag {
                    flag,
                    country: unescape_ts(&c[4]),
                },
            )
        })
        .collect()
}

fn parse_autonyms(ts: &str) -> HashMap<String, Option<String>> {
    let re =
        regex::Regex::new(r#"(?m)^\s*([a-z]+):\s*(null|"([^"]*)"),"#).expect("autonym pattern");
    re.captures_iter(ts_block(ts, "LANGUAGE_AUTONYMS"))
        .map(|c| (c[1].to_owned(), c.get(3).map(|m| unescape_ts(m.as_str()))))
        .collect()
}

/// Author identity: trim, lowercase, whitespace collapsed; `""` for no
/// translator (`normalizedTranslatorKey`).
pub fn normalized_translator_key(translator: Option<&str>) -> String {
    let Some(t) = translator else {
        return String::new();
    };
    t.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Grouping key: translator within a language; unnamed works never merge
/// (`translationAuthorKey`).
pub fn translation_author_key(entry: &TranslationEntry) -> String {
    let translator = normalized_translator_key(entry.translator.as_deref());
    if translator.is_empty() {
        return format!("{} #id:{}", entry.language_code, entry.id);
    }
    format!("{} {translator}", entry.language_code)
}

/// One author's variants in input order (`TranslationAuthorGroup`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorGroup<'a> {
    pub key: String,
    pub variants: Vec<&'a TranslationEntry>,
}

/// Groups by author within language, first-seen order
/// (`groupTranslationAuthors`).
pub fn group_translation_authors<'a>(
    entries: impl IntoIterator<Item = &'a TranslationEntry>,
) -> Vec<AuthorGroup<'a>> {
    let mut groups: Vec<AuthorGroup<'a>> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        let key = translation_author_key(entry);
        match index.get(&key) {
            Some(&i) => groups[i].variants.push(entry),
            None => {
                index.insert(key.clone(), groups.len());
                groups.push(AuthorGroup {
                    key,
                    variants: vec![entry],
                });
            }
        }
    }
    groups
}

/// Default-variant preference: originals before QUL mirrors
/// (`translationProvenanceRank`).
pub fn translation_provenance_rank(id: &str) -> u8 {
    match provenance_of(id) {
        Provenance::Qul => 2,
        Provenance::QuranEnc => 1,
        Provenance::Tanzil => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_PROFILES_TS: &str =
        include_str!("../../../../../web/src/lib/quran/view/source-profiles.ts");
    const QURAN_TYPES_TS: &str = include_str!("../../../../../web/src/lib/data/quran-types.ts");

    fn underscored(n: u64) -> String {
        let digits = n.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push('_');
            }
            out.push(c);
        }
        out
    }

    /// Drift guard: every transcribed profile constant must appear in the
    /// web's source-profiles.ts, inside that profile's own block.
    #[test]
    fn arabic_profiles_match_web_source_profiles() {
        let blocks: Vec<&str> = SOURCE_PROFILES_TS
            .split("defineSourceProfile({")
            .skip(1) // the text before the first call
            .collect();
        assert_eq!(blocks.len(), ARABIC_PROFILES.len(), "profile count drifted");
        for (profile, block) in ARABIC_PROFILES.iter().zip(blocks) {
            assert!(block.contains(&format!("id: \"{}\"", profile.profile_id)));
            assert!(block.contains(&format!("r2Path: \"{}\"", profile.r2_path)));
            assert!(block.contains(&format!("repositoryPath: \"{}\"", profile.repository_path)));
            assert!(
                block.contains(&format!("sizeBytes: {},", underscored(profile.size_bytes))),
                "size of {}",
                profile.profile_id
            );
            assert!(block.contains(&format!(
                "canonicalRowCount: {}",
                profile.canonical_row_count
            )));
            let adapter = if profile.variant {
                "VARIANT_QURAN_DATABASE"
            } else {
                "TANZIL_QURAN_DATABASE"
            };
            assert!(block.contains(&format!("database: {adapter}")));
            // sourceId: QuranSourceId.<Key> maps to the id in quran-types.ts.
            let key = block
                .split("sourceId: QuranSourceId.")
                .nth(1)
                .and_then(|r| r.split(',').next())
                .unwrap();
            assert!(
                QURAN_TYPES_TS.contains(&format!("{key}: \"{}\"", profile.source.id())),
                "{key} -> {}",
                profile.source.id()
            );
        }
    }

    #[test]
    fn translation_catalogue_decodes_every_row() {
        let all = translations();
        assert_eq!(all.len(), 378);
        let sahih = translation("en.sahih").unwrap();
        assert_eq!(sahih.language_code, "en");
        assert_eq!(
            sahih.artifact_path(),
            "tanzil/translations/sqlite/en.sahih.sqlite"
        );
        assert!(translation("does.not.exist").is_none());
        assert_eq!(all_artifacts().len(), 383);
        assert_eq!(artifact("uthmani").unwrap().size_bytes, 1_593_344);
    }

    #[test]
    fn flags_and_autonyms_parse_from_web_catalogue() {
        let c = catalogue();
        assert_eq!(c.flags.len(), 96);
        assert_eq!(c.autonyms.len(), 96);
        assert_eq!(flag_for("en").flag, "\u{1F1EC}\u{1F1E7}");
        assert_eq!(flag_for("en").country, "United Kingdom");
        assert_eq!(flag_for("ber").flag, GLOBE);
        assert_eq!(flag_for("zz").flag, GLOBE);
        assert_eq!(native_name_for("ur"), Some("اردو"));
        assert_eq!(native_name_for("en"), None);
        for t in translations() {
            assert!(
                c.flags.contains_key(&t.language_code),
                "{}",
                t.language_code
            );
        }
    }

    #[test]
    fn author_grouping_mirrors_web() {
        assert_eq!(
            normalized_translator_key(Some("  Muhammad  Hamidullah ")),
            "muhammad hamidullah"
        );
        assert_eq!(normalized_translator_key(None), "");
        assert_eq!(translation_provenance_rank("qul.r1.x"), 2);
        assert_eq!(translation_provenance_rank("quranenc.en.x"), 1);
        assert_eq!(translation_provenance_rank("en.sahih"), 0);
        let groups = group_translation_authors(translations());
        assert!(groups.len() < translations().len());
    }
}
