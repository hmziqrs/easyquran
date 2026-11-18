//! Reader URL grammar (PARITY A2), both ways: parse a web path (+ query +
//! fragment), an `easyquran://` deep link or an `https://easyquran.fyi`
//! share link into a [`Link`], and build the canonical path back. Mirror of
//! `web/src/lib/data/quran.ts` (`surah*For` family, `routeContextFromParams`),
//! `lib/reader/{mode-param,more-param,translation-nav}.ts`, `lib/i18n/reader.ts`
//! (segment grammar) and the reader's `requestedAyah` (`#ayah-s-n`, `?v=`,
//! `?ayah=S:N-S:M`, `?verse=`). An unknown path parses to `None`: the app
//! opens it in the browser, never an error screen.

use serde::{Deserialize, Serialize};

use crate::quran::meta::{HIZB_COUNT, JUZ_COUNT, PAGE_COUNT, QuranMeta, meta};

/// A translation source as the URL names it: `/t/{lang}/{translator}`.
/// The source id is `lang.translator` (`translationIdFromSegments`); the
/// first segment is the id's prefix, not always a language
/// (`quranenc.en.hilali_khan` -> `quranenc` / `en.hilali_khan`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TranslationRef {
    pub lang: String,
    pub translator: String,
}

impl TranslationRef {
    /// Segments of a source id (`translationSegmentsFromId`).
    pub fn from_id(id: &str) -> Self {
        match id.split_once('.') {
            Some((lang, translator)) => Self {
                lang: lang.to_owned(),
                translator: translator.to_owned(),
            },
            None => Self {
                lang: id.to_owned(),
                translator: String::new(),
            },
        }
    }

    /// The source id (`translationIdFromSegments`).
    pub fn id(&self) -> String {
        if self.translator.is_empty() {
            return self.lang.clone();
        }
        format!("{}.{}", self.lang, self.translator)
    }
}

/// Which slice of the mushaf a reader shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReaderScope {
    /// Surah 1..=114.
    Surah(u16),
    /// Mushaf page 1..=604.
    Page(u16),
    /// Juz 1..=30.
    Juz(u8),
    /// Hizb 1..=60.
    Hizb(u8),
}

/// One reader route: scope, optional translation, optional ayah anchor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReaderTarget {
    pub scope: ReaderScope,
    pub translation: Option<TranslationRef>,
    pub ayah: Option<u16>,
}

impl ReaderTarget {
    /// The Arabic-only reader of `scope`, at its start.
    pub fn arabic(scope: ReaderScope) -> Self {
        Self {
            scope,
            translation: None,
            ayah: None,
        }
    }
}

/// Reader mode (`?mode=`; `ReaderMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReaderMode {
    Verse,
    Reading,
}

impl ReaderMode {
    /// The param value.
    pub fn as_str(self) -> &'static str {
        match self {
            ReaderMode::Verse => "verse",
            ReaderMode::Reading => "reading",
        }
    }

    /// `parseModeParam`: a known value or `None`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "verse" => Some(ReaderMode::Verse),
            "reading" => Some(ReaderMode::Reading),
            _ => None,
        }
    }
}

/// A reader link: the target plus the view params the URL carries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReaderLink {
    pub target: ReaderTarget,
    /// `?mode=`.
    pub mode: Option<ReaderMode>,
    /// `?more=`: stacked extra translation ids, trimmed, deduplicated.
    pub more: Vec<String>,
}

impl ReaderLink {
    /// A link to `target` with no view params.
    pub fn new(target: ReaderTarget) -> Self {
        Self {
            target,
            mode: None,
            more: Vec::new(),
        }
    }
}

/// A non-reader page of the web app.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StaticPage {
    Home,
    SurahIndex,
    JuzIndex,
    PagesIndex,
    Azkar,
    Search,
    Bookmarks,
    Yours,
    YoursCard(String),
    DailyAyah,
    Downloads,
    Settings,
    Account,
    Login,
    Register,
    VerifyEmail,
    ForgotPassword,
    Appeal(Option<String>),
    Report,
    /// `/newsletter/confirm#email=…&token=…` (the fragment carries both).
    NewsletterConfirm {
        email: String,
        token: String,
    },
    About,
    Faq,
    Contact,
    Privacy,
    Terms,
}

