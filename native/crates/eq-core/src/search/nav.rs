//! Go-to-verse parsing (`search/nav/{parse,aliases,juz-nicknames,match}.ts`):
//! the query tokenised once (leading keyword, trailing numeric reference,
//! Arabic-Indic digits folded), keyword aliases, juz nicknames, and the
//! structured-coordinate matcher shared by the palette and the search page.

use std::sync::OnceLock;

use regex::Regex;

use crate::quran::meta::{JUZ_COUNT, PAGE_COUNT, Place, QuranMeta, SajdaKind};

use super::{normalize::normalize_arabic, translit::translit_key};

/// `SURAH_ALIASES`.
pub const SURAH_ALIASES: &[&str] = &["s", "sura", "surah", "surat", "chapter", "سورة", "سوره"];
/// `JUZ_ALIASES`.
pub const JUZ_ALIASES: &[&str] = &[
    "juz", "juzz", "juzu", "jooz", "para", "sipara", "siparah", "جزء", "جز",
];
/// `PAGE_ALIASES`.
pub const PAGE_ALIASES: &[&str] = &["page", "pg", "p", "safha", "safhah", "صفحة", "صفحه"];
/// `SAJDA_ALIASES`.
pub const SAJDA_ALIASES: &[&str] = &["sajda", "sajdah", "sujud", "prostration", "سجدة", "سجده"];
const PLACE_ROWS: &[(&str, Place)] = &[
    ("meccan", Place::Meccan),
    ("makkan", Place::Meccan),
    ("mekkan", Place::Meccan),
    ("مكية", Place::Meccan),
    ("مكيه", Place::Meccan),
    ("medinan", Place::Medinan),
    ("madani", Place::Medinan),
    ("madanian", Place::Medinan),
    ("مدنية", Place::Medinan),
    ("مدنيه", Place::Medinan),
];
/// `TRANSLATION_ALIASES`.
pub const TRANSLATION_ALIASES: &[&str] = &["translation", "translations"];

/// `PLACE_ALIASES`.
pub fn place_aliases() -> impl Iterator<Item = &'static str> {
    PLACE_ROWS.iter().map(|(a, _)| *a)
}

/// `QURAN_ALIASES`: every keyword the Quran surfaces claim.
pub fn quran_aliases() -> &'static [&'static str] {
    static ALL: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v: Vec<&'static str> = Vec::new();
        v.extend_from_slice(SURAH_ALIASES);
        v.extend_from_slice(JUZ_ALIASES);
        v.extend_from_slice(PAGE_ALIASES);
        v.extend_from_slice(SAJDA_ALIASES);
        v.extend(place_aliases());
        v
    })
}

/// `ALL_KEYWORD_ALIASES`: the Quran keywords plus translation's.
pub fn all_keyword_aliases() -> &'static [&'static str] {
    static ALL: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v = quran_aliases().to_vec();
        v.extend_from_slice(TRANSLATION_ALIASES);
        v
    })
}

/// `placeForAlias`.
pub fn place_for_alias(alias: &str) -> Option<Place> {
    PLACE_ROWS
        .iter()
        .find(|(a, _)| *a == alias)
        .map(|(_, p)| *p)
}

/// Juz nicknames people name divisions by (`JUZ_KEYWORDS`).
pub const JUZ_KEYWORDS: &[(&str, u8)] = &[
    ("qad sama", 28),
    ("qadsama", 28),
    ("tabarak", 29),
    ("amma", 30),
    ("ama", 30),
    ("mubarak", 29),
];

/// `juzKeywordFor`: the first nickname row whose fold equals `raw`'s.
pub fn juz_keyword_for(raw: &str) -> Option<(&'static str, u8)> {
    let key = translit_key(raw);
    JUZ_KEYWORDS
        .iter()
        .find(|(alias, _)| translit_key(alias) == key)
        .copied()
}

/// A query tokenised once for every consumer (`ParsedQuery`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    /// Trimmed, whitespace collapsed.
    pub text: String,
    pub lower: String,
    /// Lowercased leading word; `None` when the query starts otherwise.
    pub keyword: Option<String>,
    /// `text` without the leading word, trimmed.
    pub after_keyword: String,
    /// Numbers of a trailing `2:255` / `2 255` / `255` reference, plus a
    /// range suffix's end (`2:255-286`: 3, `2:255-2:286`: 4).
    pub numbers: Vec<u32>,
    /// `text` normalised for Arabic matching.
    pub arabic: String,
    pub is_empty: bool,
}

