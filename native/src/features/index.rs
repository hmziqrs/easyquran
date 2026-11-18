//! Surah, juz and pages indexes (PARITY C1, B3): the web's
//! `routes/(application)/{surah,juz,pages}/+page.svelte` with
//! `_reader/IndexTabs.svelte` above them. Contract:
//! `pub fn surah_index_page() -> impl IntoElement`,
//! `pub fn juz_index_page() -> impl IntoElement`,
//! `pub fn pages_index_page() -> impl IntoElement`.
//!
//! The data is the baked division table (`eq_core::quran::meta`): static,
//! no store. Every index is a virtualized list (114 / 30 / 604 + 30
//! section rows); a row press pushes the reader at that scope.
//!
//! Divergences, all pending a store this slice does not own: rows open
//! the Arabic reader (the web carries the last-read translation context,
//! `resumeCtxFor(reader.lastRead)`), and the surah order (mushaf /
//! revelation, the web sidebar's control) is kept for the page's life
//! only.

use eq_core::i18n::{self, Locale, keys};
use eq_core::quran::meta::{Place, RangeEntry, RangeKind, Surah, SurahSort, meta};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::shell::{
    nav,
    routes::{ReaderScope, ReaderTarget, Route},
};
use crate::stores::locale::{locale, tr};
use crate::ui::{arabic::HAFS_FAMILY, button, chip, layout, text, tokens};

/// The route a row opens. The one place the reader route is built from
/// an index, so a change to `Route::Reader` is fixed here.
pub fn reader_route(scope: ReaderScope) -> Route {
    Route::Reader(eq_core::routes::ReaderLink {
        target: ReaderTarget::arabic(scope),
        mode: None,
        more: Vec::new(),
    })
}

fn open_reader(scope: ReaderScope) {
    nav::push(reader_route(scope));
}

// --- Shared frame -------------------------------------------------------------

/// Which index a page is (`IndexTabs` `active`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum IndexTab {
    Surah,
    Juz,
    Pages,
}

const TABS: [(IndexTab, i18n::Key, Route); 3] = [
    (
        IndexTab::Surah,
        keys::READER_INDEX_SURAHS_TITLE,
        Route::SurahIndex,
    ),
    (IndexTab::Juz, keys::READER_INDEX_JUZ_TITLE, Route::JuzIndex),
    (
        IndexTab::Pages,
        keys::READER_INDEX_PAGES_TITLE,
        Route::PagesIndex,
    ),
];

/// The index switcher: three chips, the active one filled. Made again on a
/// locale switch (the chips' spoken names are fixed when made).
fn index_tabs(active: IndexTab, l: Locale) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_1p5()
        .label(i18n::tr(l, keys::READER_BROWSE))
        .children(TABS.iter().cloned().map(move |(tab, key, route)| {
            let label = i18n::tr(l, key);
            chip::filter_chip(
                label,
                label,
                move || tab == active,
                move || {
                    if tab != active {
                        nav::open(route.clone());
                    }
                },
            )
        }))
}

/// The bar above an index: the switcher, then the row count at the end.
fn index_bar(active: IndexTab, count: impl Fn() -> SharedString + 'static) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .px_4()
        .py_2()
        .child(dynamic(move || index_tabs(active, locale()).into_element()))
        .child(view().flex_1())
        .child(
            text(count)
                .text_xs()
                .tabular_nums()
                .text_color(tokens::muted_text),
        )
}

/// A list row's padding: compact on the desktops, a full touch row on
/// phones.
fn row_pad() -> (f32, f32) {
    if is_desktop() {
        return (12.0, 8.0);
    }
    (16.0, 12.0)
}

/// The space at the end of a list: clear of the phone's floating tab bar.
fn list_end() -> f32 {
    16.0 + layout::tab_bar_room()
}

/// An Arabic name in the Arabic face, right to left whatever the UI locale.
fn arabic_name(label: impl Fn() -> SharedString + 'static, size: f32) -> impl IntoElement {
    text(label)
        .text_size(px(size))
        .font_family(HAFS_FAMILY)
        .writing_direction(WritingDirection::Rtl)
        .number_of_lines(1)
        .text_color(tokens::foreground_secondary)
}

