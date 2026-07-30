//! In-memory Quran store types (Phase 0, §4.2).
//!
//! The store is **immutable**: it is built once at boot from the two Arabic
//! SQLite artifacts + the metadata XML ([`crate::quran::loader::load_quran_store`]),
//! held behind `Arc<QuranStore>` on `AppState`, and never mutated. Arabic request
//! handling performs no SQLite query (§4.1, §10) — every read is an in-memory
//! slice or a binary search over a fixed metadata table.
//!
//! External Quran numbers are one-based; array access always subtracts one
//! explicitly (`verse g` → `g - 1`, `sura n` → `n - 1`). See §4.2.

use std::sync::Arc;

/// Total ayah count. Global indices are contiguous `1..=VERSE_COUNT` (§3.1).
pub const VERSE_COUNT: u32 = 6236;
/// Surah count.
pub const SURA_COUNT: usize = 114;
/// Maximum ayahs a single content response may carry (§6.1).
pub const RESPONSE_CAP: u32 = 300;

/// Frozen label identifying the shared Arabic normalization rule set (§7.1,
/// §8.1). Bumped only when normalization semantics change; doing so invalidates
/// search ETags and requires matching backend + web code. It does **not** alter
/// or require rebuilding the existing SQLite files.
pub const SEARCH_VERSION: &str = "arabic-search-v1";

// ── Enums (wire casing is applied in the DTO layer, §6.3) ───────────────────

/// Arabic response script. Uthmani is the default; simple-clean is the
/// alternate response script and the Arabic search corpus (§4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Script {
    Uthmani,
    SimpleClean,
}

impl Script {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Uthmani => "uthmani",
            Self::SimpleClean => "simple-clean",
        }
    }

    /// Parse the `script=` query value (§6.1). Returns `None` for any value
    /// other than `uthmani` / `simple-clean`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "uthmani" => Some(Self::Uthmani),
            "simple-clean" => Some(Self::SimpleClean),
            _ => None,
        }
    }
}

/// Revelation place (§6.3: serializes `meccan` / `medinan`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Place {
    Meccan,
    Medinan,
}

impl Place {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Meccan" => Some(Self::Meccan),
            "Medinan" => Some(Self::Medinan),
            _ => None,
        }
    }
}

/// Per-surah basmala case (§3.3). Descriptive metadata only — never an
/// instruction to alter text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Bismillah {
    /// Ayah 1 *is* the basmala (surah 1 only).
    FirstAyah,
    /// No basmala (surah 9 only).
    None,
    /// Ayah 1 begins `basmala + " "` (112 surahs).
    EmbeddedPrefix,
}

/// Prostration kind (§6.3: serializes `recommended` / `obligatory`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum SajdaKind {
    Recommended,
    Obligatory,
}

impl SajdaKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "recommended" => Some(Self::Recommended),
            "obligatory" => Some(Self::Obligatory),
            _ => None,
        }
    }
}

// ── Per-script text arena (§4.2) ────────────────────────────────────────────

/// Per-script contiguous text arena with a per-verse offset table. Verse `g`
/// (1-based global index) occupies `arena[offsets[g-1]..offsets[g]]` (byte
/// offsets; every text boundary is a UTF-8 char boundary).
///
/// Stored as arenas — not one boxed string per verse per script — for scan
/// locality and to collapse 18,708 allocations down to 6 (§4.2).
#[derive(Clone)]
pub struct Corpus {
    arena: Box<str>,
    /// Length `VERSE_COUNT + 1` (`6237`); `offsets[0] == 0`.
    offsets: Box<[u32]>,
}

impl Corpus {
    /// Build a corpus from rows already in global-index order (1..=6236).
    /// `offsets` is allocated here; the arena is the texts concatenated with no
    /// separator (boundaries are encoded in `offsets`).
    pub(crate) fn from_texts(texts: &[&str]) -> Self {
        let total = texts.iter().map(|t| t.len()).sum();
        let mut buf = String::with_capacity(total);
        let mut offsets = Vec::with_capacity(texts.len() + 1);
        offsets.push(0u32);
        for t in texts {
            buf.push_str(t);
            offsets.push(buf.len() as u32);
        }
        Self {
            arena: buf.into_boxed_str(),
            offsets: offsets.into_boxed_slice(),
        }
    }

