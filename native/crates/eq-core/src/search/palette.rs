//! The command palette's sources, scoring, dedupe and grouping
//! (`search/palette/**`). Pure: the app passes the parsed query and its
//! context in, runs the two text sources (Quran text, translation text)
//! off the engine thread through the corpora, and maps their hits through
//! [`quran_text_entries`] / [`translation_text_entries`].
//!
//! Source order is the web's `BUILTIN_PALETTE_SOURCES`: earlier sources win
//! dedupe ties, so exact references outrank fuzzy and full-text hits.

use crate::{
    i18n::{Key, keys},
    quran::meta::{JUZ_COUNT, PAGE_COUNT, Place, QuranMeta, SURAH_COUNT, SajdaKind, Surah},
    routes::{Link, ReaderLink, ReaderScope, ReaderTarget, StaticPage, TranslationRef},
};

use super::{
    corpus::{Highlight, HitKind, SearchHit},
    latin::contains_arabic_script,
    nav::{
        AyahRange, JUZ_ALIASES, NavTarget, PAGE_ALIASES, ParsedQuery, SAJDA_ALIASES, SURAH_ALIASES,
        all_keyword_aliases, arabic_terms_for, has_keyword, is_bare_number, juz_keyword_for,
        match_sajdas, quran_aliases, range_numbers, reference_numbers, residual_text,
        split_place_token, strip_trailing_ref, terms_for,
    },
    normalize::MIN_QUERY_LEN,
    suggest::{rank_surahs, score_fields},
};

/// A row heading (`PaletteGroups`), ordered by [`Group::order`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    JumpTo,
    RecentQueries,
    QuranText,
    TranslationText,
    Surahs,
    Ranges,
    Sajdas,
    Settings,
    AppPages,
    Site,
    /// Native: the shell's own destinations (sidebar pages the web reaches
    /// through its navigation instead), so the palette still reaches every
    /// page. Not a web group.
    Destinations,
    Actions,
}

impl Group {
    /// Every group, in order.
    pub const ALL: [Group; 12] = [
        Group::JumpTo,
        Group::RecentQueries,
        Group::QuranText,
        Group::TranslationText,
        Group::Surahs,
        Group::Ranges,
        Group::Sajdas,
        Group::Settings,
        Group::AppPages,
        Group::Site,
        Group::Destinations,
        Group::Actions,
    ];

    /// The web's group id.
    pub fn id(self) -> &'static str {
        match self {
            Group::JumpTo => "jump-to",
            Group::RecentQueries => "recent-queries",
            Group::QuranText => "quran-text",
            Group::TranslationText => "translation-text",
            Group::Surahs => "surahs",
            Group::Ranges => "ranges",
            Group::Sajdas => "sajdas",
            Group::Settings => "settings",
            Group::AppPages => "app-pages",
            Group::Site => "site",
            Group::Destinations => "go-to",
            Group::Actions => "actions",
        }
    }

    /// Ascending: lower sorts nearer the top.
    pub fn order(self) -> u16 {
        match self {
            Group::JumpTo => 10,
            Group::RecentQueries => 15,
            Group::QuranText => 20,
            Group::TranslationText => 25,
            Group::Surahs => 30,
            Group::Ranges => 40,
            Group::Sajdas => 45,
            Group::Settings => 55,
            Group::AppPages => 56,
            Group::Site => 90,
            Group::Destinations => 95,
            Group::Actions => 100,
        }
    }

    /// The heading's message; `None` for the native-only group.
    pub fn label_key(self) -> Option<Key> {
        Some(match self {
            Group::JumpTo => keys::PALETTE_GROUP_JUMP_TO,
            Group::RecentQueries => keys::PALETTE_GROUP_RECENT_QUERIES,
            Group::QuranText => keys::PALETTE_GROUP_QURAN_TEXT,
            Group::TranslationText => keys::PALETTE_GROUP_TRANSLATION_TEXT,
            Group::Surahs => keys::PALETTE_GROUP_SURAHS,
            Group::Ranges => keys::PALETTE_GROUP_RANGES,
            Group::Sajdas => keys::PALETTE_GROUP_SAJDAS,
            Group::Settings => keys::PALETTE_GROUP_SETTINGS,
            Group::AppPages => keys::PALETTE_GROUP_APP_PAGES,
            Group::Site => keys::PALETTE_GROUP_SITE,
            Group::Actions => keys::PALETTE_GROUP_ACTIONS,
            Group::Destinations => return None,
        })
    }
}