/// A round number medallion in hue `slot` (the surah cards' `HUE_EDGE` /
/// `HUE_DIM`).
fn medallion(
    number: impl Fn() -> SharedString + 'static,
    slot: impl Fn() -> u8 + Copy + 'static,
) -> View {
    view()
        .size(px(40.0))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_2()
        .border_color(move || tokens::hue(slot()))
        .bg(move || tokens::hue_soft(slot()))
        .child(
            text(number)
                .text_size(px(15.0))
                .font_weight(FontWeight::MEDIUM)
                .tabular_nums()
                .text_color(move || tokens::hue_legible(slot())),
        )
}

/// `hueSlotFor`: slots 1-8 cycling by position.
fn hue_slot(index: usize) -> u8 {
    u8::try_from((index.saturating_sub(1)) % 8).unwrap_or(0) + 1
}

/// "2:255".
fn verse_key((surah, ayah): (u16, u16)) -> String {
    format!("{surah}:{ayah}")
}

/// The surah's English name, "" past the table.
fn surah_name(num: u16) -> &'static str {
    meta().surah(num).map_or("", |s| s.name.as_str())
}

/// The sajdas inside a range, as verse keys.
fn sajdas_in(range: &RangeEntry) -> Vec<String> {
    meta()
        .sajdas()
        .iter()
        .filter(|s| s.global_index >= range.start_global && s.global_index <= range.end_global)
        .map(|s| format!("{}:{}", s.surah, s.ayah))
        .collect()
}

fn range_item(kind: i18n::Key, index: u16) -> String {
    i18n::tr_args(locale(), kind, &[("index", index.into())])
}

// --- Surah index --------------------------------------------------------------

pub fn surah_index_page() -> impl IntoElement {
    surah_index_view()
}

#[component]
fn SurahIndexView() -> impl IntoElement {
    let sort = signal(SurahSort::Mushaf);
    let order = computed(move || {
        meta()
            .surahs_in_order(sort.get())
            .iter()
            .map(|s| s.num)
            .collect::<Vec<u16>>()
    });
    let (pad_x, pad_y) = row_pad();
    view()
        .flex_1()
        .flex_col()
        .child(index_bar(IndexTab::Surah, move || {
            meta().surahs().len().to_string().into()
        }))
        .child(dynamic(move || sort_bar(sort, locale()).into_element()))
        .child(
            virtual_list(
                move || order.with(Vec::len),
                move |at| {
                    let surah = computed(move || order.with(|o| o.get(at.index()).copied()));
                    let row = move || surah.get().and_then(|n| meta().surah(n));
                    surah_row(row, pad_x, pad_y)
                },
            )
            .keys(move || order.get())
            .estimated_item_size(px(if is_desktop() { 64.0 } else { 80.0 }))
            .content_padding_x(px(16.0))
            .content_padding_bottom(px(list_end()))
            .row_gap(px(8.0))
            .flex_1(),
        )
}

/// The surah order control (`Sidebar.svelte` `SORTS`): mushaf or
/// revelation order.
fn sort_bar(sort: Signal<SurahSort>, l: Locale) -> impl IntoElement {
    let option = move |value: SurahSort, key: i18n::Key| {
        let label = i18n::tr(l, key);
        chip::filter_chip(
            label,
            label,
            move || sort.get() == value,
            move || sort.set(value),
        )
    };
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .px_4()
        .pb_2()
        .child(
            text(i18n::tr(l, keys::READER_SORT_LABEL))
                .text_xs()
                .text_color(tokens::muted_text),
        )
        .child(option(SurahSort::Mushaf, keys::READER_SORT_MUSHAF))
        .child(option(SurahSort::Revelation, keys::READER_SORT_REVELATION))
}