    /// Verse text for global index `g` (1-based). Returns `None` out of range.
    #[inline]
    pub fn verse(&self, g: u32) -> Option<&str> {
        if !(1..=VERSE_COUNT).contains(&g) {
            return None;
        }
        let i = g as usize;
        let start = self.offsets[i - 1] as usize;
        let end = self.offsets[i] as usize;
        Some(&self.arena[start..end])
    }

    /// All verses, joined by `\n` in global-index order — exactly the
    /// representation the §3.3 golden digest hashes (no trailing newline).
    pub fn joined_for_digest(&self) -> String {
        let mut out =
            String::with_capacity(self.arena.len() + VERSE_COUNT as usize);
        for g in 1..=VERSE_COUNT {
            if g > 1 {
                out.push('\n');
            }
            out.push_str(self.verse(g).expect("verse in range"));
        }
        out
    }
}

// ── Metadata (§4.2) ─────────────────────────────────────────────────────────

/// Surah metadata (§4.2). Fixed 114 entries.
#[derive(Clone, Debug)]
pub struct SuraMeta {
    pub index: u16, // 1..=114
    pub ayas: u16,
    /// Global index of this surah's first ayah (1-based). Equals XML `start` + 1
    /// (§3.1: `start` is zero-based). So `global_of(sura, aya) == start_global - 1 + aya`.
    pub start_global: u32,
    /// Global index of this surah's last ayah (`start_global - 1 + ayas`).
    pub end_global: u32,
    pub revelation_order: u16,
    pub ruku_count: u16,
    pub place: Place,
    pub name_arabic: Box<str>,
    pub name_translit: Box<str>,
    pub name_english: Box<str>,
    pub bismillah: Bismillah,
}

impl SuraMeta {
    /// `global_of(sura, aya)` for a valid ayah within this surah (§3.1).
    #[inline]
    pub fn global_of(&self, aya: u16) -> Option<u32> {
        if !(1..=self.ayas).contains(&aya) {
            return None;
        }
        // start_global - 1 == zero-based XML `start`; +aya (1-based) == global.
        Some(self.start_global - 1 + aya as u32)
    }
}

/// Zero-sized family marker so `pages[juz - 1]` is a compile error (§4.2).
pub struct Juz;
pub struct Page;
pub struct Ruku;
pub struct HizbQuarter;
pub struct Manzil;

/// A tiled navigation range for one of the five start-marker families
/// (juz / page / ruku / hizb-quarter / manzil). One range builder covers all
/// five (§3.2): `start_global` is the marker's global index; `end_global` is
/// the next marker's start minus one; the last end is `VERSE_COUNT`.
pub struct Range<K> {
    pub index: u16,
    pub start_global: u32,
    pub end_global: u32,
    pub start_sura: u16,
    pub start_aya: u16,
    pub end_sura: u16,
    pub end_aya: u16,
    pub _family: std::marker::PhantomData<K>,
}

impl<K> Range<K> {
    /// Ayah count in the unit (inclusive span).
    #[inline]
    pub fn count(&self) -> u32 {
        self.end_global - self.start_global + 1
    }
}

impl<K> std::fmt::Debug for Range<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Range")
            .field("index", &self.index)
            .field("start_global", &self.start_global)
            .field("end_global", &self.end_global)
            .field("start_sura", &self.start_sura)
            .field("start_aya", &self.start_aya)
            .field("end_sura", &self.end_sura)
            .field("end_aya", &self.end_aya)
            .finish()
    }
}

