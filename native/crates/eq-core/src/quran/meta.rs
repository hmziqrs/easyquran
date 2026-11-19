//! Quran metadata (PARITY B3): surah catalogue, canonical coordinates,
//! mushaf divisions (page, juz, ruku, hizb-quarter, manzil, derived hizb),
//! sajdas, surah-local pages and browse orders. Baked from the web's own
//! `static/quran-meta/quran-data.json` and `lib/data/surah-names.json`
//! (mirror of `quran-data.ts`, `mushaf-divisions.ts`, `quran-browse.ts`).
//! No Quran DB access and no hashing: the snapshot's first field is a
//! provenance record the web keeps and this module never reads.

use std::{collections::HashMap, sync::OnceLock};

use serde::{Deserialize, Serialize};

use super::types::OpenerKind;

const QURAN_DATA_JSON: &str = include_str!("../../../../../web/static/quran-meta/quran-data.json");
const SURAH_NAMES_JSON: &str = include_str!("../../../../../web/src/lib/data/surah-names.json");

pub const SURAH_COUNT: u16 = 114;
pub const AYAH_COUNT: u32 = 6236;
pub const PAGE_COUNT: u16 = 604;
pub const JUZ_COUNT: u8 = 30;
pub const HIZB_COUNT: u8 = 60;
pub const HIZB_QUARTER_COUNT: u16 = 240;
const QUARTERS_PER_HIZB: u16 = 4;

/// Revelation place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    Meccan,
    Medinan,
}

/// How the bismillah sits in a surah (`Bismillah`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Bismillah {
    FirstAyah,
    None,
    EmbeddedPrefix,
}

/// One surah (`CatalogEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surah {
    pub num: u16,
    pub slug: String,
    /// English display name (`Al-Fatihah`).
    pub name: String,
    /// Diacritized Arabic display name.
    pub arabic: String,
    /// Plain Tanzil Arabic name (matching form).
    pub arabic_plain: String,
    pub transliteration: String,
    /// English meaning.
    pub meaning: String,
    pub place: Place,
    pub ayah_count: u16,
    pub revelation_order: u16,
    pub rukus: u16,
    pub opener_kind: OpenerKind,
    pub bismillah: Bismillah,
    pub start_global: u32,
}

impl Surah {
    /// Last global index of the surah.
    pub fn end_global(&self) -> u32 {
        self.start_global + u32::from(self.ayah_count) - 1
    }
}

/// A division family (`RangeKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RangeKind {
    Page,
    Juz,
    Ruku,
    HizbQuarter,
    Manzil,
}

impl RangeKind {
    const ALL: [RangeKind; 5] = [
        RangeKind::Page,
        RangeKind::Juz,
        RangeKind::Ruku,
        RangeKind::HizbQuarter,
        RangeKind::Manzil,
    ];

    fn root(self) -> usize {
        match self {
            RangeKind::Page => 2,
            RangeKind::Juz => 3,
            RangeKind::Ruku => 4,
            RangeKind::HizbQuarter => 5,
            RangeKind::Manzil => 6,
        }
    }

    /// Ranges in the family (`RANGE_COUNTS`).
    pub fn count(self) -> usize {
        match self {
            RangeKind::Page => 604,
            RangeKind::Juz => 30,
            RangeKind::Ruku => 556,
            RangeKind::HizbQuarter => 240,
            RangeKind::Manzil => 7,
        }
    }
}

/// One division range (`RangeEntry`), keys as `(surah, ayah)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RangeEntry {
    pub index: u16,
    pub start_global: u32,
    pub end_global: u32,
    pub first: (u16, u16),
    pub last: (u16, u16),
}

/// Sajda kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SajdaKind {
    Recommended,
    Obligatory,
}

/// One sajda marker (`SajdaEntry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sajda {
    pub index: u16,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    pub kind: SajdaKind,
}

/// One surah's slice of a global mushaf page (`SurahLocalPage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurahLocalPage {
    pub surah: u16,
    pub local_page: u16,
    pub global_page: u16,
    pub start_global: u32,
    pub end_global: u32,
    pub start_ayah: u16,
    pub end_ayah: u16,
}