fn name_of(s: &'static Surah) -> &'static str {
    s.name.as_str()
}

fn meaning_of(s: &'static Surah) -> &'static str {
    s.meaning.as_str()
}

fn arabic_of(s: &'static Surah) -> &'static str {
    s.arabic.as_str()
}

fn place_key(place: Place) -> i18n::Key {
    match place {
        Place::Meccan => keys::SEARCH_NAV_PLACE_MECCAN,
        Place::Medinan => keys::SEARCH_NAV_PLACE_MEDINAN,
    }
}

/// One surah: medallion, name over meaning, ayah count and place, the
/// Arabic name at the end.
fn surah_row(
    row: impl Fn() -> Option<&'static Surah> + Copy + 'static,
    pad_x: f32,
    pad_y: f32,
) -> impl IntoElement {
    let num = move || row().map_or(0, |s| s.num);
    let field = move |f: fn(&'static Surah) -> &'static str| {
        move || SharedString::from(row().map_or("", f))
    };
    layout::pressable_card("", move || {
        if num() > 0 {
            open_reader(ReaderScope::Surah(num()));
        }
    })
    .accessibility_label(move || {
        let Some(s) = row() else {
            return SharedString::default();
        };
        format!("{} {}, {}", s.num, s.name, s.meaning).into()
    })
    .flex_row()
    .items_center()
    .gap(px(14.0))
    .px(px(pad_x))
    .py(px(pad_y))
    .child(medallion(
        move || num().to_string().into(),
        move || hue_slot(usize::from(num())),
    ))
    .child(
        view()
            .flex_1()
            .min_w(px(0.0))
            .flex_col()
            .gap_0p5()
            .child(
                text(field(name_of))
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .number_of_lines(1),
            )
            .child(
                text(field(meaning_of))
                    .text_xs()
                    .number_of_lines(1)
                    .text_color(tokens::muted_text),
            )
            .child(
                text(move || {
                    let Some(s) = row() else {
                        return SharedString::default();
                    };
                    let count = i18n::tr_args(
                        locale(),
                        keys::READER_INDEX_AYAH_COUNT,
                        &[("count", s.ayah_count.into())],
                    );
                    format!("{count} · {}", tr(place_key(s.place))).into()
                })
                .text_xs()
                .number_of_lines(1)
                .text_color(tokens::muted_text),
            ),
    )
    .child(arabic_name(field(arabic_of), 19.0))
}

// --- Juz index ----------------------------------------------------------------

pub fn juz_index_page() -> impl IntoElement {
    let juzs = meta().ranges(RangeKind::Juz);
    let count = juzs.len();
    let (pad_x, pad_y) = row_pad();
    view()
        .flex_1()
        .flex_col()
        .child(index_bar(IndexTab::Juz, move || {
            i18n::tr_args(locale(), keys::READER_JUZ_COUNT, &[("count", count.into())]).into()
        }))
        .child(
            virtual_list(count, move |at| {
                let index = move || u16::try_from(at.index() + 1).unwrap_or(1);
                juz_row(index, pad_x, pad_y)
            })
            .estimated_item_size(px(if is_desktop() { 104.0 } else { 132.0 }))
            .content_padding_x(px(16.0))
            .content_padding_bottom(px(list_end()))
            .row_gap(px(12.0))
            .flex_1(),
        )
}

/// "Al-Fatihah 1:1".
fn bound_label(verse: (u16, u16)) -> String {
    format!("{} {}", surah_name(verse.0), verse_key(verse))
}

/// The sajda chip's text: "Sajda" for one, "2 sajdas" for more.
fn sajda_label(count: usize) -> String {
    if count == 1 {
        return tr(keys::READER_INDEX_SAJDA).to_owned();
    }
    i18n::tr_args(
        locale(),
        keys::READER_INDEX_SAJDA_COUNT,
        &[("count", count.into())],
    )
}

/// One juz: the juz itself (number, where it starts and ends, sajdas),
/// then its two hizbs, each its own reader.
fn juz_row(index: impl Fn() -> u16 + Copy + 'static, pad_x: f32, pad_y: f32) -> impl IntoElement {
    let range = move || meta().range(RangeKind::Juz, index());
    let sajdas = move || range().map(|r| sajdas_in(&r)).unwrap_or_default();
    let slot = move || hue_slot(usize::from(index()));
    let juz = move || u8::try_from(index()).unwrap_or(1);
    let hizb = move |half: u8| {
        let number = move || juz() * 2 - 1 + half;
        button::button_with(
            move || {
                let first = meta()
                    .hizb_range(number())
                    .map(|r| verse_key(r.first))
                    .unwrap_or_default();
                SharedString::from(format!(
                    "{} · {first}",
                    range_item(keys::READER_HIZB_ITEM, u16::from(number()))
                ))
            },
            button::ButtonVariant::Secondary,
            button::ButtonSize::Small,
            false,
            None,
            move || open_reader(ReaderScope::Hizb(number())),
        )
        .flex_1()
    };
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(
            layout::pressable_card("", move || open_reader(ReaderScope::Juz(juz())))
                .accessibility_label(move || {
                    SharedString::from(range_item(keys::READER_JUZ_ITEM, index()))
                })
                .flex_row()
                .items_center()
                .gap(px(14.0))
                .px(px(pad_x))
                .py(px(pad_y))
                .child(
                    view()
                        .h(px(36.0))
                        .min_w(px(44.0))
                        .px(px(10.0))
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .rounded(tokens::radius_pill)
                        .bg(move || tokens::hue_soft(slot()))
                        .child(
                            text(move || SharedString::from(index().to_string()))
                                .text_size(px(15.0))
                                .font_weight(FontWeight::MEDIUM)
                                .tabular_nums()
                                .text_color(move || tokens::hue_legible(slot())),
                        ),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_col()
                        .gap_0p5()
                        .child(
                            text(move || {
                                SharedString::from(range_item(keys::READER_JUZ_ITEM, index()))
                            })
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM),
                        )
                        .child(
                            text(move || {
                                range()
                                    .map(|r| {
                                        format!(
                                            "{} – {}",
                                            bound_label(r.first),
                                            bound_label(r.last)
                                        )
                                    })
                                    .map_or_else(SharedString::default, SharedString::from)
                            })
                            .text_xs()
                            .number_of_lines(1)
                            .text_color(tokens::muted_text),
                        ),
                )
                .child(when(
                    move || !sajdas().is_empty(),
                    move || {
                        chip::chip(
                            move || SharedString::from(sajda_label(sajdas().len())),
                            chip::ChipTone::Accent,
                            false,
                        )
                        .label(move || sajdas().join(", "))
                    },
                )),
        )
        .child(view().flex_row().gap_2().child(hizb(0)).child(hizb(1)))
}

// --- Pages index --------------------------------------------------------------

/// A row of the pages list: a juz heading, or a page under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PageItem {
    Juz { juz: u8, first: u16, last: u16 },
    Page(u16),
}

/// Pages grouped under the juz holding their first verse, in mushaf order.
fn page_items() -> Vec<PageItem> {
    let m = meta();
    let mut out = Vec::new();
    let mut current: Option<u8> = None;
    let pages = m.ranges(RangeKind::Page);
    for page in &pages {
        let juz = m.juz_of_global(page.start_global);
        if current != Some(juz) {
            current = Some(juz);
            let in_juz = pages
                .iter()
                .filter(|p| m.juz_of_global(p.start_global) == juz)
                .map(|p| p.index);
            let first = in_juz.clone().min().unwrap_or(page.index);
            let last = in_juz.max().unwrap_or(page.index);
            out.push(PageItem::Juz { juz, first, last });
        }
        out.push(PageItem::Page(page.index));
    }
    out
}

/// The surahs a page draws from, in order.
fn page_surahs(page: &RangeEntry) -> Vec<&'static Surah> {
    (page.first.0..=page.last.0)
        .filter_map(|n| meta().surah(n))
        .collect()
}

/// The surah a page card names in Arabic: the one opening on the page,
/// else the first on it (`leadSurah`).
fn lead_surah(page: &RangeEntry) -> Option<&'static Surah> {
    let surahs = page_surahs(page);
    surahs
        .iter()
        .find(|s| s.start_global >= page.start_global && s.start_global <= page.end_global)
        .or(surahs.first())
        .copied()
}