/// A sajda marker (§3.2, 15 total).
#[derive(Clone, Debug)]
pub struct Sajda {
    pub index: u16,
    pub sura: u16,
    pub aya: u16,
    pub global_index: u32,
    pub kind: SajdaKind,
}

/// Per-source corpus-text digests (§3.3) — `sha256` of all 6,236 texts joined
/// by `\n` in global-index order, no trailing newline. Asserted at startup and
/// surfaced by `/version` and `/health/ready` so an operator can diff the image
/// against the bucket.
#[derive(Clone, Debug)]
pub struct SourceDigests {
    pub uthmani: Box<str>,
    pub simple_clean: Box<str>,
}

/// File-level artifact metadata for `/scripts` (§5.1): the `sha256` + size of
/// the published SQLite file (distinct from the §3.3 corpus-text digest).
/// Computed once at load; `/scripts` derives the download URL from
/// object-storage settings and HEAD-verifies it, but never re-reads the file.
#[derive(Clone, Debug)]
pub struct ArtifactFile {
    pub id: Script,
    pub size_bytes: u64,
    pub sha256: Box<str>,
}

#[derive(Clone, Debug)]
pub struct Artifacts {
    pub uthmani: ArtifactFile,
    pub simple_clean: ArtifactFile,
}

/// All Quran metadata (§4.2). `suras` is a fixed inline array (114 × ~88 B ≈
/// 10 KB); the per-family ranges and sajdas are boxed.
pub struct QuranMeta {
    pub suras: [SuraMeta; SURA_COUNT],
    pub juzs: Box<[Range<Juz>]>,
    pub pages: Box<[Range<Page>]>,
    pub rukus: Box<[Range<Ruku>]>,
    pub hizb_quarters: Box<[Range<HizbQuarter>]>,
    pub manzils: Box<[Range<Manzil>]>,
    pub sajdas: Box<[Sajda]>,
}

impl QuranMeta {
    /// Surah `n` (1-based). `None` outside `1..=114`.
    #[inline]
    pub fn sura(&self, n: u16) -> Option<&SuraMeta> {
        if !(1..=SURA_COUNT as u16).contains(&n) {
            return None;
        }
        Some(&self.suras[(n - 1) as usize])
    }

    /// All suras, in order.
    #[inline]
    pub fn suras(&self) -> &[SuraMeta] {
        &self.suras
    }

    /// `global_of(sura, aya)` (§3.1). `None` if the surah or ayah is out of range.
    pub fn global_of(&self, sura: u16, aya: u16) -> Option<u32> {
        self.sura(sura)?.global_of(aya)
    }

    /// Inverse of `global_of`: the `(sura, aya)` for global index `g` (§3.1).
    /// Linear scan over 114 entries (trivial). `None` if `g` is out of range.
    pub fn locate(&self, g: u32) -> Option<(u16, u16)> {
        if !(1..=VERSE_COUNT).contains(&g) {
            return None;
        }
        // Find the surah whose [start_global, end_global] contains g.
        let s = self
            .suras
            .iter()
            .find(|s| g >= s.start_global && g <= s.end_global)?;
        let aya = (g - (s.start_global - 1)) as u16;
        Some((s.index, aya))
    }
}

/// Find the range in `ranges` whose inclusive `[start_global, end_global]`
/// contains `g`. Ranges are ascending by `start_global` with no gaps or
/// overlaps (§3.2), so this is a partition-point + bound check.
pub fn range_containing<K>(ranges: &[Range<K>], g: u32) -> Option<&Range<K>> {
    // Number of ranges starting at or before g.
    let idx = ranges
        .partition_point(|r| r.start_global <= g)
        .checked_sub(1)?;
    let r = ranges.get(idx)?;
    if g <= r.end_global {
        Some(r)
    } else {
        None
    }
}

// ── The store (§4.2) ────────────────────────────────────────────────────────