/// A row's icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Book,
    Rows,
    Bookmark,
    Search,
    Note,
    Moon,
    Continuous,
}

/// What choosing a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Opens a page.
    Link(Link),
    /// Opens a verse: the reader marks a search entry first (`openVerse`).
    Verse { link: ReaderLink, surah: u16 },
    /// Resumes reading thread `thread` at `link`.
    Resume { link: ReaderLink, thread: String },
    /// Fills the palette's query and keeps it open (a recent search).
    FillQuery(String),
    /// Flips dark and light.
    ToggleTheme,
}

/// A full-text hit's body preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Verbatim.
    pub text: String,
    pub highlights: Vec<Highlight>,
    /// Arabic (right to left) or a translation's direction.
    pub rtl: bool,
    /// Arabic Quran text (drawn with the Quran face).
    pub quran: bool,
}

/// One palette row (`PaletteEntry`).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Unique across the list.
    pub id: String,
    pub source: &'static str,
    pub group: Group,
    pub label: String,
    pub detail: Option<String>,
    /// Right-aligned Arabic label (a surah's name).
    pub arabic: Option<String>,
    pub preview: Option<Preview>,
    pub icon: Icon,
    /// `[0, 1]`.
    pub score: f64,
    pub target: Target,
    /// Cross-source identity: only the first entry per key renders.
    pub dedupe_key: Option<String>,
}

impl Entry {
    /// The palette stays open after choosing it.
    pub fn keep_open(&self) -> bool {
        matches!(self.target, Target::FillQuery(_))
    }
}

/// Where "Continue reading" resumes (`sessions.current ?? latest`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    pub thread: String,
    pub surah: u16,
    pub ayah: u16,
    /// The thread's own source when it is a translation, else `None`.
    pub source_id: Option<String>,
}

/// What every source reads (`PaletteQuery` plus the stores they consult).
#[derive(Debug, Clone)]
pub struct Context<'a> {
    pub meta: &'a QuranMeta,
    pub parsed: ParsedQuery,
    /// The reader's translation context, kept across jumps.
    pub route: Option<TranslationRef>,
    /// Recent searches, newest first.
    pub recents: Vec<String>,
    pub resume: Option<Resume>,
    pub authenticated: bool,
}

/// `surahDetail`.
pub fn surah_detail(s: &Surah) -> String {
    // i18n: palette surah detail (English on the web too).
    format!(
        "{} · {} · {} verses",
        s.transliteration, s.meaning, s.ayah_count
    )
}

fn reader(scope: ReaderScope, route: &Option<TranslationRef>, ayah: Option<u16>) -> ReaderLink {
    ReaderLink::new(ReaderTarget {
        scope,
        translation: route.clone(),
        ayah,
    })
}

/// `surahHref`.
pub fn surah_link(route: &Option<TranslationRef>, surah: u16) -> ReaderLink {
    reader(ReaderScope::Surah(surah), route, None)
}

/// `ayahHref`: `None` when the verse has no reachable page.
pub fn ayah_link(
    meta: &QuranMeta,
    route: &Option<TranslationRef>,
    surah: u16,
    ayah: u16,
) -> Option<ReaderLink> {
    meta.surah_local_page_for_ayah(surah, ayah)?;
    Some(reader(ReaderScope::Surah(surah), route, Some(ayah)))
}

/// `juzHref`.
pub fn juz_link(route: &Option<TranslationRef>, juz: u8) -> ReaderLink {
    reader(ReaderScope::Juz(juz), route, None)
}

/// `pageHref`.
pub fn page_link(route: &Option<TranslationRef>, page: u16) -> ReaderLink {
    reader(ReaderScope::Page(page), route, None)
}

fn in_range(n: u32, max: u32) -> bool {
    (1..=max).contains(&n)
}

#[allow(clippy::too_many_arguments)]
fn reader_entry(
    id: String,
    source: &'static str,
    group: Group,
    label: String,
    detail: &str,
    link: ReaderLink,
    score: f64,
    dedupe: String,
) -> Entry {
    Entry {
        id,
        source,
        group,
        label,
        detail: Some(detail.to_owned()),
        arabic: None,
        preview: None,
        icon: Icon::Rows,
        score,
        target: Target::Link(Link::Reader(link)),
        dedupe_key: Some(dedupe),
    }
}

