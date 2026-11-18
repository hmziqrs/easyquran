//! The translations picker (PARITY C6, C7): web `TranslationButton.svelte`,
//! `TranslationModal.svelte`, `translation-search.ts`,
//! `translation-region.ts` (+ its two data tables, read from the web
//! source), `translation-nav.ts`.
//!
//! A row toggles the translation in the stack (`?more=`, the stacked
//! store); its trailing arrow opens that translation's own reader route at
//! the same position (`hrefFor`). Adding a translation that is not on the
//! device starts its download; lanes read online meanwhile (the read
//! store's fallback).

use std::rc::Rc;

use eq_core::{
    i18n::keys,
    quran::catalogue::{
        self, Provenance, TranslationEntry, flag_for, group_translation_authors, native_name_for,
        translation_author_key, translation_provenance_rank,
    },
    routes::{ReaderLink, ReaderScope, TranslationRef},
};
use nsh_core::element::AnyElement;
use nsh_core::prelude::*;

use super::lanes::lane_settings;
use crate::{
    stores::{
        locale::{t, t_args, tr},
        quran::{DownloadStatus, QuranStore},
        reader::ReaderStore,
        translations::{RecentsStore, STACKED_MAX_EXTRAS, StackStore},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with, icon_svg_button},
        controls::switch_row_with,
        dialog::dialog_sheet,
        field::text_field,
        icons::lucide,
        layout::{divider, pressable_card},
        tokens,
    },
};

const ARROW_RIGHT: &str = lucide!(r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#);
const ARROW_UP: &str = lucide!(r#"<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>"#);
const ARROW_DOWN: &str = lucide!(r#"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"#);
const CLOSE: &str = lucide!(r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#);

/// The pinned Recent group's size (`RECENT_GROUP_MAX`).
const RECENT_GROUP_MAX: usize = 5;

// ---------------------------------------------------------------------
// Search (`translation-search.ts`)
// ---------------------------------------------------------------------

/// Combining marks NFD would split off precomposed letters, plus the
/// Arabic-script and Hebrew marks (`\p{Mark}` over the catalogue's text).
fn is_mark(c: char) -> bool {
    matches!(u32::from(c),
        0x0300..=0x036F
        | 0x0483..=0x0489
        | 0x0591..=0x05BD
        | 0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x0670
        | 0x06D6..=0x06DC
        | 0x06DF..=0x06E4
        | 0x06E7..=0x06E8
        | 0x06EA..=0x06ED
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF
        | 0xFE20..=0xFE2F)
}

/// Lower-case precomposed Latin letters and the base letter NFD leaves
/// once the mark is stripped ("García" -> "garcia").
const FOLDS: &[(&str, char)] = &[
    ("àáâãäåāăąǎ", 'a'),
    ("çćĉċč", 'c'),
    ("ďḍ", 'd'),
    ("èéêëēĕėęě", 'e'),
    ("ĝğġģ", 'g'),
    ("ĥḥ", 'h'),
    ("ìíîïĩīĭįǐ", 'i'),
    ("ĵ", 'j'),
    ("ķ", 'k'),
    ("ĺļľ", 'l'),
    ("ñńņň", 'n'),
    ("òóôõöōŏőǒ", 'o'),
    ("ŕŗř", 'r'),
    ("śŝşšṣ", 's'),
    ("ţťṭ", 't'),
    ("ùúûüũūŭůűųǔ", 'u'),
    ("ŵ", 'w'),
    ("ýÿŷ", 'y'),
    ("źżžẓ", 'z'),
];

fn fold(c: char) -> char {
    FOLDS
        .iter()
        .find(|(set, _)| set.contains(c))
        .map_or(c, |(_, base)| *base)
}

/// lowercase + marks stripped (`normalize`).
pub fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|c| !is_mark(*c))
        .map(fold)
        .collect()
}

fn words(value: &str) -> Vec<String> {
    normalize(value)
        .split(|c: char| c.is_whitespace() || c == '\'' || c == '\u{2019}' || c == '-')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Spelling aliases (TRX-04).
const SPELLING_ALIASES: [(&str, &str); 2] = [("sahih", "saheeh"), ("pickthall", "pickthal")];

fn is_alias_of(token: &str, word: &str) -> bool {
    SPELLING_ALIASES
        .iter()
        .any(|(a, b)| (token == *a && word == *b) || (token == *b && word == *a))
}

/// Two-row Levenshtein, 2 when the lengths differ by more than one.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > 1 {
        return 2;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut curr = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitution = prev[j] + usize::from(ca != cb);
            let next = (prev[j + 1] + 1).min(curr[j] + 1).min(substitution);
            curr.push(next);
        }
        prev = curr;
    }
    prev[b.len()]
}

