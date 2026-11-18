//! The reader's view model: a scope read (Arabic groups plus translation
//! lanes) flattened into the virtual list's rows, and the route math the
//! reader needs (neighbouring scopes, session scope strings, positions).
//! Text stays verbatim: every string here is a row, or a verbatim slice
//! the view rules made (`eq_core::quran::view`).

use std::collections::HashMap;

use eq_core::{
    quran::{
        catalogue::{self, Direction},
        meta::{DivisionPosition, QuranMeta, RangeKind, meta},
        read::{ReadError, ServedBy},
        types::{ArabicSource, RangeText, SurahNormalization},
        view::{RangeGroup, VerseView, header_raw},
    },
    routes::{ReaderLink, ReaderMode, ReaderScope, ReaderTarget},
};

/// One translation's text of one verse (a lane under the Arabic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneText {
    pub source_id: String,
    /// The lane label (catalogue short name).
    pub name: String,
    pub text: String,
    pub rtl: bool,
}

/// The surah's opener above its first verse: verbatim (tajweed markup
/// kept for the coloured header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opener {
    pub raw: String,
    pub normalization: SurahNormalization,
}

/// A row of the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A surah's title band (and its bismillah when the run opens it).
    Header { surah: u16, opener: Option<Opener> },
    /// Ayah-by-Ayah: one verse and its lanes.
    Verse {
        verse: VerseView,
        lanes: Vec<LaneText>,
    },
    /// Reading: the verses of one mushaf page of one surah, as one paragraph.
    Flow {
        surah: u16,
        page: u16,
        verses: Vec<VerseView>,
    },
}

impl Item {
    /// The row's identity in the list.
    pub fn key(&self) -> String {
        match self {
            Item::Header { surah, .. } => format!("h:{surah}"),
            Item::Verse { verse, .. } => verse.key.clone(),
            Item::Flow { surah, page, .. } => format!("f:{surah}:{page}"),
        }
    }

    /// The verses the row shows, in order.
    pub fn verses(&self) -> Vec<(u16, u16)> {
        match self {
            Item::Header { .. } => Vec::new(),
            Item::Verse { verse, .. } => vec![(verse.surah, verse.ayah)],
            Item::Flow { verses, .. } => verses.iter().map(|v| (v.surah, v.ayah)).collect(),
        }
    }

    /// Whether the row holds `surah:ayah`.
    pub fn holds(&self, surah: u16, ayah: u16) -> bool {
        match self {
            Item::Header { .. } => false,
            Item::Verse { verse, .. } => verse.surah == surah && verse.ayah == ayah,
            Item::Flow { verses, .. } => verses.iter().any(|v| v.surah == surah && v.ayah == ayah),
        }
    }
}

/// A finished read of the reader's scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub groups: Vec<RangeGroup>,
    pub served: ServedBy,
    pub source: ArabicSource,
    /// Lanes, in route order: (source id, text by verse key).
    pub lanes: Vec<(String, HashMap<String, String>)>,
    /// Lanes that failed to read.
    pub failed_lanes: Vec<String>,
}

/// Why the Arabic could not be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Not on the device and no online fallback (offline, or a script the
    /// read API does not serve): download it.
    NotDownloaded,
    /// The network read failed.
    Network,
    /// The on-device database failed.
    Data,
}

impl Failure {
    pub fn of(error: &ReadError) -> Failure {
        match error {
            ReadError::NotAvailable(_) => Failure::NotDownloaded,
            ReadError::Online(_) => Failure::Network,
            ReadError::Db(_) | ReadError::OutOfRange => Failure::Data,
        }
    }
}

/// The lanes of a read, in order, as text by verse key.
pub fn lanes_of(reads: Vec<(String, Result<RangeText, ReadError>)>) -> LaneSet {
    let mut lanes = Vec::new();
    let mut failed = Vec::new();
    for (id, read) in reads {
        match read {
            Ok(text) => lanes.push((
                id,
                text.ayahs
                    .into_iter()
                    .map(|a| (a.key, a.text))
                    .collect::<HashMap<_, _>>(),
            )),
            Err(_) => failed.push(id),
        }
    }
    (lanes, failed)
}

pub type LaneSet = (Vec<(String, HashMap<String, String>)>, Vec<String>);

fn lane_meta(id: &str) -> (String, bool) {
    catalogue::translation(id).map_or_else(
        || (id.to_owned(), false),
        |t| (t.name.clone(), t.direction == Direction::Rtl),
    )
}

/// The rows for `loaded` in `mode`.
pub fn build_items(loaded: &Loaded, mode: ReaderMode) -> Vec<Item> {
    let m = meta();
    let lane_info: Vec<(String, bool)> = loaded.lanes.iter().map(|(id, _)| lane_meta(id)).collect();
    let mut out = Vec::new();
    for group in &loaded.groups {
        let opens = group.verses.first().is_some_and(|v| v.ayah == 1);
        let opener = header_raw(&group.normalization)
            .filter(|_| opens)
            .map(|raw| Opener {
                raw: raw.to_owned(),
                normalization: group.normalization.clone(),
            });
        out.push(Item::Header {
            surah: group.surah,
            opener,
        });
        match mode {
            ReaderMode::Verse => {
                for verse in &group.verses {
                    let lanes = loaded
                        .lanes
                        .iter()
                        .zip(&lane_info)
                        .filter_map(|((id, by_key), (name, rtl))| {
                            by_key.get(&verse.key).map(|text| LaneText {
                                source_id: id.clone(),
                                name: name.clone(),
                                text: text.clone(),
                                rtl: *rtl,
                            })
                        })
                        .collect();
                    out.push(Item::Verse {
                        verse: verse.clone(),
                        lanes,
                    });
                }
            }
            ReaderMode::Reading => flow_rows(m, group, &mut out),
        }
    }
    out
}

