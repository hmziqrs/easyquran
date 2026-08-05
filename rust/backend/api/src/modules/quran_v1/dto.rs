use serde::{Deserialize, Serialize};

use crate::quran::{Bismillah, Place, SajdaKind, Script, SurahNormalizationDto};

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[derive(Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum RangeKind {
    Surah,
    Juz,
    Page,
    Ruku,
    HizbQuarter,
    Manzil,
    Global,
}

#[derive(Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct VerseKey {
    pub surah: u16,
    pub ayah: u16,
}

impl VerseKey {
    pub fn new(surah: u16, ayah: u16) -> Self {
        Self { surah, ayah }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RangeMeta {
    pub kind: RangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hizb: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarter_in_hizb: Option<u8>,
    pub start_global: u32,
    pub end_global: u32,
    pub first: VerseKey,
    pub last: VerseKey,
    pub count: u32,
    pub total: u32,
    pub script: Script,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<u32>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Ayah {
    pub key: String,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sajda: Option<SajdaKind>,
    pub juz: u16,
    pub page: u16,
    pub ruku: u16,
    pub hizb_quarter: u16,
    pub manzil: u8,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AyahRange {
    pub range: RangeMeta,
    pub ayahs: Vec<Ayah>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AyahsList {
    pub ayahs: Vec<Ayah>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct LeanAyah {
    pub key: String,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    pub text: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RangeText {
    pub ayahs: Vec<LeanAyah>,
    pub normalizations: Vec<SurahNormalizationDto>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SuraDto {
    pub index: u16,
    pub ayah_count: u16,
    pub start_global: u32,
    pub end_global: u32,
    pub revelation_order: u16,
    pub ruku_count: u16,
    pub place: Place,
    pub name_arabic: String,
    pub name_translit: String,
    pub name_english: String,
    pub bismillah: Bismillah,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RangeSummary {
    pub kind: RangeKind,
    pub index: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hizb: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarter_in_hizb: Option<u8>,
    pub start_global: u32,
    pub end_global: u32,
    pub first: VerseKey,
    pub last: VerseKey,
    pub total: u32,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SajdaDto {
    pub index: u16,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    pub kind: SajdaKind,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Artifact {
    pub id: String,
    pub size_bytes: u64,
    pub download_url: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ScriptsData {
    pub scripts: Vec<Artifact>,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum SourceKind {
    Arabic,
    Translation,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SourceDto {
    pub id: String,
    pub kind: SourceKind,
    pub language: String,
    pub language_code: String,
    pub direction: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translator: Option<String>,
    pub size_bytes: u64,
    pub download_url: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SourcesData {
    pub sources: Vec<SourceDto>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct HealthReady {
    pub ready: bool,
    pub verse_count: u32,
    pub surah_count: u16,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RandomAyah {
    pub date: String,
    pub ayah: Ayah,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SearchResponse {
    pub query: String,
    pub total: u32,
    pub limit: u16,
    pub offset: u32,
    pub results: Vec<SearchHit>,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum SearchHitKind {
    Ayah,
    Opener,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SearchHit {
    pub kind: SearchHitKind,
    pub ayah: Ayah,
    /// Offsets are UTF-16 code units for JS consumers — not byte or char indices.
    pub highlights: Vec<Highlight>,
}

#[derive(Serialize, Debug)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Highlight {
    pub start: u32,
    pub end: u32,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct NoQuery {}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct RangeAyahsQuery {
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub from: Option<u16>,
    #[serde(default)]
    pub to: Option<u16>,
    #[serde(default)]
    pub cursor: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct ScriptQuery {
    #[serde(default)]
    pub script: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct RangeTextQuery {
    #[serde(default)]
    pub from: Option<u32>,
    #[serde(default)]
    pub to: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct AyahsQuery {
    #[serde(default)]
    pub keys: Option<String>,
    #[serde(default, rename = "fromGlobal")]
    pub from_global: Option<u32>,
    #[serde(default, rename = "toGlobal")]
    pub to_global: Option<u32>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub cursor: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct RandomQuery {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub limit: Option<u16>,
    #[serde(default)]
    pub offset: Option<u32>,
}