fn token_hits_word(token: &str, word: &str) -> bool {
    if word.contains(token) || is_alias_of(token, word) {
        return true;
    }
    if token.chars().count() < 4 {
        return false;
    }
    edit_distance(token, word) <= 1
}

/// The fields a row matches against (`TranslationSearchFields`).
fn field_words(t: &TranslationEntry) -> Vec<Vec<String>> {
    let country = flag_for(&t.language_code).country;
    vec![
        words(&t.name),
        t.translator.as_deref().map(words).unwrap_or_default(),
        words(&t.language),
        words(&t.language_code),
        words(&country),
        native_name_for(&t.language_code)
            .map(words)
            .unwrap_or_default(),
    ]
}

/// Every query token hits some field of the row (`translationMatchesQuery`).
pub fn matches_query(query: &str, t: &TranslationEntry) -> bool {
    let tokens: Vec<String> = normalize(query)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    if tokens.is_empty() {
        return true;
    }
    let fields = field_words(t);
    tokens.iter().all(|token| {
        fields
            .iter()
            .any(|fw| fw.iter().any(|w| token_hits_word(token, w)))
    })
}

// ---------------------------------------------------------------------
// Region ranking (`translation-region.ts`)
// ---------------------------------------------------------------------

const REGION_LANGUAGES_TS: &str =
    include_str!("../../../../web/src/lib/components/translation/region-languages.ts");
const REGION_TIMEZONES_TS: &str =
    include_str!("../../../../web/src/lib/components/translation/region-timezones.ts");

/// `  XX: <rest>,` lines of a TS object literal: (country, rest).
fn ts_entries(ts: &str) -> impl Iterator<Item = (&str, &str)> {
    ts.lines().filter_map(|line| {
        let (key, rest) = line.trim().split_once(':')?;
        let ok = key.len() == 2 && key.chars().all(|c| c.is_ascii_uppercase());
        ok.then_some((key, rest.trim().trim_end_matches(',')))
    })
}

fn quoted(rest: &str) -> Vec<&str> {
    rest.split('"')
        .skip(1)
        .step_by(2)
        .filter(|s| !s.is_empty())
        .collect()
}