// ---- quran.reference ----------------------------------------------------

const REFERENCE: &str = "quran.reference";

fn reference_surah(ctx: &Context<'_>, num: u32, score: f64) -> Option<Entry> {
    let s = ctx.meta.surah(u16::try_from(num).ok()?)?;
    Some(Entry {
        id: format!("{REFERENCE}:surah:{num}"),
        source: REFERENCE,
        group: Group::JumpTo,
        label: format!("{}. {}", s.num, s.name),
        detail: Some(surah_detail(s)),
        arabic: Some(s.arabic.clone()),
        preview: None,
        icon: Icon::Book,
        score,
        target: Target::Link(Link::Reader(surah_link(&ctx.route, s.num))),
        dedupe_key: Some(format!("surah:{num}")),
    })
}

#[allow(clippy::too_many_arguments)]
fn verse_entry(
    ctx: &Context<'_>,
    source: &'static str,
    id: String,
    s: &Surah,
    ayah: u16,
    label: String,
    detail: &str,
    dedupe: String,
) -> Option<Entry> {
    let link = ayah_link(ctx.meta, &ctx.route, s.num, ayah)?;
    Some(Entry {
        id,
        source,
        group: Group::JumpTo,
        label,
        detail: Some(detail.to_owned()),
        arabic: Some(s.arabic.clone()),
        preview: None,
        icon: Icon::Book,
        score: 1.0,
        target: Target::Verse { link, surah: s.num },
        dedupe_key: Some(dedupe),
    })
}

fn reference_ayah(ctx: &Context<'_>, num: u32, ayah: u32) -> Option<Entry> {
    let s = ctx.meta.surah(u16::try_from(num).ok()?)?;
    if ayah > u32::from(s.ayah_count) {
        return None;
    }
    let ayah = u16::try_from(ayah).ok()?;
    verse_entry(
        ctx,
        REFERENCE,
        format!("{REFERENCE}:ayah:{num}:{ayah}"),
        s,
        ayah,
        format!("{} {num}:{ayah}", s.name),
        "Verse", // i18n: palette (English on the web)
        format!("ayah:{num}:{ayah}"),
    )
}

fn reference_range(ctx: &Context<'_>, r: AyahRange) -> Option<Entry> {
    let start = ctx.meta.surah(u16::try_from(r.start_surah).ok()?)?;
    let end = ctx.meta.surah(u16::try_from(r.end_surah).ok()?)?;
    if r.end_ayah < 1 || r.end_ayah > u32::from(end.ayah_count) {
        return None;
    }
    let (sa, ea) = (
        u16::try_from(r.start_ayah).ok()?,
        u16::try_from(r.end_ayah).ok()?,
    );
    let from = ctx.meta.global_index_of(start.num, sa)?;
    let to = ctx.meta.global_index_of(end.num, ea)?;
    if to < from {
        return None;
    }
    let tail = if r.start_surah == r.end_surah {
        format!("{}", r.end_ayah)
    } else {
        format!("{}:{}", r.end_surah, r.end_ayah)
    };
    let span = format!("{}:{}-{tail}", r.start_surah, r.start_ayah);
    verse_entry(
        ctx,
        REFERENCE,
        format!("{REFERENCE}:range:{span}"),
        start,
        sa,
        format!("{} {span}", start.name),
        "Verse range", // i18n: palette (English on the web)
        format!("range:{span}"),
    )
}

fn juz_entry(source: &'static str, id: String, ctx: &Context<'_>, n: u8, score: f64) -> Entry {
    reader_entry(
        id,
        source,
        Group::JumpTo,
        format!("Juz {n}"), // i18n: palette (English on the web)
        "Juz",
        juz_link(&ctx.route, n),
        score,
        format!("juz:{n}"),
    )
}

fn page_entry(source: &'static str, id: String, ctx: &Context<'_>, n: u16, score: f64) -> Entry {
    reader_entry(
        id,
        source,
        Group::JumpTo,
        format!("Page {n}"), // i18n: palette (English on the web)
        "Mushaf page",
        page_link(&ctx.route, n),
        score,
        format!("page:{n}"),
    )
}

