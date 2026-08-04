pub const VERSE_COUNT: u32 = 6236;
pub const SURA_COUNT: usize = 114;
pub const RESPONSE_CAP: u32 = 300;

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

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "uthmani" => Some(Self::Uthmani),
            "simple-clean" => Some(Self::SimpleClean),
            _ => None,
        }
    }
}

/// A translation source id (e.g. "en.sahih"). Constructed ONLY via [`TranslationId::parse`],
/// which whitelists against the boot-loaded catalogue — that membership check IS the
/// path-traversal guard: a raw user string carrying '/' or '..' can never match a real id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TranslationId(Box<str>);

impl TranslationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(s: &str, valid_ids: &std::collections::HashSet<String>) -> Option<Self> {
        if valid_ids.contains(s) {
            Some(Self(Box::from(s)))
        } else {
            None
        }
    }
}

/// Resolved source: an Arabic script (resident at boot) or a translation (on-demand pool).
#[derive(Clone, Debug)]
pub enum SourceId {
    Arabic(Script),
    Translation(TranslationId),
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Bismillah {
    FirstAyah,
    None,
    EmbeddedPrefix,
}

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

#[derive(Clone)]
pub struct Corpus {
    arena: Box<str>,
    offsets: Box<[u32]>,
}

impl Corpus {
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

    pub fn joined_for_digest(&self) -> String {
        let mut out = String::with_capacity(self.arena.len() + VERSE_COUNT as usize);
        for g in 1..=VERSE_COUNT {
            if g > 1 {
                out.push('\n');
            }
            out.push_str(self.verse(g).expect("verse in range"));
        }
        out
    }

    /// Resident byte cost: arena + offset table. Used by the translation-pool weigher.
    pub fn bytes(&self) -> usize {
        self.arena.len() + self.offsets.len() * 4
    }
}

#[derive(Clone, Debug)]
pub struct SuraMeta {
    pub index: u16,
    pub ayas: u16,
    pub start_global: u32,
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
    #[inline]
    pub fn global_of(&self, aya: u16) -> Option<u32> {
        if !(1..=self.ayas).contains(&aya) {
            return None;
        }
        Some(self.start_global - 1 + aya as u32)
    }
}

/// Zero-sized phantom markers: make `Range<Juz>` vs `Range<Page>` distinct types so cross-family indexing is a compile error, not a silent bug.
pub struct Juz;
pub struct Page;
pub struct Ruku;
pub struct HizbQuarter;
pub struct Manzil;

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

#[derive(Clone, Debug)]
pub struct Sajda {
    pub index: u16,
    pub sura: u16,
    pub aya: u16,
    pub global_index: u32,
    pub kind: SajdaKind,
}

#[derive(Clone, Debug)]
pub struct SourceDigests {
    pub uthmani: Box<str>,
    pub simple_clean: Box<str>,
}

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

/// One row of the boot-loaded translation catalogue (the widened §2 `index.min.json`).
/// Carries the sqlite digest + size so the API and the client never re-hash on the hot path.
#[derive(Clone, Debug)]
pub struct CatalogueEntry {
    pub id: Box<str>,
    pub language: Box<str>,
    pub language_code: Box<str>,
    pub direction: Box<str>,
    pub name: Box<str>,
    pub translator: Option<Box<str>>,
    /// Catalogue-relative path, e.g. "sqlite/en.sahih.sqlite".
    pub path: Box<str>,
    pub size_bytes: u64,
    pub sha256: Box<str>,
}

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
    #[inline]
    pub fn sura(&self, n: u16) -> Option<&SuraMeta> {
        if !(1..=SURA_COUNT as u16).contains(&n) {
            return None;
        }
        Some(&self.suras[(n - 1) as usize])
    }

    #[inline]
    pub fn suras(&self) -> &[SuraMeta] {
        &self.suras
    }

    pub fn global_of(&self, sura: u16, aya: u16) -> Option<u32> {
        self.sura(sura)?.global_of(aya)
    }

    pub fn locate(&self, g: u32) -> Option<(u16, u16)> {
        if !(1..=VERSE_COUNT).contains(&g) {
            return None;
        }
        let s = self
            .suras
            .iter()
            .find(|s| g >= s.start_global && g <= s.end_global)?;
        let aya = (g - (s.start_global - 1)) as u16;
        Some((s.index, aya))
    }
}

pub fn range_containing<K>(ranges: &[Range<K>], g: u32) -> Option<&Range<K>> {
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

pub struct QuranStore {
    pub uthmani: Corpus,
    pub simple_clean: Corpus,
    pub meta: QuranMeta,
    pub source_digests: SourceDigests,
    pub artifacts: Artifacts,
    pub search: super::search::SearchIndex,
}

impl QuranStore {
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
    pub fn etag_tag(&self) -> &str {
        &self.source_digests.uthmani
    }

    #[inline]
    pub fn source_digests(&self) -> &SourceDigests {
        &self.source_digests
    }

    pub fn sajda_at(&self, g: u32) -> Option<SajdaKind> {
        self.meta
            .sajdas
            .iter()
            .find(|s| s.global_index == g)
            .map(|s| s.kind)
    }

    pub fn navigation(&self, g: u32) -> Option<Navigation> {
        if !(1..=VERSE_COUNT).contains(&g) {
            return None;
        }
        let juz = range_containing(&self.meta.juzs, g)?.index;
        let page = range_containing(&self.meta.pages, g)?.index;
        let ruku = range_containing(&self.meta.rukus, g)?.index;
        let quarter = range_containing(&self.meta.hizb_quarters, g)?.index;
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

#[derive(Clone, Copy, Debug)]
pub struct Navigation {
    pub juz: u16,
    pub page: u16,
    pub ruku: u16,
    pub hizb_quarter: u16,
    pub manzil: u8,
}

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