/// `REGION_LANGUAGES[country]`.
fn region_languages(country: &str) -> Vec<String> {
    ts_entries(REGION_LANGUAGES_TS)
        .find(|(k, _)| *k == country)
        .map(|(_, rest)| quoted(rest).into_iter().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// `countryFromTimeZone`.
fn country_from_time_zone(zone: &str) -> Option<String> {
    if zone.is_empty() {
        return None;
    }
    ts_entries(REGION_TIMEZONES_TS)
        .find(|(_, rest)| {
            quoted(rest)
                .first()
                .is_some_and(|z| z.split(' ').any(|x| x == zone))
        })
        .map(|(k, _)| k.to_owned())
}

fn push_unique(out: &mut Vec<String>, value: String) {
    if !out.contains(&value) {
        out.push(value);
    }
}

/// The two-letter region of a BCP 47 tag (`ur-PK` -> `PK`).
fn region_of(tag: &str) -> Option<String> {
    tag.split(['-', '_'])
        .skip(1)
        .find(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_ascii_uppercase)
}

/// `languagePriority`: first system language, English, the other system
/// languages, the languages of the probable countries, Arabic.
fn language_priority(tags: &[String], countries: &[String]) -> Vec<String> {
    let mut browser = Vec::new();
    for tag in tags {
        let base = tag
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !base.is_empty() {
            push_unique(&mut browser, base);
        }
    }
    let mut out = Vec::new();
    for code in browser.iter().take(1) {
        push_unique(&mut out, code.clone());
    }
    push_unique(&mut out, "en".to_owned());
    for code in browser.iter().skip(1) {
        push_unique(&mut out, code.clone());
    }
    for country in countries {
        for code in region_languages(country) {
            push_unique(&mut out, code);
        }
    }
    push_unique(&mut out, "ar".to_owned());
    out
}

/// The reader's rail priority from the system's languages and time zone
/// (`detectCountries` + `languagePriority`): nothing leaves the device.
fn detect_priority() -> Vec<String> {
    let tags = crate::platform::locale::system_languages();
    let zone = locale_settings().time_zone.as_str().to_owned();
    let mut countries = Vec::new();
    if let Some(c) = country_from_time_zone(&zone) {
        push_unique(&mut countries, c);
    }
    for tag in &tags {
        if let Some(r) = region_of(tag) {
            push_unique(&mut countries, r);
        }
    }
    language_priority(&tags, &countries)
}

// ---------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------

/// One rail tab (`LanguageGroup`), by language code.
#[derive(Clone)]
struct LanguageGroup {
    language: String,
    code: String,
    flag: String,
    autonym: Option<&'static str>,
    entries: Vec<&'static TranslationEntry>,
}

fn collate(a: &str, b: &str) -> std::cmp::Ordering {
    normalize(a).cmp(&normalize(b))
}

/// `buildGroups`: one tab per code, rows by name, tabs by language.
fn build_groups(rows: &[&'static TranslationEntry]) -> Vec<LanguageGroup> {
    let mut groups: Vec<LanguageGroup> = Vec::new();
    for t in rows {
        match groups.iter_mut().find(|g| g.code == t.language_code) {
            Some(g) => g.entries.push(t),
            None => groups.push(LanguageGroup {
                language: t.language.clone(),
                code: t.language_code.clone(),
                flag: flag_for(&t.language_code).flag,
                autonym: native_name_for(&t.language_code),
                entries: vec![t],
            }),
        }
    }
    for g in &mut groups {
        g.entries.sort_by(|a, b| collate(&a.name, &b.name));
    }
    groups.sort_by(|a, b| collate(&a.language, &b.language));
    groups
}

/// One author's row (`AuthorGroup`): the representative variant and the rest.
struct Author {
    key: String,
    representative: &'static TranslationEntry,
    rest: Vec<&'static TranslationEntry>,
}

fn recency(recents: RecentsStore, id: &str) -> usize {
    recents.rank_of(id).unwrap_or(usize::MAX)
}

/// `compareRepresentatives`: selected, then recent, then the author's
/// remembered variant, then the original publisher.
fn better(
    key: &str,
    a: &TranslationEntry,
    b: &TranslationEntry,
    selected: &[String],
    recents: RecentsStore,
) -> bool {
    let sel = |id: &str| selected.iter().any(|s| s == id);
    if sel(&a.id) != sel(&b.id) {
        return sel(&a.id);
    }
    let (ra, rb) = (recency(recents, &a.id), recency(recents, &b.id));
    if ra != rb {
        return ra < rb;
    }
    if let Some(remembered) = recents.author_variant(key) {
        if a.id == remembered {
            return true;
        }
        if b.id == remembered {
            return false;
        }
    }
    translation_provenance_rank(&a.id) < translation_provenance_rank(&b.id)
}

fn authors_of(
    entries: &[&'static TranslationEntry],
    selected: &[String],
    recents: RecentsStore,
) -> Vec<Author> {
    group_translation_authors(entries.iter().copied())
        .into_iter()
        .map(|g| {
            let mut best = 0;
            for i in 1..g.variants.len() {
                if better(&g.key, g.variants[i], g.variants[best], selected, recents) {
                    best = i;
                }
            }
            let representative = g.variants[best];
            let rest = g
                .variants
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != best)
                .map(|(_, v)| *v)
                .collect();
            Author {
                key: g.key,
                representative,
                rest,
            }
        })
        .collect()
}

fn source_short(id: &str) -> &'static str {
    match catalogue::provenance_of(id) {
        Provenance::Qul => "QUL",
        Provenance::QuranEnc => "QuranEnc",
        Provenance::Tanzil => "Tanzil",
    }
}

/// `formatSize`.
fn format_size(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let b = bytes as f64;
    if bytes >= 1024 * 1024 {
        return format!("{:.1} MB", b / (1024.0 * 1024.0));
    }
    format!("{} KB", (b / 1024.0).round().max(1.0))
}

fn results_label(count: usize) -> SharedString {
    if count == 1 {
        return tr(keys::READER_TRANSLATIONS_RESULTS_ONE).into();
    }
    let n = i64::try_from(count).unwrap_or(0);
    t_args(keys::READER_TRANSLATIONS_RESULTS, move || {
        vec![("count", n.into())]
    })()
}

// ---------------------------------------------------------------------
// Navigation (`translation-nav.ts` `hrefFor`)
// ---------------------------------------------------------------------

/// The same reader position in translation `id`: a surah reader carries
/// the topmost verse (the `?v=` mirror); the stack drives `?more=`.
pub fn link_for(link: &ReaderLink, id: &str, top: Option<(u16, u16)>) -> ReaderLink {
    let mut next = link.clone();
    next.target.translation = Some(TranslationRef::from_id(id));
    next.more = Vec::new();
    if let (ReaderScope::Surah(n), Some((s, a))) = (link.target.scope, top) {
        if s == n {
            next.target.ayah = Some(a);
        }
    }
    next
}

// ---------------------------------------------------------------------
// The picker
// ---------------------------------------------------------------------

/// What every row needs.
#[derive(Clone)]
struct Picker {
    stack: StackStore,
    recents: RecentsStore,
    quran: QuranStore,
    open: Signal<bool>,
    expanded: Signal<Vec<String>>,
    go: Rc<dyn Fn(String)>,
}

impl Picker {
    /// `recordUse`.
    fn record(&self, id: &str) {
        let key = catalogue::translation(id).map(translation_author_key);
        self.recents.record(id, key.as_deref());
    }

    /// `toggle`: an add resumes a paused stack, counts as a use, and
    /// fetches the source when it is not on the device.
    fn toggle(&self, id: &str) {
        let was = self.stack.ids().iter().any(|i| i == id);
        self.stack.toggle(id);
        if !was {
            self.stack.set_enabled(true);
            self.record(id);
            if !self.quran.is_installed(id) {
                self.quran.download(id);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    /// A language pane or recent row: author line.
    Main,
    /// Search: the language under the name.
    Search,
    /// An author's other source: its size.
    Variant,
}

/// The availability line (TRX-16): on the device, downloading, or online.
fn offline_line(quran: QuranStore, id: &'static str) -> impl Fn() -> SharedString + 'static {
    move || {
        if quran.is_installed(id) {
            return tr(keys::READER_TOOLTIP_OFFLINE_DOWNLOADED).into();
        }
        match quran.downloads().get(id) {
            Some(DownloadStatus::Running(p)) if p.total > 0 => {
                format!("{}%", p.loaded.saturating_mul(100) / p.total).into()
            }
            Some(DownloadStatus::Running(_)) => "…".into(),
            _ => tr(keys::READER_TOOLTIP_OFFLINE_NEEDS_INTERNET).into(),
        }
    }
}

fn indicator(picker: &Picker, id: &'static str) -> impl IntoElement {
    let stack = picker.stack;
    let on = move || stack.ids().iter().any(|i| i == id);
    view()
        .size(px(18.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_2()
        .border_color(move || {
            if on() {
                nsh_base::theme().primary
            } else {
                nsh_base::theme().input
            }
        })
        .bg(move || {
            if on() {
                nsh_base::theme().primary
            } else {
                nsh_base::theme().background
            }
        })
        .child(
            text(move || if on() { "✓" } else { "" })
                .text_xs()
                .text_color(move || nsh_base::theme().primary_foreground),
        )
}

/// One translation row (`translationRow`).
fn row(
    picker: &Picker,
    t: &'static TranslationEntry,
    kind: RowKind,
    sources: Option<(String, usize)>,
) -> AnyElement {
    let id: &'static str = t.id.as_str();
    let stack = picker.stack;
    let locked = move || !stack.ids().iter().any(|i| i == id) && stack.is_full();
    let label = t.translator.clone().unwrap_or_else(|| t.name.clone());
    let second: Option<String> = match kind {
        RowKind::Variant => Some(format_size(t.size_bytes)),
        RowKind::Search => Some(t.language.clone()),
        RowKind::Main => t
            .translator
            .clone()
            .filter(|tr| tr.trim().to_lowercase() != t.name.trim().to_lowercase()),
    };
    let toggle = {
        let picker = picker.clone();
        move || picker.toggle(id)
    };
    let cover = pressable_card(label, toggle)
        .disabled(locked)
        .flex_1()
        .min_w(px(0.0))
        .flex_row()
        .items_center()
        .gap_3()
        .border(px(0.0))
        .bg(move || nsh_base::theme().background)
        .px_3()
        .py_2()
        .min_h(px(44.0))
        .disabled_style(|s| s.opacity(0.5))
        .child(indicator(picker, id))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap_1()
                .child(
                    text(t.name.clone())
                        .text_sm()
                        .number_of_lines(1)
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(
                    text(format!(
                        "{}{}",
                        second.map(|s| format!("{s} · ")).unwrap_or_default(),
                        source_short(id)
                    ))
                    .text_xs()
                    .number_of_lines(1)
                    .text_color(tokens::muted_text),
                )
                .child(
                    text(offline_line(picker.quran, id))
                        .text_xs()
                        .tabular_nums()
                        .text_color(tokens::muted_text),
                ),
        );
    let sources_toggle = sources.map(|(key, count)| {
        let expanded = picker.expanded;
        button_with(
            results_label(count),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            false,
            None,
            move || {
                expanded.update(|e| {
                    if e.contains(&key) {
                        e.retain(|k| *k != key);
                    } else {
                        e.push(key.clone());
                    }
                });
            },
        )
    });
    // Commentary has no route of its own (TRX-01): no go-to arrow.
    let go_to = (!t.is_tafsir()).then(|| {
        let go = picker.go.clone();
        let open = picker.open;
        icon_svg_button(
            ARROW_RIGHT,
            tr(keys::READER_TRANSLATIONS_GO_TO),
            move || {
                open.set(false);
                go(id.to_owned());
            },
        )
    });
    view()
        .w_full()
        .flex_row()
        .items_center()
        .gap_1()
        .child(cover)
        .child(sources_toggle)
        .child(go_to)
        .into_element()
}

/// Author rows, a multi-source author's other sources under it when open.
fn author_rows(picker: &Picker, authors: Vec<Author>, kind: RowKind) -> Vec<AnyElement> {
    let expanded = picker.expanded.get();
    let mut out = Vec::new();
    for a in authors {
        let open = expanded.contains(&a.key);
        let sources = (!a.rest.is_empty()).then(|| (a.key.clone(), a.rest.len() + 1));
        out.push(row(picker, a.representative, kind, sources));
        if open {
            for v in a.rest {
                out.push(row(picker, v, RowKind::Variant, None));
            }
        }
    }
    out
}

fn heading(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .px_2()
        .pt_3()
        .pb_1()
        .text_color(tokens::muted_text)
}

/// The selected texts: the Show switch, chips with up/down/remove, the
/// count and Clear, then the lane display settings.
fn stack_section(picker: &Picker, prefs: ReaderStore) -> AnyElement {
    let stack = picker.stack;
    let ids = stack.ids();
    if ids.is_empty() {
        return view().into_element();
    }
    let n = i64::try_from(ids.len()).unwrap_or(0);
    let max = i64::try_from(STACKED_MAX_EXTRAS).unwrap_or(5);
    let chips: Vec<AnyElement> = ids
        .iter()
        .filter_map(|id| catalogue::translation(id))
        .map(|t| {
            let id: &'static str = t.id.as_str();
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .pl_3()
                .rounded(tokens::radius_md)
                .border_1()
                .border_color(move || nsh_base::theme().border)
                .child(
                    text(t.name.clone())
                        .flex_1()
                        .min_w(px(0.0))
                        .number_of_lines(1)
                        .text_sm()
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(icon_svg_button(
                    ARROW_UP,
                    tr(keys::READER_STACKED_MOVE_UP),
                    move || stack.reorder(id, -1),
                ))
                .child(icon_svg_button(
                    ARROW_DOWN,
                    tr(keys::READER_STACKED_MOVE_DOWN),
                    move || stack.reorder(id, 1),
                ))
                .child(icon_svg_button(
                    CLOSE,
                    tr(keys::READER_STACKED_REMOVE),
                    move || stack.remove(id),
                ))
                .into_element()
        })
        .collect();
    view()
        .gap_2()
        .pb_2()
        .child(switch_row_with(
            t(keys::READER_STACKED_SHOW),
            move || stack.enabled(),
            move |on| stack.set_enabled(on),
            tr(keys::READER_STACKED_SHOW),
        ))
        .child(view().gap_1().children(chips))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    text(t_args(keys::READER_STACKED_COUNT, move || {
                        vec![("n", n.into()), ("max", max.into())]
                    }))
                    .text_xs()
                    .tabular_nums()
                    .text_color(tokens::muted_text),
                )
                .child(button_with(
                    t(keys::READER_STACKED_CLEAR),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    false,
                    None,
                    move || stack.clear(),
                )),
        )
        .child(lane_settings(stack, prefs))
        .child(divider())
        .into_element()
}

/// The Recent group (global MRU, one row per author, no tafsir).
fn recent_section(picker: &Picker) -> AnyElement {
    let mut seen: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    for id in picker.recents.ids() {
        if rows.len() >= RECENT_GROUP_MAX {
            break;
        }
        let Some(t) = catalogue::translation(&id).filter(|t| !t.is_tafsir()) else {
            continue;
        };
        let key = translation_author_key(t);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        rows.push(row(picker, t, RowKind::Main, None));
    }
    if rows.is_empty() {
        return view().into_element();
    }
    view()
        .child(heading(t(keys::READER_TRANSLATIONS_RECENT)))
        .children(rows)
        .child(divider())
        .into_element()
}

/// The modal's state for one open.
#[derive(Clone)]
struct Body {
    picker: Picker,
    prefs: ReaderStore,
    query: Signal<String>,
    rail_code: Signal<Option<String>>,
    mobile_pane: Signal<bool>,
    priority: Rc<Vec<String>>,
}

impl Body {
    fn searching(&self) -> bool {
        !self.query.get().trim().is_empty()
    }

    /// Search never offers tafsir (TRX-01).
    fn matches(&self) -> Vec<&'static TranslationEntry> {
        let q = self.query.get();
        let q = q.trim();
        catalogue::translations()
            .iter()
            .filter(|t| q.is_empty() || (!t.is_tafsir() && matches_query(q, t)))
            .collect()
    }

    /// The rail: matching languages while searching, priority first.
    fn languages(&self) -> Vec<LanguageGroup> {
        let mut groups = build_groups(&self.matches());
        let rank = |g: &LanguageGroup| {
            self.priority
                .iter()
                .position(|c| *c == g.code)
                .unwrap_or(usize::MAX)
        };
        groups.sort_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then_with(|| collate(&a.language, &b.language))
        });
        groups
    }

    fn is_suggested(&self, code: &str) -> bool {
        self.priority.iter().any(|c| c == code)
    }

    /// The rail code on open: the first stacked text's language, else
    /// the priority's first language the catalogue has.
    fn default_code(&self) -> Option<String> {
        let languages = self.languages();
        let anchor = self
            .picker
            .stack
            .ids()
            .first()
            .and_then(|id| catalogue::translation(id))
            .map(|t| t.language_code.clone());
        if anchor.is_some() {
            return anchor;
        }
        for code in self.priority.iter() {
            if languages.iter().any(|l| l.code == *code) {
                return Some(code.clone());
            }
        }
        languages.first().map(|l| l.code.clone())
    }

    fn active_group(&self) -> Option<LanguageGroup> {
        let languages = self.languages();
        let code = self.rail_code.get();
        languages
            .iter()
            .find(|l| Some(&l.code) == code.as_ref())
            .or(languages.first())
            .cloned()
    }

    fn rail(&self) -> AnyElement {
        let body = self.clone();
        scroll()
            .flex_1()
            .child(dynamic(move || {
                let languages = body.languages();
                let in_use: Vec<String> = body
                    .picker
                    .stack
                    .ids()
                    .iter()
                    .filter_map(|id| catalogue::translation(id))
                    .map(|t| t.language_code.clone())
                    .collect();
                let mut items = Vec::new();
                if !is_desktop() && !body.searching() {
                    items.push(untracked(|| stack_section(&body.picker, body.prefs)));
                    items.push(untracked(|| recent_section(&body.picker)));
                }
                let mut previous_suggested = false;
                for (i, l) in languages.into_iter().enumerate() {
                    let suggested = body.is_suggested(&l.code);
                    if i > 0 && previous_suggested && !suggested {
                        items.push(divider().into_element());
                    }
                    previous_suggested = suggested;
                    items.push(body.rail_row(l, &in_use));
                }
                view().gap_1().children(items).into_element()
            }))
            .into_element()
    }

    fn rail_row(&self, l: LanguageGroup, in_use: &[String]) -> AnyElement {
        let rail_code = self.rail_code;
        let mobile_pane = self.mobile_pane;
        let code = l.code.clone();
        let active_code = l.code.clone();
        let active = move || rail_code.get().as_deref() == Some(active_code.as_str());
        let dot = in_use.contains(&l.code);
        pressable_card(l.language.clone(), move || {
            rail_code.set(Some(code.clone()));
            mobile_pane.set(true);
        })
        .flex_row()
        .items_center()
        .gap_2()
        .border(px(0.0))
        .px_3()
        .py_2()
        .min_h(px(44.0))
        .selected(active)
        .selected_style(|s| s.bg(tokens::primary_soft))
        .child(text(l.flag.clone()).text_base())
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    text(l.language.clone())
                        .text_sm()
                        .number_of_lines(1)
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(l.autonym.map(|a| {
                    text(a)
                        .text_xs()
                        .number_of_lines(1)
                        .text_color(tokens::muted_text)
                })),
        )
        .child(dot.then(|| {
            view()
                .size(px(6.0))
                .rounded_full()
                .bg(move || nsh_base::theme().primary)
        }))
        .into_element()
    }

    fn pane(&self) -> AnyElement {
        let body = self.clone();
        scroll()
            .flex_1()
            .child(dynamic(move || {
                let mut items: Vec<AnyElement> = Vec::new();
                let picker = &body.picker;
                if !is_desktop() {
                    let mobile_pane = body.mobile_pane;
                    let query = body.query;
                    items.push(
                        button_with(
                            t(keys::READER_TRANSLATIONS_BACK),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            move || {
                                query.set(String::new());
                                mobile_pane.set(false);
                            },
                        )
                        .into_element(),
                    );
                }
                let selected = picker.stack.ids();
                if body.searching() {
                    let mut rows = body.matches();
                    rows.sort_by(|a, b| {
                        collate(&a.language, &b.language).then_with(|| collate(&a.name, &b.name))
                    });
                    items.push(heading(results_label(rows.len())).into_element());
                    if rows.is_empty() {
                        items.push(
                            text(t(keys::READER_TRANSLATIONS_NO_MATCHES))
                                .text_sm()
                                .p_3()
                                .text_color(tokens::muted_text)
                                .into_element(),
                        );
                    }
                    let authors = authors_of(&rows, &selected, picker.recents);
                    items.extend(untracked(|| author_rows(picker, authors, RowKind::Search)));
                    return view().children(items).into_element();
                }
                if is_desktop() {
                    items.push(untracked(|| stack_section(picker, body.prefs)));
                    items.push(untracked(|| recent_section(picker)));
                }
                let Some(group) = body.active_group() else {
                    return view().children(items).into_element();
                };
                let recents = picker.recents;
                let mut entries: Vec<&'static TranslationEntry> = group
                    .entries
                    .iter()
                    .copied()
                    .filter(|t| !t.is_tafsir())
                    .collect();
                entries.sort_by_key(|t| recency(recents, &t.id));
                let commentary: Vec<&'static TranslationEntry> = catalogue::translations()
                    .iter()
                    .filter(|t| t.is_tafsir() && t.language == group.language)
                    .collect();
                items.push(
                    view()
                        .px_2()
                        .pt_2()
                        .child(
                            text(group.language.clone())
                                .text_base()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(move || nsh_base::theme().foreground),
                        )
                        .child(
                            text(results_label(entries.len() + commentary.len()))
                                .text_xs()
                                .text_color(tokens::muted_text),
                        )
                        .into_element(),
                );
                let authors = authors_of(&entries, &selected, recents);
                items.extend(untracked(|| author_rows(picker, authors, RowKind::Main)));
                if !commentary.is_empty() {
                    items.push(heading(t(keys::READER_TRANSLATIONS_COMMENTARY)).into_element());
                    for c in commentary {
                        items.push(untracked(|| row(picker, c, RowKind::Main, None)));
                    }
                }
                view().children(items).into_element()
            }))
            .into_element()
    }
}

/// The modal's content for one open.
fn picker_body(picker: Picker, prefs: ReaderStore) -> AnyElement {
    let body = Body {
        picker: picker.clone(),
        prefs,
        query: signal(String::new()),
        rail_code: signal(None),
        mobile_pane: signal(false),
        priority: Rc::new(detect_priority()),
    };
    // Auto-select the rail language on open.
    body.rail_code.set(untracked(|| body.default_code()));
    let open = picker.open;
    let content = {
        let body = body.clone();
        dynamic(move || {
            let pane_visible = body.mobile_pane.get() || body.searching();
            untracked(|| {
                if is_desktop() {
                    return view()
                        .flex_1()
                        .flex_row()
                        .gap_3()
                        .child(
                            view()
                                .w(px(220.0))
                                .border_r_1()
                                .border_color(move || nsh_base::theme().border)
                                .child(body.rail()),
                        )
                        .child(view().flex_1().child(body.pane()))
                        .into_element();
                }
                if pane_visible {
                    return body.pane();
                }
                body.rail()
            })
        })
    };
    view()
        .flex_1()
        .p_4()
        .gap_3()
        .label(tr(keys::READER_STACKED_TITLE))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    text(t(keys::READER_STACKED_TITLE))
                        .text_lg()
                        .font_weight(FontWeight::MEDIUM)
                        .role(nsh_core::element::AccessibilityRole::Header)
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(icon_svg_button(
                    CLOSE,
                    tr(keys::READER_TRANSLATIONS_CLOSE),
                    move || open.set(false),
                )),
        )
        .child(text_field(
            body.query,
            tr(keys::READER_STACKED_SEARCH_PLACEHOLDER),
        ))
        .child(content)
        .into_element()
}

/// The translations control in the reader bar (`TranslationButton`): the
/// label and the count of stacked texts not already on screen; it opens
/// the picker. `go` opens a reader link (the row's go-to arrow).
pub fn translation_button(
    link: ReaderLink,
    top: Signal<Option<(u16, u16)>>,
    prefs: ReaderStore,
    go: impl Fn(ReaderLink) + 'static,
) -> AnyElement {
    let stack = StackStore::get();
    // Debug builds: `EQ_OPEN_TRANSLATIONS` opens the picker (screenshots).
    let open = signal(cfg!(debug_assertions) && std::env::var_os("EQ_OPEN_TRANSLATIONS").is_some());
    let route_id = link.target.translation.as_ref().map(TranslationRef::id);
    let hidden = move || {
        stack
            .active_ids()
            .iter()
            .filter(|id| Some(*id) != route_id.as_ref())
            .count()
    };
    let held = stored(link);
    let go: Rc<dyn Fn(String)> = Rc::new(move |id: String| {
        let next = held.with(|l| link_for(l, &id, top.get_untracked()));
        go(next);
    });
    let picker = Picker {
        stack,
        recents: RecentsStore::get(),
        quran: crate::app::quran_store(),
        open,
        expanded: signal(Vec::new()),
        go,
    };
    view()
        .flex_shrink_0()
        .child(
            pressable_card(tr(keys::READER_TRANSLATIONS_OPEN), move || open.set(true))
                .flex_row()
                .items_center()
                .gap_2()
                .px_3()
                .py(px(6.0))
                .min_h(px(36.0))
                .child(
                    text(t(keys::READER_TRANSLATIONS_OPEN))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || nsh_base::theme().foreground),
                )
                .child(dynamic(move || {
                    let n = hidden();
                    if n == 0 {
                        return view().into_element();
                    }
                    view()
                        .min_w(px(20.0))
                        .h(px(20.0))
                        .px(px(6.0))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(move || nsh_base::theme().primary)
                        .child(
                            text(n.to_string())
                                .text_xs()
                                .tabular_nums()
                                .text_color(move || nsh_base::theme().primary_foreground),
                        )
                        .into_element()
                })),
        )
        .child(dialog_sheet(
            open,
            move || open.set(false),
            move || picker_body(picker.clone(), prefs),
        ))
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_folds_diacritics_and_typos() {
        assert_eq!(normalize("García"), "garcia");
        assert!(token_hits_word("englsh", "english"));
        assert!(token_hits_word("sahih", "saheeh"));
        assert!(!token_hits_word("eng", "german"));
    }

    #[test]
    fn region_tables_parse() {
        assert_eq!(region_languages("PK"), vec!["ur", "pa", "sd", "ps"]);
        assert_eq!(country_from_time_zone("Asia/Kabul").as_deref(), Some("AF"));
        let p = language_priority(&["ur-PK".into()], &["PK".into()]);
        assert_eq!(&p[..3], &["ur", "en", "pa"]);
        assert_eq!(p.last().map(String::as_str), Some("ar"));
    }
}