/// `quranReferenceSource`: `2:255`, `2 255`, `surah 18`, `juz 5`, `p 100`,
/// ranges, and a bare number offering surah, juz and page at once.
fn reference_entries(ctx: &Context<'_>) -> Vec<Entry> {
    let p = &ctx.parsed;
    let Some((primary, secondary)) = reference_numbers(p) else {
        return Vec::new();
    };
    if let Some(range) = range_numbers(p)
        && (p.keyword.is_none() || has_keyword(p, SURAH_ALIASES))
    {
        return reference_range(ctx, range).into_iter().collect();
    }
    let mut out = Vec::new();
    let juz = |n: u32, score: f64| {
        juz_entry(
            REFERENCE,
            format!("{REFERENCE}:juz:{n}"),
            ctx,
            n as u8,
            score,
        )
    };
    let page = |n: u32, score: f64| {
        page_entry(
            REFERENCE,
            format!("{REFERENCE}:page:{n}"),
            ctx,
            n as u16,
            score,
        )
    };
    if has_keyword(p, SURAH_ALIASES) {
        if !in_range(primary, u32::from(SURAH_COUNT)) {
            return out;
        }
        out.extend(match secondary {
            None => reference_surah(ctx, primary, 1.0),
            Some(a) => reference_ayah(ctx, primary, a),
        });
    } else if has_keyword(p, JUZ_ALIASES) {
        if in_range(primary, u32::from(JUZ_COUNT)) {
            out.push(juz(primary, 1.0));
        }
    } else if has_keyword(p, PAGE_ALIASES) {
        if in_range(primary, u32::from(PAGE_COUNT)) {
            out.push(page(primary, 1.0));
        }
    } else if p.keyword.is_none() {
        match secondary {
            Some(a) => {
                if in_range(primary, u32::from(SURAH_COUNT)) {
                    out.extend(reference_ayah(ctx, primary, a));
                }
            }
            None if is_bare_number(p) => {
                if in_range(primary, u32::from(SURAH_COUNT)) {
                    out.extend(reference_surah(ctx, primary, 1.0));
                }
                if in_range(primary, u32::from(JUZ_COUNT)) {
                    out.push(juz(primary, 0.9));
                }
                if in_range(primary, u32::from(PAGE_COUNT)) {
                    out.push(page(primary, 0.8));
                }
            }
            None => {}
        }
    }
    out
}

// ---- quran.surahs ---------------------------------------------------------

const SURAHS: &str = "quran.surahs";
const IDLE_SUGGESTIONS: usize = 5;
const SURAHS_LIMIT: usize = 7;

fn surah_row(ctx: &Context<'_>, s: &Surah, score: f64) -> Entry {
    Entry {
        id: format!("{SURAHS}:surah:{}", s.num),
        source: SURAHS,
        group: Group::Surahs,
        label: format!("{}. {}", s.num, s.name),
        detail: Some(surah_detail(s)),
        arabic: Some(s.arabic.clone()),
        preview: None,
        icon: Icon::Book,
        score,
        target: Target::Link(Link::Reader(surah_link(&ctx.route, s.num))),
        dedupe_key: Some(format!("surah:{}", s.num)),
    }
}

/// `quranSurahsSource`: names in any script, a place filter, and a name
/// plus a verse number promoted into "Jump to".
fn surah_entries(ctx: &Context<'_>) -> Vec<Entry> {
    let p = &ctx.parsed;
    if p.is_empty {
        return ctx
            .meta
            .surahs()
            .iter()
            .take(IDLE_SUGGESTIONS)
            .map(|s| surah_row(ctx, s, 0.0))
            .collect();
    }
    let placed = split_place_token(terms_for(p, SURAH_ALIASES));
    let needle = strip_trailing_ref(&placed.text);
    if let Some(place) = placed.place
        && needle.is_empty()
    {
        return in_place(ctx.meta, place)
            .take(SURAHS_LIMIT)
            .map(|s| surah_row(ctx, s, 0.0))
            .collect();
    }
    if needle.is_empty() {
        return Vec::new();
    }
    let arabic_needle =
        strip_trailing_ref(&split_place_token(&arabic_terms_for(p, SURAH_ALIASES)).text);
    let mut ranked = rank_surahs(ctx.meta, &needle, &arabic_needle);
    if let Some(place) = placed.place {
        ranked.retain(|r| r.surah.place == place);
    }
    let Some(best) = ranked.first().map(|r| r.surah) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // A name plus one trailing number pins an exact verse in the best match.
    if placed.text != needle
        && let Some((ayah, None)) = reference_numbers(p)
        && ayah <= u32::from(best.ayah_count)
        && let Ok(ayah) = u16::try_from(ayah)
    {
        out.extend(verse_entry(
            ctx,
            SURAHS,
            format!("{SURAHS}:ayah:{}:{ayah}", best.num),
            best,
            ayah,
            format!("{} {}:{ayah}", best.name, best.num),
            "Verse", // i18n: palette (English on the web)
            format!("ayah:{}:{ayah}", best.num),
        ));
    }
    out.extend(ranked.iter().map(|r| surah_row(ctx, r.surah, r.score)));
    out
}