/// Where a verse sits in the mushaf divisions (`ReaderPositionState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DivisionPosition {
    pub global_page: u16,
    pub juz: u8,
    pub hizb: u8,
}

/// Surah list order (`SurahSort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurahSort {
    #[default]
    Mushaf,
    Revelation,
}

/// The decoded metadata snapshot (`QuranData`).
#[derive(Debug)]
pub struct QuranMeta {
    surahs: Vec<Surah>,
    by_slug: HashMap<String, u16>,
    starts: HashMap<RangeKind, Vec<u32>>,
    sajdas: Vec<Sajda>,
}

/// The baked metadata, decoded once.
pub fn meta() -> &'static QuranMeta {
    static META: OnceLock<QuranMeta> = OnceLock::new();
    META.get_or_init(|| {
        QuranMeta::decode(QURAN_DATA_JSON, SURAH_NAMES_JSON)
            .unwrap_or_else(|e| panic!("[quran-data] baked snapshot invalid: {e}"))
    })
}

fn int(v: &serde_json::Value, label: &str, min: u64) -> Result<u64, String> {
    v.as_u64()
        .filter(|n| *n >= min)
        .ok_or(format!("{label} must be an integer >= {min}"))
}

fn text(v: &serde_json::Value, label: &str) -> Result<String, String> {
    v.as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or(format!("{label} must be a non-empty string"))
}

fn opener_kind(num: u16) -> OpenerKind {
    match num {
        1 => OpenerKind::Verse,
        9 => OpenerKind::None,
        _ => OpenerKind::Header,
    }
}

fn bismillah(num: u16) -> Bismillah {
    match num {
        1 => Bismillah::FirstAyah,
        9 => Bismillah::None,
        _ => Bismillah::EmbeddedPrefix,
    }
}

fn narrow<T: TryFrom<u64>>(n: u64, label: &str) -> Result<T, String> {
    T::try_from(n).map_err(|_| format!("{label} out of range"))
}