fn opens_surah(page: &RangeEntry) -> bool {
    page_surahs(page)
        .iter()
        .any(|s| s.start_global >= page.start_global && s.start_global <= page.end_global)
}

/// The sajda tag: "Sajda 32:15" for one, "2 sajdas" for more.
fn page_sajda_label(sajdas: &[String]) -> String {
    match sajdas {
        [one] => format!("{} {one}", tr(keys::READER_INDEX_SAJDA)),
        many => i18n::tr_args(
            locale(),
            keys::READER_INDEX_SAJDA_COUNT,
            &[("count", many.len().into())],
        ),
    }
}

pub fn pages_index_page() -> impl IntoElement {
    let items = page_items();
    let headers: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| matches!(item, PageItem::Juz { .. }))
        .map(|(at, _)| at)
        .collect();
    let items = stored(items);
    let page_count = meta().range_count(RangeKind::Page);
    let (pad_x, pad_y) = row_pad();
    view()
        .flex_1()
        .flex_col()
        .child(index_bar(IndexTab::Pages, move || {
            i18n::tr_args(
                locale(),
                keys::READER_PAGE_COUNT,
                &[("count", page_count.into())],
            )
            .into()
        }))
        .child(
            virtual_list(
                move || items.with(|i| i.len()),
                move |at| {
                    let item = computed(move || items.with(|i| i.get(at.index()).copied()));
                    page_cell(item, pad_x, pad_y)
                },
            )
            .sticky_header_indices(headers)
            .estimated_item_size(px(if is_desktop() { 60.0 } else { 76.0 }))
            .content_padding_x(px(16.0))
            .content_padding_bottom(px(list_end()))
            .row_gap(px(8.0))
            .flex_1(),
        )
}