fn in_place(meta: &QuranMeta, place: Place) -> impl Iterator<Item = &Surah> {
    meta.surahs().iter().filter(move |s| s.place == place)
}

// ---- search.recents -------------------------------------------------------

const RECENTS: &str = "search.recents";
const RECENTS_LIMIT: usize = 5;

/// `searchRecentsSource`: on an empty query only; choosing one fills it.
fn recent_entries(ctx: &Context<'_>) -> Vec<Entry> {
    if !ctx.parsed.is_empty {
        return Vec::new();
    }
    ctx.recents
        .iter()
        .map(|q| Entry {
            id: format!("{RECENTS}:{q}"),
            source: RECENTS,
            group: Group::RecentQueries,
            label: q.clone(),
            detail: None,
            arabic: None,
            preview: None,
            icon: Icon::Search,
            score: 0.0,
            target: Target::FillQuery(q.clone()),
            dedupe_key: None,
        })
        .collect()
}

// ---- quran.ranges ---------------------------------------------------------

const RANGES: &str = "quran.ranges";
const RANGES_LIMIT: usize = 6;

fn title_case(alias: &str) -> String {
    alias
        .split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `quranRangesSource`: browsable juz/page lists for a bare keyword, and a
/// juz nickname ahead of everything.
fn range_entries(ctx: &Context<'_>) -> Vec<Entry> {
    let p = &ctx.parsed;
    let wants_juz = has_keyword(p, JUZ_ALIASES);
    let wants_pages = has_keyword(p, PAGE_ALIASES);
    let nickname = juz_keyword_for(&residual_text(p, quran_aliases()));
    let enabled = p.numbers.is_empty() && (wants_juz || wants_pages || nickname.is_some());
    if !enabled {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some((alias, n)) = nickname {
        let mut e = reader_entry(
            format!("{RANGES}:juz:nickname:{n}"),
            RANGES,
            Group::Ranges,
            format!("Juz {n} ({})", title_case(alias)), // i18n: palette
            "Juz",
            juz_link(&ctx.route, n),
            0.8,
            format!("juz:{n}"),
        );
        e.icon = Icon::Rows;
        out.push(e);
    }
    if wants_juz {
        for n in 1..=RANGES_LIMIT.min(usize::from(JUZ_COUNT)) as u8 {
            out.push(reader_entry(
                format!("{RANGES}:juz:{n}"),
                RANGES,
                Group::Ranges,
                format!("Juz {n}"), // i18n: palette
                "Juz",
                juz_link(&ctx.route, n),
                0.5,
                format!("juz:{n}"),
            ));
        }
    } else if wants_pages {
        for n in 1..=RANGES_LIMIT.min(usize::from(PAGE_COUNT)) as u16 {
            out.push(reader_entry(
                format!("{RANGES}:page:{n}"),
                RANGES,
                Group::Ranges,
                format!("Page {n}"), // i18n: palette
                "Mushaf page",
                page_link(&ctx.route, n),
                0.5,
                format!("page:{n}"),
            ));
        }
    }
    out
}

// ---- quran.sajdas ---------------------------------------------------------

const SAJDAS: &str = "quran.sajdas";

/// "Recommended prostration" / "Obligatory prostration".
pub fn sajda_detail(kind: SajdaKind) -> &'static str {
    match kind {
        SajdaKind::Obligatory => "Obligatory prostration",
        SajdaKind::Recommended => "Recommended prostration",
    }
}

/// `quranSajdasSource`: behind the sajda keyword.
fn sajda_entries(ctx: &Context<'_>) -> Vec<Entry> {
    if !has_keyword(&ctx.parsed, SAJDA_ALIASES) {
        return Vec::new();
    }
    match_sajdas(ctx.meta, &ctx.parsed)
        .into_iter()
        .filter_map(|m| {
            let NavTarget::Sajda {
                surah, ayah, kind, ..
            } = m.target
            else {
                return None;
            };
            let s = ctx.meta.surah(surah)?;
            let link = ayah_link(ctx.meta, &ctx.route, surah, ayah)?;
            Some(Entry {
                id: format!("{SAJDAS}:{surah}:{ayah}"),
                source: SAJDAS,
                group: Group::Sajdas,
                label: format!("{} {surah}:{ayah}", s.name),
                detail: Some(sajda_detail(kind).to_owned()),
                arabic: Some(s.arabic.clone()),
                preview: None,
                icon: Icon::Bookmark,
                score: m.score,
                target: Target::Verse { link, surah },
                dedupe_key: Some(format!("sajda:{surah}:{ayah}")),
            })
        })
        .collect()
}

// ---- settings / search / site routes -------------------------------------

#[allow(clippy::too_many_arguments)]
fn page_row(
    source: &'static str,
    group: Group,
    key: &str,
    label: &str,
    detail: &str,
    keywords: &[&str],
    icon: Icon,
    page: StaticPage,
    parsed: &ParsedQuery,
) -> Option<Entry> {
    let mut fields = vec![label];
    fields.extend_from_slice(keywords);
    let score = if parsed.is_empty {
        0.0
    } else {
        score_fields(&fields, &parsed.text)
    };
    if !parsed.is_empty && score == 0.0 {
        return None;
    }
    Some(Entry {
        id: format!("{source}:{key}"),
        source,
        group,
        label: label.to_owned(),
        detail: Some(detail.to_owned()),
        arabic: None,
        preview: None,
        icon,
        score,
        target: Target::Link(Link::Page(page)),
        dedupe_key: Some(format!("href:{key}")),
    })
}

/// `settingsRoutesSource`.
fn settings_entries(ctx: &Context<'_>) -> Vec<Entry> {
    // i18n: palette labels stay English on the web.
    page_row(
        "settings.routes",
        Group::Settings,
        "/settings",
        "Settings",
        "Storage, appearance, reading, privacy",
        &[
            "storage",
            "appearance",
            "fonts",
            "theme",
            "privacy",
            "account",
        ],
        Icon::Rows,
        StaticPage::Settings,
        &ctx.parsed,
    )
    .into_iter()
    .collect()
}

/// `searchRoutesSource`.
fn search_route_entries(ctx: &Context<'_>) -> Vec<Entry> {
    page_row(
        "search.routes",
        Group::AppPages,
        "/search",
        "Search",
        "Full-text Arabic and translations",
        &["find", "translations", "lookup", "quran"],
        Icon::Search,
        StaticPage::Search,
        &ctx.parsed,
    )
    .into_iter()
    .collect()
}

/// `siteRoutesSource`: the reader, the marketing pages, account or sign in.
fn site_entries(ctx: &Context<'_>) -> Vec<Entry> {
    let account: (&str, &str, &[&str], StaticPage) = if ctx.authenticated {
        (
            "/account",
            "Account",
            &["profile", "settings"],
            StaticPage::Account,
        )
    } else {
        (
            "/login",
            "Sign in",
            &["login", "sign up", "account"],
            StaticPage::Login,
        )
    };
    let routes: [(&str, &str, &str, &[&str], StaticPage); 8] = [
        (
            "/surah",
            "Read the Quran",
            "Reader",
            &["reader", "read", "mushaf", "quran", "open app"],
            StaticPage::SurahIndex,
        ),
        ("/", "Home", "Page", &["home"], StaticPage::Home),
        ("/about", "About", "Page", &["about"], StaticPage::About),
        ("/faq", "FAQ", "Page", &["faq"], StaticPage::Faq),
        (
            "/contact",
            "Contact",
            "Page",
            &["contact"],
            StaticPage::Contact,
        ),
        (
            "/privacy",
            "Privacy",
            "Page",
            &["privacy"],
            StaticPage::Privacy,
        ),
        ("/terms", "Terms", "Page", &["terms"], StaticPage::Terms),
        (account.0, account.1, "Page", account.2, account.3),
    ];
    let mut out: Vec<Entry> = routes
        .into_iter()
        .filter_map(|(href, label, detail, keywords, page)| {
            let mut fields = vec![href];
            fields.extend_from_slice(keywords);
            let mut e = page_row(
                "site.routes",
                Group::Site,
                href,
                label,
                detail,
                &fields,
                Icon::Note,
                page,
                &ctx.parsed,
            )?;
            e.id = format!("site.routes:{href}");
            Some(e)
        })
        .collect();
    by_score(&mut out);
    out
}

// ---- app.actions ----------------------------------------------------------

const ACTIONS: &str = "app.actions";
const CONTINUE_KEYWORDS: [&str; 4] = [
    "continue reading",
    "resume",
    "last read",
    "where i left off",
];

fn continue_reading(ctx: &Context<'_>) -> Option<Entry> {
    let resume = ctx.resume.as_ref()?;
    let s = ctx.meta.surah(resume.surah)?;
    let score = if ctx.parsed.is_empty {
        0.0
    } else {
        score_fields(&CONTINUE_KEYWORDS, &ctx.parsed.text)
    };
    if !ctx.parsed.is_empty && score == 0.0 {
        return None;
    }
    // `resumeCtxFor`: the thread's own translation, else the route's.
    let route = match &resume.source_id {
        Some(id) => Some(TranslationRef::from_id(id)),
        None => ctx.route.clone(),
    };
    let link = ayah_link(ctx.meta, &route, resume.surah, resume.ayah)
        .unwrap_or_else(|| surah_link(&route, resume.surah));
    Some(Entry {
        id: format!("{ACTIONS}:continue-reading"),
        source: ACTIONS,
        group: Group::JumpTo,
        label: "Continue reading".into(), // i18n: palette
        detail: Some(format!("{} {}:{}", s.name, resume.surah, resume.ayah)),
        arabic: None,
        preview: None,
        icon: Icon::Continuous,
        score,
        target: Target::Resume {
            link,
            thread: resume.thread.clone(),
        },
        dedupe_key: Some(format!("ayah:{}:{}", resume.surah, resume.ayah)),
    })
}

/// `appActionsSource`: continue reading, toggle theme.
fn action_entries(ctx: &Context<'_>) -> Vec<Entry> {
    let mut out: Vec<Entry> = continue_reading(ctx).into_iter().collect();
    let p = &ctx.parsed;
    let fields = [
        "Toggle theme",
        "dark mode",
        "light mode",
        "appearance",
        "theme",
    ];
    let score = if p.is_empty {
        0.0
    } else {
        score_fields(&fields, &p.text)
    };
    if p.is_empty || score > 0.0 {
        out.push(Entry {
            id: format!("{ACTIONS}:toggle-theme"),
            source: ACTIONS,
            group: Group::Actions,
            label: "Toggle theme".into(), // i18n: palette
            detail: Some("Switch between dark and light".into()),
            arabic: None,
            preview: None,
            icon: Icon::Moon,
            score,
            target: Target::ToggleTheme,
            dedupe_key: None,
        });
    }
    by_score(&mut out);
    out
}

fn by_score(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.score.total_cmp(&a.score));
}