/// Anything the app opens from a URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Link {
    Reader(ReaderLink),
    /// `/azkar/{slug}` and `/t/{lang}/{translator}/azkar/{slug}`.
    Zikr {
        slug: String,
        translation: Option<TranslationRef>,
    },
    Page(StaticPage),
}

/// The app's URL scheme.
pub const SCHEME: &str = "easyquran://";
/// Hosts whose https links are the app's.
pub const HOSTS: [&str; 2] = ["easyquran.fyi", "www.easyquran.fyi"];

/// Segments that are never a surah slug (`RESERVED_SURAH_SEGMENTS`).
const RESERVED: [&str; 31] = [
    "juz",
    "page",
    "hizb",
    "rub",
    "t",
    "surah",
    "pages",
    "yours",
    "azkar",
    "search",
    "settings",
    "downloads",
    "bookmarks",
    "about",
    "faq",
    "contact",
    "privacy",
    "terms",
    "login",
    "register",
    "forgot-password",
    "verify-email",
    "account",
    "auth",
    "health",
    "design",
    "app",
    "en",
    "daily-ayah",
    "report",
    "appeal",
];

/// `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$` (surah slugs and content-language
/// segments).
fn is_kebab(s: &str) -> bool {
    let b = s.as_bytes();
    if b.first().is_none_or(|c| !c.is_ascii_lowercase()) || s.ends_with('-') || s.contains("--") {
        return false;
    }
    b.iter()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// `^[a-z0-9]+(?:[._-][a-z0-9]+)*$`.
fn is_translator_segment(s: &str) -> bool {
    let sep = |c: char| matches!(c, '.' | '_' | '-');
    !s.is_empty()
        && s.split(sep).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// `^[1-9]\d*$` within `1..=max`.
fn positive(s: &str, max: u16) -> Option<u16> {
    if s.starts_with('0') || s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<u16>().ok().filter(|n| (1..=max).contains(n))
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).and_then(|d| u8::try_from(d).ok())
}

/// `application/x-www-form-urlencoded` value decoding (URLSearchParams).
fn decode_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b',' | b':') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Query or fragment pairs, decoded, in order.
fn pairs(s: &str) -> Vec<(String, String)> {
    s.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (decode_component(k), decode_component(v)),
            None => (decode_component(p), String::new()),
        })
        .collect()
}

fn get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// `parseMoreParam`.
pub fn parse_more(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if !out.iter().any(|x| x == part) {
            out.push(part.to_owned());
        }
    }
    out
}

/// Splits `path?query#fragment`.
fn split_href(href: &str) -> (&str, &str, &str) {
    let (rest, fragment) = href.split_once('#').unwrap_or((href, ""));
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    (path, query, fragment)
}

/// `(s, a)` from `"{s}:{a}"`.
fn verse_pair(v: &str) -> Option<(u16, u16)> {
    crate::quran::types::parse_verse_key(v)
}

/// The ayah a surah URL asks for (`requestedAyah`): `#ayah-s-n`, then
/// `?v=s:a`, then `?ayah=S:N-S:M` (first ayah), then legacy `?verse=n`.
fn requested_ayah(
    surah: u16,
    ayah_count: u16,
    query: &[(String, String)],
    fragment: &str,
) -> Option<u16> {
    let hash = fragment
        .strip_prefix(&format!("ayah-{surah}-"))
        .filter(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|n| n.parse::<u16>().ok());
    let shared = get(query, "v")
        .and_then(verse_pair)
        .filter(|(s, _)| *s == surah)
        .map(|(_, a)| a);
    let range = get(query, "ayah")
        .and_then(|r| r.split_once('-'))
        .and_then(|(a, b)| verse_pair(a).zip(verse_pair(b)))
        .filter(|((s, _), _)| *s == surah)
        .map(|((_, a), _)| a);
    let legacy = get(query, "verse").and_then(|v| v.parse::<u16>().ok());
    let value = hash.or(shared).or(range).or(legacy)?;
    (1..=ayah_count).contains(&value).then_some(value)
}

fn reader(
    scope: ReaderScope,
    translation: Option<TranslationRef>,
    query: &[(String, String)],
) -> Link {
    Link::Reader(ReaderLink {
        target: ReaderTarget {
            scope,
            translation,
            ayah: None,
        },
        mode: get(query, "mode").and_then(ReaderMode::parse),
        more: get(query, "more").map(parse_more).unwrap_or_default(),
    })
}

