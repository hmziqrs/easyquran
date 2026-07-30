//! Quran API wire types (§6.3).
//!
//! Serialization is a deliberate divergence from the rest of the codebase:
//! these are the API's first **camelCase** success bodies (structs use
//! `rename_all = "camelCase"`, enums use `rename_all = "kebab-case"`). The
//! baseline modules serialize snake_case; see `docs/quran-api.md` §6.3.
//!
//! `content_version` / `script` live on the envelope / range, never on each
//! verse — per-verse copies cost hundreds of redundant allocations (§6.3).
//!
//! `derive(utoipa::ToSchema)` / `derive(utoipa::IntoParams)` are gated on the
//! `openapi` feature (Phase 1c, off by default).

use serde::{Deserialize, Serialize};

use crate::quran::{Bismillah, Place, SajdaKind, Script};

/// The single success envelope (§6.3). A client caching one ayah can still tell
/// which content version it holds. Readiness and the OpenAPI document are the
/// operational exceptions (not enveloped).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T: Serialize> {
    pub data: T,
    pub content_version: String,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(data: T, content_version: impl Into<String>) -> Self {
        Self {
            data,
            content_version: content_version.into(),
        }
    }
}

// ── enums (kebab-case wire values, §6.3) ────────────────────────────────────

/// Which navigation family a range belongs to (§6.3). `global` is the
/// `fromGlobal`/`toGlobal` range (index is `None`).
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

// ── shared shapes ───────────────────────────────────────────────────────────

/// A verse identifier (`{surah, ayah}`, both 1-based).
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

/// Metadata describing a served ayah window (§6.3). `script` is constant across
/// a response, so it lives here, not on each `Ayah`.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RangeMeta {
    pub kind: RangeKind,
    /// `None` only for `kind = global`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u16>,
    /// hizb-quarter only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hizb: Option<u8>,
    /// hizb-quarter only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarter_in_hizb: Option<u8>,
    pub start_global: u32,
    pub end_global: u32,
    pub first: VerseKey,
    pub last: VerseKey,
    /// Ayahs returned in this response.
    pub count: u32,
    /// Ayahs in the unit, before `from`/`to`/`cursor` (§6.1).
    pub total: u32,
    pub script: Script,
    /// Present when more ayahs remain beyond this page (§6.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<u32>,
}

/// One ayah (§6.3). `text` is the exact selected-source text — verbatim (§3.3).
/// `sajda` is omitted (never null) when absent.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Ayah {
    /// `"{surah}:{ayah}"`.
    pub key: String,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sajda: Option<SajdaKind>,
    /// Navigation position, free from the in-memory ranges (§6.3).
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

/// A loose list of ayahs (`/ayahs?keys=…`, order preserved) — no range meta.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AyahsList {
    pub ayahs: Vec<Ayah>,
}

/// Surah metadata (§6.1). `ayahCount` (not `ayas`) on the wire.
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

/// One navigation-range summary (the `GET /{family}` and `GET /{family}/{n}`
/// bodies, without ayahs).
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
    /// Ayah count in the unit.
    pub total: u32,
}

/// A sajda marker (§6.1).
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

/// One downloadable Arabic database artifact (§5.1).
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Artifact {
    pub id: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub download_url: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ScriptsData {
    pub scripts: Vec<Artifact>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SourceDigestsDto {
    pub uthmani: String,
    pub simple_clean: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TranslationVersion {
    pub id: String,
    pub content_version: String,
}

/// `GET /version` body (§8.1) — every version axis.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct VersionData {
    pub api_version: String,
    pub search_version: String,
    pub source_digests: SourceDigestsDto,
    /// Empty for the Arabic MVP (translations are future, §1/§6.2).
    pub translations: Vec<TranslationVersion>,
}

/// `GET /quran/v1/health/ready` (§8.4). Operational — not enveloped.
/// `Cache-Control: no-store`.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct HealthReady {
    pub ready: bool,
    pub content_version: String,
    pub search_version: String,
    pub source_digests: SourceDigestsDto,
    pub verse_count: u32,
    pub surah_count: u16,
}

/// `GET /random` body (§8.5) — the resolved UTC date is echoed so a client can
/// detect a stale cache.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RandomAyah {
    pub date: String,
    pub ayah: Ayah,
}

// ── search (Phase 2, §7.1) ──────────────────────────────────────────────────

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

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SearchHit {
    pub ayah: Ayah,
    /// UTF-16 code-unit offsets into `ayah.text`, for a JavaScript consumer.
    pub highlights: Vec<Highlight>,
}

#[derive(Serialize, Debug)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Highlight {
    pub start: u32,
    pub end: u32,
}

// ── query structs (§6.1: `deny_unknown_fields` everywhere) ──────────────────

/// Empty query — rejects any parameter on routes that take none.
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct NoQuery {}

/// `script` + within-unit `from`/`to` + pagination, for every `…/ayahs` route.
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

/// Single-ayah routes (`/ayahs/{surah}/{ayah}`): just `script`.
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct ScriptQuery {
    #[serde(default)]
    pub script: Option<String>,
}

/// `/ayahs` accepts EITHER `keys` OR `fromGlobal`/`toGlobal` (§6.1).
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

/// `/random?date=YYYY-MM-DD` (§8.5). `date` defaults to today (UTC).
#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
pub struct RandomQuery {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
}

/// `/search` (Phase 2, §7.1).
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