// ---- full-text sources ----------------------------------------------------

/// Hits the Quran text source asks for (`LIMIT`).
pub const TEXT_LIMIT: usize = 8;

/// `quranTextSource.enabled`: real free text, not a coordinate.
pub fn quran_text_enabled(parsed: &ParsedQuery) -> bool {
    residual_text(parsed, quran_aliases()).chars().count() >= MIN_QUERY_LEN
}

/// The text the translation source searches (`searchableText`).
pub fn translation_query(parsed: &ParsedQuery) -> String {
    residual_text(parsed, all_keyword_aliases())
}

/// `translationTextSource.enabled` for a scope of direction `rtl`.
pub fn translation_text_enabled(parsed: &ParsedQuery, scope_rtl: Option<bool>) -> bool {
    let text = translation_query(parsed);
    text.chars().count() >= MIN_QUERY_LEN
        && !contains_arabic_script(&text)
        && scope_rtl == Some(false)
}

/// Quran text hits as rows.
pub fn quran_text_entries(ctx: &Context<'_>, hits: &[SearchHit]) -> Vec<Entry> {
    hits.iter()
        .filter_map(|hit| {
            let s = ctx.meta.surah(hit.surah)?;
            let link = ayah_link(ctx.meta, &ctx.route, hit.surah, hit.ayah)?;
            let opener = hit.kind == HitKind::Opener;
            let (label, detail, dedupe) = if opener {
                (
                    format!("{} · Surah opener", s.name), // i18n: palette
                    Some("Opener".to_owned()),
                    format!("opener:{}", hit.surah),
                )
            } else {
                (
                    format!("{} {}:{}", s.name, hit.surah, hit.ayah),
                    None,
                    format!("ayah:{}:{}", hit.surah, hit.ayah),
                )
            };
            Some(Entry {
                id: format!("quran.text:{}", hit.key()),
                source: "quran.text",
                group: Group::QuranText,
                label,
                detail,
                arabic: None,
                preview: (!hit.text.is_empty()).then(|| Preview {
                    text: hit.text.clone(),
                    highlights: hit.highlights.clone(),
                    rtl: true,
                    quran: true,
                }),
                icon: Icon::Search,
                score: 0.7,
                target: Target::Verse {
                    link,
                    surah: hit.surah,
                },
                dedupe_key: Some(dedupe),
            })
        })
        .collect()
}