const DIGIT: &str = "0-9٠-٩۰-۹";

fn leading_word() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\p{L}[\p{L}'’-]*)").expect("leading word"))
}

fn trailing_ref() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"([{DIGIT}]{{1,4}})(?:\s*[:.\-\s]\s*([{DIGIT}]{{1,4}}))?(?:\s*-\s*([{DIGIT}]{{1,4}})(?:\s*:\s*([{DIGIT}]{{1,4}}))?)?\s*$"
        ))
        .expect("trailing ref")
    })
}

fn bare_number() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"^[{DIGIT}]{{1,4}}$")).expect("bare number"))
}

/// `foldDigits`: Arabic-Indic and Persian digits onto ASCII.
pub fn fold_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '٠'..='٩' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            '۰'..='۹' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            other => other,
        })
        .collect()
}

/// `parseQuery`.
pub fn parse_query(raw: &str) -> ParsedQuery {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = text.to_lowercase();
    let word = leading_word()
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned());
    let keyword = word.as_ref().map(|w| w.to_lowercase());
    let after_keyword = match &word {
        Some(w) => text[w.len()..].trim().to_owned(),
        None => text.clone(),
    };
    let mut numbers = Vec::new();
    if let Some(caps) = trailing_ref().captures(&text) {
        for group in caps.iter().skip(1).flatten() {
            if let Ok(n) = fold_digits(group.as_str()).parse::<u32>() {
                numbers.push(n);
            }
        }
    }
    ParsedQuery {
        arabic: normalize_arabic(&text),
        is_empty: text.is_empty(),
        lower,
        keyword,
        after_keyword,
        numbers,
        text,
    }
}

/// `hasKeyword`.
pub fn has_keyword(parsed: &ParsedQuery, aliases: &[&str]) -> bool {
    parsed
        .keyword
        .as_deref()
        .is_some_and(|k| aliases.contains(&k))
}

/// `termsFor`: the query without a recognised keyword.
pub fn terms_for<'a>(parsed: &'a ParsedQuery, aliases: &[&str]) -> &'a str {
    if has_keyword(parsed, aliases) {
        return &parsed.after_keyword;
    }
    &parsed.text
}

/// `arabicTermsFor`.
pub fn arabic_terms_for(parsed: &ParsedQuery, aliases: &[&str]) -> String {
    if has_keyword(parsed, aliases) {
        return normalize_arabic(&parsed.after_keyword);
    }
    parsed.arabic.clone()
}

/// `stripTrailingRef`: `baqarah 255` -> `baqarah`.
pub fn strip_trailing_ref(text: &str) -> String {
    trailing_ref().replace(text, "").trim().to_owned()
}

/// `referenceNumbers`: `(primary, secondary)`.
pub fn reference_numbers(parsed: &ParsedQuery) -> Option<(u32, Option<u32>)> {
    let primary = *parsed.numbers.first()?;
    Some((primary, parsed.numbers.get(1).copied()))
}

/// A range's endpoints (`AyahRange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AyahRange {
    pub start_surah: u32,
    pub start_ayah: u32,
    pub end_surah: u32,
    pub end_ayah: u32,
}

/// `rangeNumbers`: `2:255-286` (end in the start surah) or `2:255-2:286`.
pub fn range_numbers(parsed: &ParsedQuery) -> Option<AyahRange> {
    let n = &parsed.numbers;
    let (start_surah, start_ayah, end) = (*n.first()?, *n.get(1)?, *n.get(2)?);
    match n.get(3) {
        None => Some(AyahRange {
            start_surah,
            start_ayah,
            end_surah: start_surah,
            end_ayah: end,
        }),
        Some(second) => Some(AyahRange {
            start_surah,
            start_ayah,
            end_surah: end,
            end_ayah: *second,
        }),
    }
}