fn range_scope(kind: &str, n: &str) -> Option<ReaderScope> {
    match kind {
        "page" => positive(n, PAGE_COUNT).map(ReaderScope::Page),
        "juz" => positive(n, u16::from(JUZ_COUNT))
            .and_then(|n| u8::try_from(n).ok())
            .map(ReaderScope::Juz),
        "hizb" => positive(n, u16::from(HIZB_COUNT))
            .and_then(|n| u8::try_from(n).ok())
            .map(ReaderScope::Hizb),
        _ => None,
    }
}

fn static_page(seg: &[&str], fragment: &str) -> Option<StaticPage> {
    let page = match seg {
        [] => StaticPage::Home,
        ["surah"] => StaticPage::SurahIndex,
        ["juz"] => StaticPage::JuzIndex,
        ["pages"] => StaticPage::PagesIndex,
        ["azkar"] => StaticPage::Azkar,
        ["search"] => StaticPage::Search,
        ["bookmarks"] => StaticPage::Bookmarks,
        ["yours"] => StaticPage::Yours,
        ["yours", "cards", id] => StaticPage::YoursCard((*id).to_owned()),
        ["daily-ayah"] => StaticPage::DailyAyah,
        ["downloads"] => StaticPage::Downloads,
        ["settings"] => StaticPage::Settings,
        ["account"] => StaticPage::Account,
        ["login"] => StaticPage::Login,
        ["register"] => StaticPage::Register,
        ["verify-email"] => StaticPage::VerifyEmail,
        ["forgot-password"] => StaticPage::ForgotPassword,
        ["appeal"] => StaticPage::Appeal(None),
        ["appeal", token] => StaticPage::Appeal(Some((*token).to_owned())),
        ["report"] => StaticPage::Report,
        ["newsletter", "confirm"] => {
            let f = pairs(fragment);
            StaticPage::NewsletterConfirm {
                email: get(&f, "email").unwrap_or("").trim().to_owned(),
                token: get(&f, "token").unwrap_or("").trim().to_owned(),
            }
        }
        ["about"] => StaticPage::About,
        ["faq"] => StaticPage::Faq,
        ["contact"] => StaticPage::Contact,
        ["privacy"] => StaticPage::Privacy,
        ["terms"] => StaticPage::Terms,
        _ => return None,
    };
    Some(page)
}

/// Parses a site path with optional `?query` and `#fragment` against `m`.
pub fn parse_path_with(m: &QuranMeta, href: &str) -> Option<Link> {
    let (path, query, fragment) = split_href(href);
    let query = pairs(query);
    let mut seg: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if seg.first() == Some(&"ar") {
        seg.remove(0);
    }
    if let Some(page) = static_page(&seg, fragment) {
        return Some(Link::Page(page));
    }
    let surah_link = |slug: &str, translation: Option<TranslationRef>| -> Option<Link> {
        if !is_kebab(slug) || RESERVED.contains(&slug) {
            return None;
        }
        let s = m.surah_by_slug(slug)?;
        let mut link = reader(ReaderScope::Surah(s.num), translation, &query);
        if let Link::Reader(r) = &mut link {
            r.target.ayah = requested_ayah(s.num, s.ayah_count, &query, fragment);
        }
        Some(link)
    };
    let translation = |lang: &str, translator: &str| -> Option<TranslationRef> {
        (is_kebab(lang) && is_translator_segment(translator)).then(|| TranslationRef {
            lang: lang.to_owned(),
            translator: translator.to_owned(),
        })
    };
    match seg.as_slice() {
        ["azkar", slug] if is_kebab(slug) => Some(Link::Zikr {
            slug: (*slug).to_owned(),
            translation: None,
        }),
        [kind, n] if range_scope(kind, n).is_some() => {
            range_scope(kind, n).map(|scope| reader(scope, None, &query))
        }
        ["t", lang, translator, "azkar", slug] if is_kebab(slug) => Some(Link::Zikr {
            slug: (*slug).to_owned(),
            translation: Some(translation(lang, translator)?),
        }),
        ["t", lang, translator, kind, n] => {
            let t = translation(lang, translator)?;
            range_scope(kind, n).map(|scope| reader(scope, Some(t), &query))
        }
        [slug] => surah_link(slug, None),
        [slug, "t", lang, translator] => surah_link(slug, Some(translation(lang, translator)?)),
        _ => None,
    }
}