/// Reading mode: one paragraph per mushaf page of the run (the web's
/// local pages), so a long surah stays virtualized.
fn flow_rows(m: &QuranMeta, group: &RangeGroup, out: &mut Vec<Item>) {
    let mut page: Option<u16> = None;
    let mut verses: Vec<VerseView> = Vec::new();
    for verse in &group.verses {
        let p = m.page_of_global(verse.global_index);
        if page.is_some_and(|q| q != p) && !verses.is_empty() {
            out.push(Item::Flow {
                surah: group.surah,
                page: page.unwrap_or(p),
                verses: std::mem::take(&mut verses),
            });
        }
        page = Some(p);
        verses.push(verse.clone());
    }
    if let (Some(page), false) = (page, verses.is_empty()) {
        out.push(Item::Flow {
            surah: group.surah,
            page,
            verses,
        });
    }
}

/// The row holding `surah:ayah`.
pub fn index_of(items: &[Item], surah: u16, ayah: u16) -> Option<usize> {
    items.iter().position(|i| i.holds(surah, ayah))
}

/// The first verse at or after row `index` (a header reads as the verse
/// under it).
pub fn verse_at(items: &[Item], index: usize) -> Option<(u16, u16)> {
    items
        .iter()
        .skip(index)
        .find_map(|i| i.verses().first().copied())
}

/// The session scope string (`surah:73`, `page:574`, `juz:30`, `hizb:60`).
pub fn scope_key(scope: &ReaderScope) -> String {
    match *scope {
        ReaderScope::Surah(n) => format!("surah:{n}"),
        ReaderScope::Page(n) => format!("page:{n}"),
        ReaderScope::Juz(n) => format!("juz:{n}"),
        ReaderScope::Hizb(n) => format!("hizb:{n}"),
    }
}

/// The first verse of `scope`.
pub fn first_verse(scope: &ReaderScope) -> Option<(u16, u16)> {
    let m = meta();
    let (from, _) = eq_core::quran::read::scope_range(m, scope)?;
    m.verse_at_global(from)
}

/// Whether `surah:ayah` is inside `scope`.
pub fn in_scope(scope: &ReaderScope, surah: u16, ayah: u16) -> bool {
    let m = meta();
    let Some((from, to)) = eq_core::quran::read::scope_range(m, scope) else {
        return false;
    };
    m.global_index_of(surah, ayah)
        .is_some_and(|g| g >= from && g <= to)
}

/// Mushaf divisions of a verse (the position indicator).
pub fn position_of(surah: u16, ayah: u16) -> Option<DivisionPosition> {
    let m = meta();
    m.global_index_of(surah, ayah)
        .map(|g| m.position_for_global(g))
}

/// The scope before (`-1`) or after (`+1`) `scope`, inside the mushaf.
pub fn neighbour(scope: ReaderScope, step: i32) -> Option<ReaderScope> {
    let m = meta();
    let (index, max) = match scope {
        ReaderScope::Surah(n) => (i32::from(n), 114),
        ReaderScope::Page(n) => (i32::from(n), count(m, RangeKind::Page)),
        ReaderScope::Juz(n) => (i32::from(n), count(m, RangeKind::Juz)),
        ReaderScope::Hizb(n) => (i32::from(n), 60),
    };
    let next = index + step;
    if next < 1 || next > max {
        return None;
    }
    let next16 = u16::try_from(next).ok()?;
    let next8 = u8::try_from(next).ok();
    Some(match scope {
        ReaderScope::Surah(_) => ReaderScope::Surah(next16),
        ReaderScope::Page(_) => ReaderScope::Page(next16),
        ReaderScope::Juz(_) => ReaderScope::Juz(next8?),
        ReaderScope::Hizb(_) => ReaderScope::Hizb(next8?),
    })
}

fn count(m: &QuranMeta, kind: RangeKind) -> i32 {
    i32::try_from(m.range_count(kind)).unwrap_or(0)
}

/// `link` moved to `scope` (at `ayah`), keeping translation and view params.
pub fn link_to(link: &ReaderLink, scope: ReaderScope, ayah: Option<u16>) -> ReaderLink {
    ReaderLink {
        target: ReaderTarget {
            scope,
            translation: link.target.translation.clone(),
            ayah,
        },
        mode: link.mode,
        more: link.more.clone(),
    }
}

/// The translation ids the route stacks: the route's own first, then `?more=`.
pub fn route_translations(link: &ReaderLink) -> Vec<String> {
    let mut ids: Vec<String> = link.target.translation.iter().map(|t| t.id()).collect();
    for id in &link.more {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids
}

/// The surah-local page of a verse and the verse's place in it (0..1):
/// the thread's fine anchor (`setLastReadAnchor`), by verse count.
pub fn local_anchor(surah: u16, ayah: u16) -> Option<(i64, f64)> {
    let page = meta().surah_local_page_for_ayah(surah, ayah)?;
    let span = f64::from(page.end_ayah.saturating_sub(page.start_ayah) + 1);
    let at = f64::from(ayah.saturating_sub(page.start_ayah));
    Some((i64::from(page.local_page), at / span))
}