/// Translation hits of source `scope` as rows; they open in that
/// translation.
pub fn translation_text_entries(
    ctx: &Context<'_>,
    scope: &str,
    detail: Option<&str>,
    rtl: bool,
    hits: &[SearchHit],
) -> Vec<Entry> {
    let route = Some(TranslationRef::from_id(scope));
    hits.iter()
        .filter_map(|hit| {
            let s = ctx.meta.surah(hit.surah)?;
            let link = ayah_link(ctx.meta, &route, hit.surah, hit.ayah)?;
            Some(Entry {
                id: format!("translation.text:tayah:{scope}:{}:{}", hit.surah, hit.ayah),
                source: "translation.text",
                group: Group::TranslationText,
                label: format!("{} {}:{}", s.name, hit.surah, hit.ayah),
                detail: detail.map(str::to_owned),
                arabic: None,
                preview: (!hit.text.is_empty()).then(|| Preview {
                    text: hit.text.clone(),
                    highlights: hit.highlights.clone(),
                    rtl,
                    quran: false,
                }),
                icon: Icon::Search,
                score: 0.7,
                target: Target::Verse {
                    link,
                    surah: hit.surah,
                },
                dedupe_key: Some(format!("tayah:{scope}:{}:{}", hit.surah, hit.ayah)),
            })
        })
        .collect()
}