/// [`parse_path_with`] over the baked metadata.
pub fn parse_path(href: &str) -> Option<Link> {
    parse_path_with(meta(), href)
}

/// Parses an `easyquran://…` deep link or an `https://easyquran.fyi/…`
/// share link; `None` for other hosts and unknown paths.
pub fn parse_url(url: &str) -> Option<Link> {
    if let Some(rest) = url.strip_prefix(SCHEME) {
        return parse_path(&format!("/{rest}"));
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..end];
    if !HOSTS.contains(&host) {
        return None;
    }
    parse_path(&format!("/{}", rest[end..].trim_start_matches('/')))
}

/// Route context of a reader (`SurahRouteContext`): Arabic or one
/// translation.
pub fn route_context(source_id: &str) -> Option<TranslationRef> {
    match crate::quran::types::ArabicSource::from_id(source_id) {
        Some(_) => None,
        None => Some(TranslationRef::from_id(source_id)),
    }
}

/// `surahPathFor`.
pub fn surah_path_for(ctx: Option<&TranslationRef>, slug: &str) -> String {
    match ctx {
        None => format!("/{slug}"),
        Some(t) => format!("/{slug}/t/{}/{}", t.lang, t.translator),
    }
}

/// `surahAyahPathFor`: `#ayah-{num}-{ayah}` on the surah root.
pub fn surah_ayah_path_for(
    ctx: Option<&TranslationRef>,
    slug: &str,
    num: u16,
    ayah: u16,
) -> String {
    format!("{}#ayah-{num}-{ayah}", surah_path_for(ctx, slug))
}

/// `surahAyahRevealPathFor`: the `?v={num}:{ayah}` form.
pub fn surah_ayah_reveal_path_for(
    ctx: Option<&TranslationRef>,
    slug: &str,
    num: u16,
    ayah: u16,
) -> String {
    format!("{}?v={num}:{ayah}", surah_path_for(ctx, slug))
}

fn range_path(ctx: Option<&TranslationRef>, kind: &str, n: u16) -> String {
    match ctx {
        None => format!("/{kind}/{n}"),
        Some(t) => format!("/t/{}/{}/{kind}/{n}", t.lang, t.translator),
    }
}

/// `globalPagePathFor`.
pub fn global_page_path_for(ctx: Option<&TranslationRef>, page: u16) -> String {
    range_path(ctx, "page", page)
}

/// `juzPathFor`.
pub fn juz_path_for(ctx: Option<&TranslationRef>, juz: u8) -> String {
    range_path(ctx, "juz", u16::from(juz))
}

/// `hizbPathFor`.
pub fn hizb_path_for(ctx: Option<&TranslationRef>, hizb: u8) -> String {
    range_path(ctx, "hizb", u16::from(hizb))
}

/// `snippetPathFor`.
pub fn snippet_path_for(ctx: Option<&TranslationRef>, slug: &str) -> String {
    match ctx {
        None => format!("/azkar/{slug}"),
        Some(t) => format!("/t/{}/{}/azkar/{slug}", t.lang, t.translator),
    }
}

/// Canonical path of a reader target (`None` for an unknown surah).
pub fn reader_target_path_with(m: &QuranMeta, target: &ReaderTarget) -> Option<String> {
    let ctx = target.translation.as_ref();
    match target.scope {
        ReaderScope::Surah(n) => {
            let s = m.surah(n)?;
            Some(match target.ayah {
                Some(a) => surah_ayah_path_for(ctx, &s.slug, n, a),
                None => surah_path_for(ctx, &s.slug),
            })
        }
        ReaderScope::Page(n) => Some(global_page_path_for(ctx, n)),
        ReaderScope::Juz(n) => Some(juz_path_for(ctx, n)),
        ReaderScope::Hizb(n) => Some(hizb_path_for(ctx, n)),
    }
}

/// Canonical path of a reader link, `?mode=`/`?more=` included.
pub fn reader_link_path(link: &ReaderLink) -> Option<String> {
    let path = reader_target_path_with(meta(), &link.target)?;
    let (base, fragment) = match path.split_once('#') {
        Some((b, f)) => (b.to_owned(), format!("#{f}")),
        None => (path, String::new()),
    };
    let mut params = Vec::new();
    if let Some(mode) = link.mode {
        params.push(format!("mode={}", mode.as_str()));
    }
    if !link.more.is_empty() {
        params.push(format!(
            "more={}",
            encode_component(&parse_more(&link.more.join(",")).join(","))
        ));
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };
    Some(format!("{base}{query}{fragment}"))
}