impl QuranMeta {
    fn decode(data: &str, names: &str) -> Result<Self, String> {
        let root: serde_json::Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
        let root = root
            .as_array()
            .filter(|a| a.len() == 8)
            .ok_or("snapshot root must have eight fields")?;
        let names: serde_json::Value = serde_json::from_str(names).map_err(|e| e.to_string())?;
        let arabic_names = names["names"]
            .as_array()
            .filter(|a| a.len() == 114)
            .ok_or("expected 114 baked diacritized names")?;
        let rows = root[1]
            .as_array()
            .filter(|a| a.len() == 114)
            .ok_or("expected 114 Surah rows")?;
        let mut start_global = 1_u32;
        let mut surahs = Vec::with_capacity(114);
        for (i, row) in rows.iter().enumerate() {
            let num = narrow::<u16>(i as u64 + 1, "surah")?;
            let r = row
                .as_array()
                .filter(|a| a.len() == 9)
                .ok_or(format!("invalid row for Surah {num}"))?;
            let ayah_count: u16 = narrow(int(&r[6], "ayah count", 1)?, "ayah count")?;
            let place = match int(&r[5], "place", 0)? {
                0 => Place::Meccan,
                1 => Place::Medinan,
                other => return Err(format!("invalid place code {other}")),
            };
            surahs.push(Surah {
                num,
                slug: text(&r[0], "slug")?,
                name: text(&r[1], "display name")?,
                arabic: text(&arabic_names[i], "diacritized name")?,
                arabic_plain: text(&r[2], "Arabic name")?,
                transliteration: text(&r[3], "transliteration")?,
                meaning: text(&r[4], "meaning")?,
                place,
                ayah_count,
                revelation_order: narrow(int(&r[7], "revelation order", 1)?, "order")?,
                rukus: narrow(int(&r[8], "ruku count", 1)?, "rukus")?,
                opener_kind: opener_kind(num),
                bismillah: bismillah(num),
                start_global,
            });
            start_global += u32::from(ayah_count);
        }
        if start_global - 1 != AYAH_COUNT {
            return Err(format!(
                "expected {AYAH_COUNT} ayahs, got {}",
                start_global - 1
            ));
        }
        let by_slug: HashMap<String, u16> =
            surahs.iter().map(|s| (s.slug.clone(), s.num)).collect();
        if by_slug.len() != surahs.len() {
            return Err("Surah slugs must be unique".into());
        }
        let mut starts = HashMap::new();
        for kind in RangeKind::ALL {
            let series = root[kind.root()]
                .as_array()
                .filter(|a| a.len() == kind.count())
                .ok_or(format!("expected {} {kind:?} ranges", kind.count()))?;
            let mut decoded = Vec::with_capacity(series.len());
            for (i, delta) in series.iter().enumerate() {
                let d = int(delta, "range delta", 1)?;
                let start: u32 = if i == 0 {
                    if d != 1 {
                        return Err("range series must start at global ayah 1".into());
                    }
                    1
                } else {
                    decoded[i - 1] + narrow::<u32>(d, "delta")?
                };
                decoded.push(start);
            }
            if decoded.last().is_some_and(|&s| s > AYAH_COUNT) {
                return Err(format!("{kind:?} starts past the Quran"));
            }
            starts.insert(kind, decoded);
        }
        let mut meta = QuranMeta {
            surahs,
            by_slug,
            starts,
            sajdas: Vec::new(),
        };
        let sajda_rows = root[7]
            .as_array()
            .filter(|a| a.len() == 15)
            .ok_or("expected 15 sajda markers")?;
        for (i, row) in sajda_rows.iter().enumerate() {
            let r = row
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or(format!("invalid sajda row {}", i + 1))?;
            let surah: u16 = narrow(int(&r[0], "sajda surah", 1)?, "sajda surah")?;
            let ayah: u16 = narrow(int(&r[1], "sajda ayah", 1)?, "sajda ayah")?;
            let kind = match int(&r[2], "sajda kind", 0)? {
                0 => SajdaKind::Recommended,
                1 => SajdaKind::Obligatory,
                _ => return Err(format!("invalid sajda row {}", i + 1)),
            };
            let global_index = meta
                .global_index_of(surah, ayah)
                .ok_or(format!("invalid sajda row {}", i + 1))?;
            meta.sajdas.push(Sajda {
                index: narrow(i as u64 + 1, "sajda")?,
                surah,
                ayah,
                global_index,
                kind,
            });
        }
        Ok(meta)
    }

    /// The 114 surahs, mushaf order.
    pub fn surahs(&self) -> &[Surah] {
        &self.surahs
    }

    /// Surah `num` (1..=114).
    pub fn surah(&self, num: u16) -> Option<&Surah> {
        if num == 0 {
            return None;
        }
        self.surahs.get(usize::from(num) - 1)
    }

    /// The surah a slug names.
    pub fn surah_by_slug(&self, slug: &str) -> Option<&Surah> {
        self.by_slug.get(slug).and_then(|&n| self.surah(n))
    }

    /// Global index (1..=6236) of `surah:ayah`.
    pub fn global_index_of(&self, surah: u16, ayah: u16) -> Option<u32> {
        let s = self.surah(surah)?;
        if ayah < 1 || ayah > s.ayah_count {
            return None;
        }
        Some(s.start_global + u32::from(ayah) - 1)
    }

    /// `(surah, ayah)` at a global index (`verseKeyAtGlobal`).
    pub fn verse_at_global(&self, global: u32) -> Option<(u16, u16)> {
        if !(1..=AYAH_COUNT).contains(&global) {
            return None;
        }
        let i = self
            .surahs
            .partition_point(|s| s.start_global <= global)
            .checked_sub(1)?;
        let s = &self.surahs[i];
        let ayah = u16::try_from(global - s.start_global + 1).ok()?;
        Some((s.num, ayah))
    }

    /// Ranges in a family.
    pub fn range_count(&self, kind: RangeKind) -> usize {
        self.starts.get(&kind).map_or(0, Vec::len)
    }

