//! The not-found screen (PARITY A6): the web's `routes/+error.svelte`, 404
//! branch. Contract: `pub fn not_found_page(path: String) -> impl IntoElement`.
//!
//! The web writes this page in English literals (no catalogue keys); they
//! are transcribed in `eq_core::i18n::static_copy`. Native has no address
//! bar, so the unknown path is shown under the body.

use eq_core::i18n::{
    Locale,
    static_copy::{self, NotFoundCopy, QuickLink, SITE_DOMAIN},
};
use eq_core::quran::meta::meta;
use eq_core::routes::ReaderScope;
use nsh_core::prelude::*;

use crate::features::{index::reader_route, marketing};
use crate::shell::{
    nav,
    routes::{Route, SettingsSection},
};
use crate::stores::locale::locale;
use crate::ui::{
    brand, button,
    icons::{self, lucide},
    layout,
    text::{self, TextRole},
    tokens,
};

/// The often-opened surahs (`POPULAR`).
const POPULAR: [u16; 5] = [1, 18, 36, 55, 67];

const ROWS_ICON: &str = lucide!(
    r#"<rect width="18" height="7" x="3" y="3" rx="1"/><rect width="18" height="7" x="3" y="14" rx="1"/>"#
);
const NOTE_ICON: &str = lucide!(
    r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/>"#
);
const HISTORY_ICON: &str = lucide!(
    r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>"#
);
const USER_ICON: &str = lucide!(
    r#"<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>"#
);
const PLUS_ICON: &str = lucide!(r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#);
const BOOKMARK_ICON: &str =
    lucide!(r#"<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>"#);

pub fn not_found_page(path: String) -> impl IntoElement {
    let path: SharedString = path.into();
    scroll()
        .flex_1()
        .content_inset(layout::page_insets(0.0))
        .child(
            view()
                .w_full()
                .flex_col()
                .child(dynamic(move || {
                    let l = locale();
                    body(static_copy::not_found(l), l, path.clone()).into_element()
                }))
                .child(marketing::footer()),
        )
}

fn body(copy: &'static NotFoundCopy, l: Locale, path: SharedString) -> impl IntoElement {
    layout::page_body()
        .items_center()
        .gap_8()
        .child(hero(copy, path))
        .child(often_opened(copy, l))
        .child(
            view()
                .w_full()
                .max_w(px(960.0))
                .flex_row()
                .flex_wrap()
                .gap_4()
                .child(quick_panel(
                    copy.keep_reading,
                    l,
                    vec![
                        (icons::SEARCH, copy.search, Route::Search),
                        (ROWS_ICON, copy.browse_juz, Route::JuzIndex),
                        (NOTE_ICON, copy.browse_pages, Route::PagesIndex),
                        (HISTORY_ICON, copy.history, Route::Yours),
                    ],
                ))
                .child(quick_panel(
                    copy.account_and_tools,
                    l,
                    vec![
                        (USER_ICON, copy.sign_in_link, Route::Login),
                        (PLUS_ICON, copy.create_account_link, Route::Register),
                        (BOOKMARK_ICON, copy.bookmarks, Route::Bookmarks),
                        (
                            ROWS_ICON,
                            copy.settings,
                            Route::Settings(SettingsSection::Storage),
                        ),
                    ],
                )),
        )
}

fn hero(copy: &'static NotFoundCopy, path: SharedString) -> impl IntoElement {
    let action = |label: &'static str, variant: button::ButtonVariant, to: Route| {
        button::button_with(
            label,
            variant,
            button::ButtonSize::Medium,
            false,
            None,
            move || nav::open(to.clone()),
        )
    };
    let small_link = |label: &'static str, to: Route| {
        button::button_with(
            label,
            button::ButtonVariant::Ghost,
            button::ButtonSize::Small,
            false,
            None,
            move || nav::push(to.clone()),
        )
    };
    view()
        .w_full()
        .max_w(px(640.0))
        .flex_col()
        .items_center()
        .gap_4()
        .pt_8()
        .child(
            text("٤٠٤")
                .text_size(px(120.0))
                .font_family(crate::ui::arabic::HAFS_FAMILY)
                .writing_direction(WritingDirection::Rtl)
                .text_color(tokens::border_strong)
                .accessible(false),
        )
        .child(brand::bismillah(176.0, None).text_color(tokens::foreground_secondary))
        .child(layout::eyebrow(format!("404 · {SITE_DOMAIN}")))
        .child(
            text::role(copy.heading, TextRole::H2)
                .text_center()
                .max_w(px(440.0)),
        )
        .child(
            text::role(copy.body_text(), TextRole::Body)
                .text_center()
                .text_color(tokens::foreground_secondary),
        )
        .child(text::code(path))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .justify_center()
                .gap_3()
                .child(action(
                    copy.back_home,
                    button::ButtonVariant::Primary,
                    Route::Home,
                ))
                .child(action(
                    copy.browse_surahs,
                    button::ButtonVariant::Secondary,
                    Route::SurahIndex,
                ))
                .child(action(
                    copy.search_quran,
                    button::ButtonVariant::Ghost,
                    Route::Search,
                )),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .justify_center()
                .items_center()
                .gap_1()
                .child(text::muted(copy.have_account))
                .child(small_link(copy.sign_in, Route::Login))
                .child(text::muted("·"))
                .child(text::muted(copy.new_here))
                .child(small_link(copy.create_account, Route::Register)),
        )
}