fn static_path(page: &StaticPage) -> String {
    match page {
        StaticPage::Home => "/".into(),
        StaticPage::SurahIndex => "/surah".into(),
        StaticPage::JuzIndex => "/juz".into(),
        StaticPage::PagesIndex => "/pages".into(),
        StaticPage::Azkar => "/azkar".into(),
        StaticPage::Search => "/search".into(),
        StaticPage::Bookmarks => "/bookmarks".into(),
        StaticPage::Yours => "/yours".into(),
        StaticPage::YoursCard(id) => format!("/yours/cards/{}", encode_component(id)),
        StaticPage::DailyAyah => "/daily-ayah".into(),
        StaticPage::Downloads => "/downloads".into(),
        StaticPage::Settings => "/settings".into(),
        StaticPage::Account => "/account".into(),
        StaticPage::Login => "/login".into(),
        StaticPage::Register => "/register".into(),
        StaticPage::VerifyEmail => "/verify-email".into(),
        StaticPage::ForgotPassword => "/forgot-password".into(),
        StaticPage::Appeal(None) => "/appeal".into(),
        StaticPage::Appeal(Some(t)) => format!("/appeal/{}", encode_component(t)),
        StaticPage::Report => "/report".into(),
        StaticPage::NewsletterConfirm { email, token } => format!(
            "/newsletter/confirm#email={}&token={}",
            encode_component(email),
            encode_component(token)
        ),
        StaticPage::About => "/about".into(),
        StaticPage::Faq => "/faq".into(),
        StaticPage::Contact => "/contact".into(),
        StaticPage::Privacy => "/privacy".into(),
        StaticPage::Terms => "/terms".into(),
    }
}

/// Canonical path of any link.
pub fn link_path(link: &Link) -> Option<String> {
    match link {
        Link::Reader(r) => reader_link_path(r),
        Link::Zikr { slug, translation } => Some(snippet_path_for(translation.as_ref(), slug)),
        Link::Page(p) => Some(static_path(p)),
    }
}