/// A cell of the pages list: a juz heading or a page card, by its item.
fn page_cell(item: Memo<Option<PageItem>>, pad_x: f32, pad_y: f32) -> impl IntoElement {
    let is_header = move || matches!(item.get(), Some(PageItem::Juz { .. }));
    let page = move || match item.get() {
        Some(PageItem::Page(n)) => meta().range(RangeKind::Page, n),
        _ => None,
    };
    view().w_full().child(
        when(is_header, move || juz_heading(item)).otherwise(move || page_card(page, pad_x, pad_y)),
    )
}

fn juz_heading(item: Memo<Option<PageItem>>) -> impl IntoElement {
    let parts = move || match item.get() {
        Some(PageItem::Juz { juz, first, last }) => (juz, first, last),
        _ => (0, 0, 0),
    };
    view()
        .w_full()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .pt_4()
        .pb_1()
        .bg(move || theme().background)
        .child(text::section_title(move || {
            SharedString::from(range_item(keys::READER_JUZ_ITEM, u16::from(parts().0)))
        }))
        .child(
            text(move || {
                let (_, first, last) = parts();
                SharedString::from(format!(
                    "{}–{last}",
                    range_item(keys::READER_PAGE_ITEM, first)
                ))
            })
            .text_xs()
            .tabular_nums()
            .text_color(tokens::muted_text),
        )
}

/// One page: its tile (tinted where a surah opens), the surahs it draws
/// from, its verse range with a sajda tag, the lead surah's Arabic name.
fn page_card(
    page: impl Fn() -> Option<RangeEntry> + Copy + 'static,
    pad_x: f32,
    pad_y: f32,
) -> impl IntoElement {
    let index = move || page().map_or(0, |p| p.index);
    let opens = move || page().is_some_and(|p| opens_surah(&p));
    let names = move || {
        page()
            .map(|p| {
                page_surahs(&p)
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default()
    };
    let sajdas = move || page().map(|p| sajdas_in(&p)).unwrap_or_default();
    let verses = move || {
        page()
            .map(|p| format!("{} – {}", verse_key(p.first), verse_key(p.last)))
            .unwrap_or_default()
    };
    layout::pressable_card("", move || {
        if index() > 0 {
            open_reader(ReaderScope::Page(index()));
        }
    })
    .accessibility_label(move || {
        let mut out = format!(
            "{} · {} · {}",
            range_item(keys::READER_PAGE_ITEM, index()),
            names(),
            verses()
        );
        let refs = sajdas();
        if !refs.is_empty() {
            out.push_str(&format!(
                " · {} {}",
                tr(keys::READER_INDEX_SAJDA),
                refs.join(", ")
            ));
        }
        SharedString::from(out)
    })
    .flex_row()
    .items_center()
    .gap(px(14.0))
    .px(px(pad_x))
    .py(px(pad_y))
    .child(
        view()
            .size(px(44.0))
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(tokens::radius_md)
            .bg(move || {
                if opens() {
                    return tokens::primary_soft();
                }
                tokens::background_subtle()
            })
            .child(
                text(move || SharedString::from(index().to_string()))
                    .text_size(px(15.0))
                    .font_weight(FontWeight::MEDIUM)
                    .tabular_nums()
                    .text_color(move || {
                        if opens() {
                            return tokens::primary_hover();
                        }
                        tokens::foreground_secondary()
                    }),
            ),
    )
    .child(
        view()
            .flex_1()
            .min_w(px(0.0))
            .flex_col()
            .gap_1()
            .child(
                text(move || SharedString::from(names()))
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .number_of_lines(1),
            )
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        text(move || SharedString::from(verses()))
                            .text_xs()
                            .tabular_nums()
                            .text_color(tokens::muted_text),
                    )
                    .child(when(
                        move || !sajdas().is_empty(),
                        move || {
                            chip::chip(
                                move || SharedString::from(page_sajda_label(&sajdas())),
                                chip::ChipTone::Accent,
                                false,
                            )
                        },
                    )),
            ),
    )
    .child(arabic_name(
        move || {
            page()
                .and_then(|p| lead_surah(&p))
                .map_or(SharedString::default(), |s| s.arabic.clone().into())
        },
        19.0,
    ))
}