/// `isBareNumber`.
pub fn is_bare_number(parsed: &ParsedQuery) -> bool {
    bare_number().is_match(&parsed.text)
}

/// `residualText`: free text left after a keyword and a trailing ref.
pub fn residual_text(parsed: &ParsedQuery, aliases: &[&str]) -> String {
    strip_trailing_ref(terms_for(parsed, aliases))
}

/// Where a nav match goes (`NavTarget`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavTarget {
    Ayah {
        surah: u16,
        ayah: u16,
    },
    Surah(u16),
    Juz(u8),
    Page(u16),
    Sajda {
        surah: u16,
        ayah: u16,
        index: u16,
        kind: SajdaKind,
    },
    Place {
        place: Place,
        surah_count: usize,
    },
}

/// One nav suggestion (`NavMatch`).
#[derive(Debug, Clone, PartialEq)]
pub struct NavMatch {
    /// Unique: `ayah:2:255`, `juz:5`, `sajda:7:206`, `place:meccan`.
    pub id: String,
    /// `[0, 1]`.
    pub score: f64,
    pub target: NavTarget,
}

fn push_ayah(out: &mut Vec<NavMatch>, meta: &QuranMeta, surah: u32, ayah: u32) {
    let Ok(s) = u16::try_from(surah) else { return };
    let Some(entry) = meta.surah(s) else { return };
    if ayah < 1 || ayah > u32::from(entry.ayah_count) {
        return;
    }
    out.push(NavMatch {
        id: format!("ayah:{surah}:{ayah}"),
        score: 1.0,
        target: NavTarget::Ayah {
            surah: s,
            ayah: ayah as u16,
        },
    });
}

fn push_ayah_range(out: &mut Vec<NavMatch>, meta: &QuranMeta, range: AyahRange) {
    let end = u16::try_from(range.end_surah)
        .ok()
        .and_then(|s| meta.surah(s));
    let Some(end) = end else { return };
    if range.end_ayah < 1 || range.end_ayah > u32::from(end.ayah_count) {
        return;
    }
    push_ayah(out, meta, range.start_surah, range.start_ayah);
}

fn push_surah(out: &mut Vec<NavMatch>, meta: &QuranMeta, surah: u32, score: f64) {
    let Ok(s) = u16::try_from(surah) else { return };
    if meta.surah(s).is_none() {
        return;
    }
    out.push(NavMatch {
        id: format!("surah:{surah}"),
        score,
        target: NavTarget::Surah(s),
    });
}

fn push_juz(out: &mut Vec<NavMatch>, juz: u32, score: f64) {
    if juz < 1 || juz > u32::from(JUZ_COUNT) {
        return;
    }
    out.push(NavMatch {
        id: format!("juz:{juz}"),
        score,
        target: NavTarget::Juz(juz as u8),
    });
}

fn push_page(out: &mut Vec<NavMatch>, page: u32, score: f64) {
    if page < 1 || page > u32::from(PAGE_COUNT) {
        return;
    }
    out.push(NavMatch {
        id: format!("page:{page}"),
        score,
        target: NavTarget::Page(page as u16),
    });
}

/// `matchSajdas`: the whole table at 0.5, or one surah's / one verse's
/// sajda at 1.
pub fn match_sajdas(meta: &QuranMeta, parsed: &ParsedQuery) -> Vec<NavMatch> {
    let reference = reference_numbers(parsed);
    meta.sajdas()
        .iter()
        .filter_map(|s| {
            let score = match reference {
                Some((p, Some(sec))) => {
                    if u32::from(s.surah) == p && u32::from(s.ayah) == sec {
                        1.0
                    } else {
                        return None;
                    }
                }
                Some((p, None)) => {
                    if u32::from(s.surah) == p {
                        1.0
                    } else {
                        return None;
                    }
                }
                None => 0.5,
            };
            Some(NavMatch {
                id: format!("sajda:{}:{}", s.surah, s.ayah),
                score,
                target: NavTarget::Sajda {
                    surah: s.surah,
                    ayah: s.ayah,
                    index: s.index,
                    kind: s.kind,
                },
            })
        })
        .collect()
}

/// A place filter word split out of a query (`SplitPlace`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitPlace {
    pub place: Option<Place>,
    pub text: String,
}