    /// Range `index` (1-based) of a family.
    pub fn range(&self, kind: RangeKind, index: u16) -> Option<RangeEntry> {
        let family = self.starts.get(&kind)?;
        let i = usize::from(index);
        if i < 1 || i > family.len() {
            return None;
        }
        let start = family[i - 1];
        let end = family.get(i).map_or(AYAH_COUNT, |next| next - 1);
        Some(RangeEntry {
            index,
            start_global: start,
            end_global: end,
            first: self.verse_at_global(start)?,
            last: self.verse_at_global(end)?,
        })
    }

    /// Every range of a family.
    pub fn ranges(&self, kind: RangeKind) -> Vec<RangeEntry> {
        (1..=self.range_count(kind))
            .filter_map(|i| self.range(kind, u16::try_from(i).ok()?))
            .collect()
    }

    fn range_index_containing(&self, kind: RangeKind, global: u32) -> Option<u16> {
        let family = self.starts.get(&kind)?;
        if !(1..=AYAH_COUNT).contains(&global) {
            return None;
        }
        u16::try_from(family.partition_point(|&s| s <= global)).ok()
    }

    /// Global page (1..=604) holding a global index; 1 when out of range
    /// (`pageOfGlobal`).
    pub fn page_of_global(&self, global: u32) -> u16 {
        self.range_index_containing(RangeKind::Page, global)
            .unwrap_or(1)
    }

    /// Juz (1..=30) holding a global index (`juzOfPage`).
    pub fn juz_of_global(&self, global: u32) -> u8 {
        self.range_index_containing(RangeKind::Juz, global)
            .and_then(|j| u8::try_from(j).ok())
            .unwrap_or(1)
    }

    /// Hizb quarter (1..=240) holding a global index (`quarterOfPage`).
    pub fn quarter_of_global(&self, global: u32) -> u16 {
        self.range_index_containing(RangeKind::HizbQuarter, global)
            .unwrap_or(1)
    }

    /// Hizb (1..=60): ceil(quarter / 4) (`hizbOfPage`).
    pub fn hizb_of_global(&self, global: u32) -> u8 {
        u8::try_from(self.quarter_of_global(global).div_ceil(QUARTERS_PER_HIZB)).unwrap_or(1)
    }

    /// Division position of a global index (`positionForGlobal`).
    pub fn position_for_global(&self, global: u32) -> DivisionPosition {
        DivisionPosition {
            global_page: self.page_of_global(global),
            juz: self.juz_of_global(global),
            hizb: self.hizb_of_global(global),
        }
    }

    /// Hizb `hizb` (1..=60) as one range: quarters 4h-3..4h (`hizbRange`).
    pub fn hizb_range(&self, hizb: u8) -> Option<RangeEntry> {
        if !(1..=HIZB_COUNT).contains(&hizb) {
            return None;
        }
        let h = u16::from(hizb);
        let first = self.range(RangeKind::HizbQuarter, h * QUARTERS_PER_HIZB - 3)?;
        let last = self.range(RangeKind::HizbQuarter, h * QUARTERS_PER_HIZB)?;
        Some(RangeEntry {
            index: h,
            start_global: first.start_global,
            end_global: last.end_global,
            first: first.first,
            last: last.last,
        })
    }

    /// The 15 sajdas.
    pub fn sajdas(&self) -> &[Sajda] {
        &self.sajdas
    }

    /// The sajda at a verse.
    pub fn sajda_at(&self, surah: u16, ayah: u16) -> Option<&Sajda> {
        self.sajdas
            .iter()
            .find(|s| s.surah == surah && s.ayah == ayah)
    }

    /// The surah's slices of the global pages it touches
    /// (`surahLocalPages`).
    pub fn surah_local_pages(&self, surah: u16) -> Vec<SurahLocalPage> {
        let Some(entry) = self.surah(surah) else {
            return Vec::new();
        };
        let end = entry.end_global();
        self.ranges(RangeKind::Page)
            .into_iter()
            .filter(|p| p.end_global >= entry.start_global && p.start_global <= end)
            .enumerate()
            .map(|(i, p)| {
                let start_global = p.start_global.max(entry.start_global);
                let end_global = p.end_global.min(end);
                let local = |g: u32| u16::try_from(g - entry.start_global + 1).unwrap_or(0);
                SurahLocalPage {
                    surah,
                    local_page: u16::try_from(i + 1).unwrap_or(0),
                    global_page: p.index,
                    start_global,
                    end_global,
                    start_ayah: local(start_global),
                    end_ayah: local(end_global),
                }
            })
            .collect()
    }