fn often_opened(copy: &'static NotFoundCopy, l: Locale) -> impl IntoElement {
    view()
        .w_full()
        .max_w(px(960.0))
        .flex_col()
        .gap_4()
        .child(
            view()
                .flex_row()
                .items_end()
                .justify_between()
                .gap_4()
                .child(text::role(copy.often_opened, TextRole::H3))
                .child(button::button_with(
                    copy.all_surahs,
                    button::ButtonVariant::Ghost,
                    button::ButtonSize::Small,
                    false,
                    None,
                    || nav::open(Route::SurahIndex),
                )),
        )
        .child(view().flex_row().flex_wrap().gap_4().children(
            POPULAR.iter().enumerate().filter_map(|(at, &num)| {
                let surah = meta().surah(num)?;
                let slot = u8::try_from(at % 8).unwrap_or(0) + 1;
                let name = match l {
                    Locale::Ar => surah.arabic.as_str(),
                    Locale::En => surah.name.as_str(),
                };
                Some(
                    layout::pressable_card(name.to_owned(), move || {
                        nav::push(reader_route(ReaderScope::Surah(num)))
                    })
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .min_w(px(260.0))
                    .flex_1()
                    .child(
                        view()
                            .size(px(40.0))
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .border_2()
                            .border_color(move || tokens::hue(slot))
                            .bg(move || tokens::hue_soft(slot))
                            .child(
                                text(num.to_string())
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(move || tokens::hue_legible(slot)),
                            ),
                    )
                    .child(
                        view()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex_col()
                            .gap_0p5()
                            .child(text::role(name, TextRole::Body).font_weight(FontWeight::MEDIUM))
                            .child(
                                text::role(surah.meaning.as_str(), TextRole::Caption)
                                    .text_color(tokens::muted_text)
                                    .number_of_lines(1),
                            )
                            .child(
                                text::role(copy.verse_count(surah.ayah_count), TextRole::Caption)
                                    .text_color(tokens::muted_text),
                            ),
                    )
                    .child(
                        text(surah.arabic.as_str())
                            .text_size(px(22.0))
                            .font_family(crate::ui::arabic::HAFS_FAMILY)
                            .writing_direction(WritingDirection::Rtl)
                            .text_color(tokens::foreground_secondary),
                    ),
                )
            }),
        ))
}

/// A panel of quick links (`quickLink`): icon, label over hint, arrow.
fn quick_panel(
    title: &'static str,
    l: Locale,
    links: Vec<(&'static str, QuickLink, Route)>,
) -> impl IntoElement {
    layout::card()
        .flex_1()
        .min_w(px(280.0))
        .gap_1()
        .p_3()
        .bg(tokens::surface)
        .label(title)
        .child(view().px_2().pt_1().child(layout::eyebrow(title)))
        .children(links.into_iter().map(move |(icon, link, to)| {
            layout::pressable_card(link.label, move || nav::push(to.clone()))
                .flex_row()
                .items_center()
                .gap_3()
                .px_2()
                .py(px(if is_desktop() { 8.0 } else { 10.0 }))
                .bg(AdaptiveColor::TRANSPARENT)
                .border_color(AdaptiveColor::TRANSPARENT)
                .child(
                    view()
                        .size(px(36.0))
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(tokens::border_strong)
                        .child(marketing::icon_svg(
                            icon,
                            16.0,
                            tokens::foreground_secondary,
                        )),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_col()
                        .child(
                            text::role(link.label, TextRole::Caption)
                                .font_weight(FontWeight::MEDIUM),
                        )
                        .child(
                            text::role(link.hint, TextRole::Caption)
                                .text_color(tokens::muted_text)
                                .number_of_lines(1),
                        ),
                )
                .child(marketing::icon_svg(
                    marketing::forward_arrow(l),
                    15.0,
                    tokens::muted_text,
                ))
        }))
}