/// The immutable in-memory Quran store. Holds content only — artifact URLs are
/// derived from object-storage settings and joined into `/scripts` by its
/// handler, so a CDN hostname change is not a Quran content change (§4.2).
///
/// `search` (the normalized `SearchIndex`, §4.2/§7.1) is added in Phase 2.
pub struct QuranStore {
    pub uthmani: Corpus,
    pub simple_clean: Corpus,
    pub meta: QuranMeta,
    pub content_version: Arc<str>,
    pub source_digests: SourceDigests,
    pub artifacts: Artifacts,
    /// Normalized search index over simple-clean (§7.1). Built at boot from the
    /// in-memory corpora — no SQLite.
    pub search: super::search::SearchIndex,
}

impl QuranStore {
    /// Verse text in `script` for global index `g` (1-based). `None` if `g` is
    /// out of range. This is the exact loaded source text — verbatim, never
    /// normalized (§3.3).
    #[inline]
    pub fn verse(&self, script: Script, g: u32) -> Option<&str> {
        match script {
            Script::Uthmani => self.uthmani.verse(g),
            Script::SimpleClean => self.simple_clean.verse(g),
        }
    }

    #[inline]
    pub fn meta(&self) -> &QuranMeta {
        &self.meta
    }

    #[inline]
    pub fn content_version(&self) -> &str {
        &self.content_version
    }

    #[inline]
    pub fn source_digests(&self) -> &SourceDigests {
        &self.source_digests
    }

    /// Prostration kind at global index `g`, if any (15 sajdas total).
    pub fn sajda_at(&self, g: u32) -> Option<SajdaKind> {
        self.meta
            .sajdas
            .iter()
            .find(|s| s.global_index == g)
            .map(|s| s.kind)
    }

    /// Navigation position of global index `g` (the `juz`/`page`/`ruku`/
    /// `hizb_quarter`/`manzil` fields on `Ayah`, §6.3). Free from the
    /// in-memory ranges — no SQLite query.
    pub fn navigation(&self, g: u32) -> Option<Navigation> {
        if !(1..=VERSE_COUNT).contains(&g) {
            return None;
        }
        let juz = range_containing(&self.meta.juzs, g)?.index;
        let page = range_containing(&self.meta.pages, g)?.index;
        let ruku = range_containing(&self.meta.rukus, g)?.index;
        let quarter = range_containing(&self.meta.hizb_quarters, g)?.index;
        // Manzil indices are 1..=7 (validated at load), so they fit `u8`.
        let manzil = u8::try_from(range_containing(&self.meta.manzils, g)?.index)
            .expect("manzil index is 1..=7, fits u8");
        Some(Navigation {
            juz,
            page,
            ruku,
            hizb_quarter: quarter,
            manzil,
        })
    }

    /// Everything the `Ayah` DTO needs for global index `g`, in `script`.
    pub fn ayah_view(&self, script: Script, g: u32) -> Option<AyahView<'_>> {
        let (surah, ayah) = self.meta.locate(g)?;
        let text = self.verse(script, g)?;
        let sajda = self.sajda_at(g);
        let nav = self.navigation(g)?;
        Some(AyahView {
            global_index: g,
            surah,
            ayah,
            text,
            sajda,
            juz: nav.juz,
            page: nav.page,
            ruku: nav.ruku,
            hizb_quarter: nav.hizb_quarter,
            manzil: nav.manzil,
        })
    }
}

/// Navigation position of one ayah (§6.3 `Ayah` navigation fields).
#[derive(Clone, Copy, Debug)]
pub struct Navigation {
    pub juz: u16,
    pub page: u16,
    pub ruku: u16,
    pub hizb_quarter: u16,
    pub manzil: u8,
}

/// Borrowed view backing one `Ayah` DTO (§6.3).
pub struct AyahView<'a> {
    pub global_index: u32,
    pub surah: u16,
    pub ayah: u16,
    pub text: &'a str,
    pub sajda: Option<SajdaKind>,
    pub juz: u16,
    pub page: u16,
    pub ruku: u16,
    pub hizb_quarter: u16,
    pub manzil: u8,
}