/// The same reader position in another translation, or in Arabic with
/// `None` (`hrefFor` / `arabicHrefFor`): the ayah anchor rides along.
pub fn retarget(link: &Link, translation: Option<TranslationRef>) -> Option<Link> {
    match link {
        Link::Reader(r) => {
            let mut next = r.clone();
            next.target.translation = translation;
            Some(Link::Reader(next))
        }
        Link::Zikr { slug, .. } => Some(Link::Zikr {
            slug: slug.clone(),
            translation,
        }),
        Link::Page(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(lang: &str, translator: &str) -> TranslationRef {
        TranslationRef {
            lang: lang.into(),
            translator: translator.into(),
        }
    }

    fn reader_of(href: &str) -> ReaderLink {
        match parse_path(href) {
            Some(Link::Reader(r)) => r,
            other => panic!("{href}: {other:?}"),
        }
    }

    #[test]
    fn parses_arabic_surah_page_juz_hizb_never_rub() {
        assert_eq!(
            reader_of("/al-fatihah").target,
            ReaderTarget::arabic(ReaderScope::Surah(1))
        );
        assert_eq!(reader_of("/page/42").target.scope, ReaderScope::Page(42));
        assert_eq!(reader_of("/juz/30").target.scope, ReaderScope::Juz(30));
        assert_eq!(reader_of("/hizb/60").target.scope, ReaderScope::Hizb(60));
        assert_eq!(parse_path("/rub/240"), None);
        assert_eq!(parse_path("/page/605"), None);
        assert_eq!(parse_path("/juz/0"), None);
        assert_eq!(parse_path("/juz/07"), None);
        assert_eq!(parse_path("/hizb/61"), None);
    }

    #[test]
    fn parses_translated_routes_keeping_lang_translator() {
        let j = reader_of("/t/ms/basmeih/juz/30").target;
        assert_eq!(
            (j.scope, j.translation),
            (ReaderScope::Juz(30), Some(t("ms", "basmeih")))
        );
        let s = reader_of("/ar-rum/t/en/sahih").target;
        assert_eq!(
            (s.scope, s.translation),
            (ReaderScope::Surah(30), Some(t("en", "sahih")))
        );
        let h = reader_of("/t/en/sahih/hizb/7").target;
        assert_eq!(h.scope, ReaderScope::Hizb(7));
        let q = reader_of("/t/quranenc/en.hilali_khan/page/3").target;
        assert_eq!(q.translation.unwrap().id(), "quranenc.en.hilali_khan");
    }

    #[test]
    fn unknown_and_malformed_paths_are_none() {
        for href in [
            "/app",
            "/t/ms",
            "/t/ms/basmeih",
            "/does-not-exist",
            "/al-fatihah/page/1",
            "/al-fatihah/t/en",
            "/t/EN/sahih/juz/1",
            "/design",
            "/health/quran",
        ] {
            assert_eq!(parse_path(href), None, "{href}");
        }
        assert_eq!(parse_path("/"), Some(Link::Page(StaticPage::Home)));
    }

    #[test]
    fn locale_prefix_and_trailing_slash() {
        assert_eq!(
            reader_of("/ar/al-baqarah").target.scope,
            ReaderScope::Surah(2)
        );
        assert_eq!(
            reader_of("/t/ms/basmeih/page/7/").target.scope,
            ReaderScope::Page(7)
        );
    }

    #[test]
    fn ayah_anchor_forms() {
        assert_eq!(reader_of("/ar-rum#ayah-30-12").target.ayah, Some(12));
        assert_eq!(reader_of("/ar-rum?v=30:12").target.ayah, Some(12));
        assert_eq!(reader_of("/ar-rum?v=31:12").target.ayah, None);
        assert_eq!(reader_of("/ar-rum?ayah=30:5-30:9").target.ayah, Some(5));
        assert_eq!(reader_of("/ar-rum?verse=3").target.ayah, Some(3));
        assert_eq!(reader_of("/ar-rum?verse=999").target.ayah, None);
        assert_eq!(reader_of("/ar-rum#ayah-30-0").target.ayah, None);
    }

    #[test]
    fn mode_and_more_params() {
        assert_eq!(
            reader_of("/al-fatihah?mode=reading").mode,
            Some(ReaderMode::Reading)
        );
        assert_eq!(
            reader_of("/al-fatihah?mode=verse").mode,
            Some(ReaderMode::Verse)
        );
        assert_eq!(reader_of("/al-fatihah?mode=foo").mode, None);
        assert_eq!(reader_of("/al-fatihah?mode=").mode, None);
        assert_eq!(
            reader_of("/al-fatihah?more=en.sahih,, ,ur.jalandhry").more,
            vec!["en.sahih", "ur.jalandhry"]
        );
        assert_eq!(
            reader_of("/al-fatihah?more=en.sahih,ur.jalandhry,en.sahih").more,
            vec!["en.sahih", "ur.jalandhry"]
        );
        assert_eq!(
            reader_of("/al-fatihah?more=en.sahih%2Cur.x").more,
            vec!["en.sahih", "ur.x"]
        );
    }

    #[test]
    fn builders_match_web_route_tests() {
        let ctx = route_context("ms.basmeih");
        let c = ctx.as_ref();
        assert_eq!(surah_path_for(None, "al-baqarah"), "/al-baqarah");
        assert_eq!(surah_path_for(c, "luqman"), "/luqman/t/ms/basmeih");
        assert_eq!(
            surah_ayah_path_for(c, "ar-rum", 30, 12),
            "/ar-rum/t/ms/basmeih#ayah-30-12"
        );
        assert_eq!(
            surah_ayah_path_for(None, "ar-rum", 30, 12),
            "/ar-rum#ayah-30-12"
        );
        assert_eq!(global_page_path_for(None, 42), "/page/42");
        assert_eq!(global_page_path_for(c, 42), "/t/ms/basmeih/page/42");
        assert_eq!(juz_path_for(c, 30), "/t/ms/basmeih/juz/30");
        assert_eq!(hizb_path_for(None, 60), "/hizb/60");
        assert_eq!(hizb_path_for(c, 60), "/t/ms/basmeih/hizb/60");
        assert_eq!(
            snippet_path_for(c, "morning"),
            "/t/ms/basmeih/azkar/morning"
        );
        assert_eq!(route_context("uthmani"), None);
        assert_eq!(route_context("en.sahih.int"), Some(t("en", "sahih.int")));
        assert_eq!(
            surah_ayah_reveal_path_for(None, "ar-rum", 30, 2),
            "/ar-rum?v=30:2"
        );
    }

    #[test]
    fn translation_ids_round_trip_through_segments() {
        for translator in ["sahih", "sahih.int", "khan.maududi", "a.b.c"] {
            let r = TranslationRef::from_id(&format!("en.{translator}"));
            assert_eq!(r, t("en", translator));
            assert_eq!(r.id(), format!("en.{translator}"));
        }
        assert_eq!(TranslationRef::from_id("en"), t("en", ""));
        assert_eq!(t("en", "").id(), "en");
    }

    #[test]
    fn every_route_round_trips() {
        let m = meta();
        let mut hrefs: Vec<String> = Vec::new();
        for s in m.surahs() {
            hrefs.push(surah_path_for(None, &s.slug));
            hrefs.push(surah_path_for(Some(&t("en", "sahih")), &s.slug));
            hrefs.push(surah_ayah_path_for(None, &s.slug, s.num, s.ayah_count));
        }
        for p in [1, 302, 604] {
            hrefs.push(global_page_path_for(None, p));
            hrefs.push(global_page_path_for(
                Some(&t("quranenc", "en.hilali_khan")),
                p,
            ));
        }
        for j in [1, 30] {
            hrefs.push(juz_path_for(Some(&t("ur", "jalandhry")), j));
        }
        for h in [1, 60] {
            hrefs.push(hizb_path_for(None, h));
        }
        hrefs.push("/al-fatihah?mode=reading&more=en.sahih,ur.jalandhry#ayah-1-3".into());
        hrefs.push(snippet_path_for(None, "morning"));
        hrefs.push(snippet_path_for(Some(&t("en", "sahih")), "evening"));
        for p in [
            "/",
            "/surah",
            "/juz",
            "/pages",
            "/azkar",
            "/search",
            "/bookmarks",
            "/yours",
            "/yours/cards/abc",
            "/daily-ayah",
            "/downloads",
            "/settings",
            "/account",
            "/login",
            "/register",
            "/verify-email",
            "/forgot-password",
            "/appeal",
            "/appeal/tok",
            "/report",
            "/newsletter/confirm#email=a%40b.c&token=xyz",
            "/about",
            "/faq",
            "/contact",
            "/privacy",
            "/terms",
        ] {
            hrefs.push(p.into());
        }
        for href in hrefs {
            let link = parse_path(&href).unwrap_or_else(|| panic!("{href}"));
            assert_eq!(link_path(&link).as_deref(), Some(href.as_str()));
        }
    }

    #[test]
    fn urls_and_retargeting() {
        let l = parse_url("https://easyquran.fyi/t/ms/basmeih/juz/30").unwrap();
        let en = retarget(&l, Some(t("en", "sahih"))).unwrap();
        assert_eq!(link_path(&en).unwrap(), "/t/en/sahih/juz/30");
        let ar = retarget(
            &parse_url("easyquran://ar-rum/t/en/sahih?v=30:4").unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(link_path(&ar).unwrap(), "/ar-rum#ayah-30-4");
        assert_eq!(parse_url("https://example.com/al-fatihah"), None);
        assert_eq!(
            parse_url("https://www.easyquran.fyi/").unwrap(),
            Link::Page(StaticPage::Home)
        );
        assert_eq!(
            parse_url("easyquran://azkar/morning").unwrap(),
            Link::Zikr {
                slug: "morning".into(),
                translation: None
            }
        );
        assert_eq!(
            parse_path("/newsletter/confirm#email=a%40b.c&token=%20x%20"),
            Some(Link::Page(StaticPage::NewsletterConfirm {
                email: "a@b.c".into(),
                token: "x".into()
            }))
        );
    }

    #[test]
    fn target_serde_shape_is_kept() {
        let v = serde_json::to_value(ReaderTarget {
            scope: ReaderScope::Surah(2),
            translation: Some(t("en", "sahih")),
            ayah: Some(255),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"scope": {"Surah": 2}, "translation": {"lang": "en", "translator": "sahih"}, "ayah": 255})
        );
    }
}