/// `splitPlaceToken`: the first place word, wherever it sits.
pub fn split_place_token(raw: &str) -> SplitPlace {
    let mut place = None;
    let mut kept = Vec::new();
    for word in raw.split_whitespace() {
        let candidate = if place.is_none() {
            place_for_alias(&word.to_lowercase())
        } else {
            None
        };
        match candidate {
            Some(p) => place = Some(p),
            None => kept.push(word),
        }
    }
    SplitPlace {
        place,
        text: kept.join(" "),
    }
}

/// `isNavCandidate`: a short query that still deserves nav chips.
pub fn is_nav_candidate(raw: &str) -> bool {
    let parsed = parse_query(raw);
    if parsed.text.chars().count() < 2 {
        return false;
    }
    !parsed.numbers.is_empty() || has_keyword(&parsed, quran_aliases())
}

fn rank_and_cap(mut matches: Vec<NavMatch>, limit: usize) -> Vec<NavMatch> {
    // `sort_by` is stable: ties keep insertion order.
    matches.sort_by(|a, b| b.score.total_cmp(&a.score));
    matches.truncate(limit);
    matches
}

/// Default nav suggestion cap.
pub const NAV_LIMIT: usize = 12;

/// `matchNav`: verse refs and ranges, keyword-guarded juz/page/surah/sajda
/// targets, juz nicknames, bare-number ambiguity and the place facet.
pub fn match_nav(meta: &QuranMeta, raw: &str, limit: usize) -> Vec<NavMatch> {
    let parsed = parse_query(raw);
    if parsed.is_empty {
        return Vec::new();
    }
    let reference = reference_numbers(&parsed);
    let range = range_numbers(&parsed);
    let mut out = Vec::new();

    if has_keyword(&parsed, SAJDA_ALIASES) {
        out.extend(match_sajdas(meta, &parsed));
        return rank_and_cap(out, limit);
    }
    if has_keyword(&parsed, JUZ_ALIASES) {
        if let Some((juz, _)) = reference {
            push_juz(&mut out, juz, 1.0);
        }
        if let Some((_, juz)) = juz_keyword_for(&residual_text(&parsed, quran_aliases())) {
            push_juz(&mut out, u32::from(juz), 0.8);
        }
        return rank_and_cap(out, limit);
    }
    if has_keyword(&parsed, PAGE_ALIASES) {
        if let Some((page, _)) = reference {
            push_page(&mut out, page, 1.0);
        }
        return rank_and_cap(out, limit);
    }
    if has_keyword(&parsed, SURAH_ALIASES) {
        match (range, reference) {
            (Some(r), _) => push_ayah_range(&mut out, meta, r),
            (None, Some((s, Some(a)))) => push_ayah(&mut out, meta, s, a),
            (None, Some((s, None))) => push_surah(&mut out, meta, s, 1.0),
            (None, None) => {}
        }
        return rank_and_cap(out, limit);
    }
    if let Some(r) = range {
        push_ayah_range(&mut out, meta, r);
        return rank_and_cap(out, limit);
    }
    if let Some((s, Some(a))) = reference {
        push_ayah(&mut out, meta, s, a);
        return rank_and_cap(out, limit);
    }
    if let Some((n, None)) = reference
        && is_bare_number(&parsed)
    {
        push_surah(&mut out, meta, n, 1.0);
        push_juz(&mut out, n, 0.9);
        push_page(&mut out, n, 0.8);
        return rank_and_cap(out, limit);
    }
    if let Some((_, juz)) = juz_keyword_for(&residual_text(&parsed, quran_aliases())) {
        push_juz(&mut out, u32::from(juz), 0.8);
    }
    if let Some(place) = split_place_token(&parsed.text).place {
        let surah_count = meta.surahs().iter().filter(|s| s.place == place).count();
        out.push(NavMatch {
            id: format!("place:{}", place_id(place)),
            score: 0.85,
            target: NavTarget::Place { place, surah_count },
        });
    }
    rank_and_cap(out, limit)
}

/// `meccan` / `medinan`.
pub fn place_id(place: Place) -> &'static str {
    match place {
        Place::Meccan => "meccan",
        Place::Medinan => "medinan",
    }
}