    /// The surah-local page holding a verse (`surahLocalPageForAyah`).
    pub fn surah_local_page_for_ayah(&self, surah: u16, ayah: u16) -> Option<SurahLocalPage> {
        let global = self.global_index_of(surah, ayah)?;
        self.surah_local_pages(surah)
            .into_iter()
            .find(|p| global >= p.start_global && global <= p.end_global)
    }

    /// Surahs in a browse order (`surahsInSortOrder`): revelation order
    /// ties keep mushaf order.
    pub fn surahs_in_order(&self, sort: SurahSort) -> Vec<&Surah> {
        let mut out: Vec<&Surah> = self.surahs.iter().collect();
        if sort == SurahSort::Revelation {
            out.sort_by_key(|s| (s.revelation_order, s.num));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_every_canonical_field_and_coordinate() {
        let m = meta();
        assert_eq!(m.surahs().len(), 114);
        let f = m.surah(1).unwrap();
        assert_eq!(f.slug, "al-fatihah");
        assert_eq!(f.transliteration, "Al-Faatiha");
        assert_eq!(f.meaning, "The Opening");
        assert_eq!(f.start_global, 1);
        assert_eq!(m.verse_at_global(262), Some((2, 255)));
        assert_eq!(m.global_index_of(2, 255), Some(262));
        assert_eq!(m.verse_at_global(6236), Some((114, 6)));
    }

    #[test]
    fn fails_explicitly_instead_of_falling_back() {
        let m = meta();
        assert!(m.surah(0).is_none());
        assert!(m.surah_by_slug("does-not-exist").is_none());
        assert_eq!(m.verse_at_global(0), None);
        assert_eq!(m.global_index_of(2, 287), None);
    }

    #[test]
    fn range_families() {
        let m = meta();
        assert_eq!(m.range_count(RangeKind::Page), 604);
        assert_eq!(m.range_count(RangeKind::Juz), 30);
        assert_eq!(m.range_count(RangeKind::Ruku), 556);
        assert_eq!(m.range_count(RangeKind::HizbQuarter), 240);
        assert_eq!(m.range_count(RangeKind::Manzil), 7);
        let p1 = m.range(RangeKind::Page, 1).unwrap();
        assert_eq!((p1.first, p1.last), ((1, 1), (1, 7)));
        assert!(m.range(RangeKind::Page, 605).is_none());
        assert_eq!(m.sajdas().len(), 15);
        let h60 = m.hizb_range(60).unwrap();
        assert_eq!(h60.end_global, AYAH_COUNT);
        assert!(m.hizb_range(61).is_none());
        assert_eq!(m.hizb_of_global(AYAH_COUNT), 60);
        assert_eq!(m.position_for_global(1).global_page, 1);
    }

    #[test]
    fn local_pages_tile_every_surah() {
        let m = meta();
        let total: usize = (1..=114).map(|s| m.surah_local_pages(s).len()).sum();
        assert_eq!(total, 662);
        for s in m.surahs() {
            let pages = m.surah_local_pages(s.num);
            assert_eq!(pages[0].start_ayah, 1);
            assert_eq!(pages.last().unwrap().end_ayah, s.ayah_count);
            for w in pages.windows(2) {
                assert_eq!(w[1].start_ayah, w[0].end_ayah + 1);
            }
        }
        let b = m.surah_local_pages(2);
        assert_eq!(
            (b[0].global_page, b[0].start_ayah, b[0].end_ayah),
            (2, 1, 5)
        );
        assert!(m.surah_local_page_for_ayah(2, 255).unwrap().local_page > 1);
    }

    #[test]
    fn revelation_order_is_total() {
        let m = meta();
        let order = m.surahs_in_order(SurahSort::Revelation);
        assert_eq!(order.len(), 114);
        assert_eq!(order[0].revelation_order, 1);
        assert_eq!(m.surahs_in_order(SurahSort::Mushaf)[0].num, 1);
    }
}