// ---- registry -------------------------------------------------------------

/// The in-memory sources, in registry order, each capped at its limit
/// (`collectSyncEntries`). The two full-text sources sit between the
/// recents and the ranges; their rows go after these (`[...sync, ...async]`).
pub fn sync_entries(ctx: &Context<'_>) -> Vec<Entry> {
    type Source = fn(&Context<'_>) -> Vec<Entry>;
    let sources: [(Source, usize); 9] = [
        (reference_entries, 4),
        (surah_entries, SURAHS_LIMIT),
        (recent_entries, RECENTS_LIMIT),
        (range_entries, RANGES_LIMIT),
        (sajda_entries, 15),
        (settings_entries, 2),
        (search_route_entries, 1),
        (site_entries, 6),
        (action_entries, 5),
    ];
    let mut out = Vec::new();
    for (source, limit) in sources {
        out.extend(source(ctx).into_iter().take(limit));
    }
    out
}

/// `dedupeEntries`: drops later rows sharing an id or a target.
pub fn dedupe(entries: Vec<Entry>) -> Vec<Entry> {
    let mut ids = std::collections::HashSet::new();
    let mut targets = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|e| {
            if ids.contains(&e.id) {
                return false;
            }
            let target = e.dedupe_key.clone().unwrap_or_else(|| e.id.clone());
            if targets.contains(&target) {
                return false;
            }
            ids.insert(e.id.clone());
            targets.insert(target);
            true
        })
        .collect()
}

/// One group's rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub group: Group,
    pub entries: Vec<Entry>,
}

/// `sectionsFor`: groups by strongest match, then designed order.
pub fn sections(entries: Vec<Entry>) -> Vec<Section> {
    let mut out: Vec<Section> = Group::ALL
        .iter()
        .map(|g| Section {
            group: *g,
            entries: Vec::new(),
        })
        .collect();
    for e in entries {
        if let Some(s) = out.iter_mut().find(|s| s.group == e.group) {
            s.entries.push(e);
        }
    }
    out.retain(|s| !s.entries.is_empty());
    let top = |s: &Section| s.entries.iter().map(|e| e.score).fold(0.0, f64::max);
    out.sort_by(|a, b| {
        top(b)
            .total_cmp(&top(a))
            .then(a.group.order().cmp(&b.group.order()))
    });
    out
}
