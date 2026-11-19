//! Home (PARITY A3; web `(marketing)/+page.svelte`, `lib/home/LandingCards.svelte`).
//! Contract: `pub fn page() -> impl IntoElement`.
//!
//! The landing hero (bismillah, iqra, the search pill that opens the
//! palette, often-opened surahs), the personal cards strip (hidden until
//! the device has cards: up to four tiles and the way to Yours), the surah
//! index and the footer. The widget column lives on Yours.

use eq_core::{i18n::keys, quran::meta::meta, routes::ReaderScope};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    features::{
        bookmarks::follow_notes_owner,
        index::reader_route,
        marketing::footer,
        yours::cards::{card_grid, strip_more},
    },
    shell::{nav, palette, routes::Route},
    stores::{
        home::{HomeData, home_data},
        locale::{t, t_args, tr},
    },
    ui::{
        arabic::{ArabicStyle, QuranScript, plain_ayah},
        brand::bismillah,
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{eyebrow, page_body, page_insets, pressable_card},
        tokens,
    },
};

const OFTEN_OPENED: [u16; 5] = [1, 18, 36, 55, 67];
const STRIP_LIMIT: usize = 4;

fn pill(label: impl IntoReactive<SharedString>, on_press: impl Fn() + 'static) -> impl IntoElement {
    button_with(
        label,
        ButtonVariant::Outline,
        ButtonSize::Small,
        false,
        None,
        on_press,
    )
}

// --- hero ---------------------------------------------------------------------

fn hero() -> impl IntoElement {
    let chips = OFTEN_OPENED.map(|num| {
        let name = meta()
            .surah(num)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        pill(SharedString::from(name), move || {
            nav::push(reader_route(ReaderScope::Surah(num)))
        })
        .into_element()
    });
    view()
        .w_full()
        .flex_col()
        .items_center()
        .gap_2()
        .pt_6()
        .child(
            bismillah(240.0, Some(tr(keys::LANDING_BISMILLAH)))
                .text_color(move || theme().foreground),
        )
        .child(
            text(t(keys::LANDING_BISMILLAH_MEANING))
                .text_xs()
                .text_color(tokens::muted_text),
        )
        .child(
            view().pt_4().child(
                plain_ayah(tr(keys::LANDING_IQRA), None, || ArabicStyle {
                    size: 26.0,
                    ..ArabicStyle::of(QuranScript::Uthmani)
                })
                .text_color(move || theme().foreground),
            ),
        )
        .child(
            text(t(keys::LANDING_IQRA_MEANING))
                .text_xs()
                .text_color(tokens::muted_text),
        )
        .child(
            view().w_full().max_w(px(720.0)).pt_4().child(
                pressable_card(tr(keys::LANDING_SEARCH_LABEL), palette::toggle)
                    .min_h(px(56.0))
                    .justify_center()
                    .rounded(tokens::radius_pill)
                    .px_5()
                    .child(
                        text(t(keys::LANDING_SEARCH_PLACEHOLDER))
                            .text_base()
                            .text_color(tokens::muted_text),
                    ),
            ),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap_2()
                .pt_2()
                .child(
                    text(t(keys::LANDING_OFTEN_OPENED))
                        .text_xs()
                        .text_color(tokens::muted_text),
                )
                .children(chips),
        )
}

// --- cards strip ---------------------------------------------------------------

/// `LandingCards`: hidden while the device has no cards; else the title,
/// "{n} more →" (or "→") to Yours, and up to four tiles.
fn cards_strip(data: Signal<HomeData>) -> impl IntoElement {
    let count = computed(move || data.with(|d| d.cards.len()));
    when(
        move || count.get() > 0,
        move || {
            let extra = move || count.get().saturating_sub(STRIP_LIMIT);
            view()
                .w_full()
                .max_w(px(720.0))
                .flex_col()
                .gap_2()
                .pt_4()
                .child(
                    view()
                        .w_full()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(eyebrow(t(keys::READER_CARDS_STRIP_TITLE)))
                        .child(pill(
                            move || {
                                let n = extra();
                                if n > 0 {
                                    return SharedString::from(format!("{} →", strip_more(n)()));
                                }
                                SharedString::from("→")
                            },
                            || nav::push(Route::Yours),
                        )),
                )
                .child(card_grid(data, STRIP_LIMIT, false))
                .into_element()
        },
    )
}

// --- surah index ------------------------------------------------------------------

fn surah_grid() -> impl IntoElement {
    let tabs = [
        (keys::LANDING_INDEX_JUZ, Route::JuzIndex),
        (keys::LANDING_INDEX_PAGES, Route::PagesIndex),
        (keys::LANDING_INDEX_YOURS, Route::Yours),
    ];
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_1()
                .pb_2()
                .border_b_1()
                .border_color(move || theme().border)
                .child(
                    text(t(keys::LANDING_INDEX_SURAHS))
                        .text_base()
                        .font_weight(FontWeight::MEDIUM)
                        .px_3(),
                )
                .children(tabs.map(|(key, route)| {
                    button_with(
                        t(key),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || nav::push(route.clone()),
                    )
                    .into_element()
                }))
                .child(view().flex_1())
                .child(
                    text(meta().surahs().len().to_string())
                        .text_xs()
                        .text_color(tokens::muted_text),
                ),
        )
        .child(view().w_full().flex_row().flex_wrap().gap_4().children(
            meta().surahs().iter().enumerate().map(|(i, s)| {
                #[allow(clippy::cast_possible_truncation)]
                let hue = (i % 8) as u8 + 1;
                let num = s.num;
                let count = i64::from(s.ayah_count);
                view()
                    .min_w(px(260.0))
                    .flex_1()
                    .child(
                        pressable_card(s.name.clone(), move || {
                            nav::push(reader_route(ReaderScope::Surah(num)))
                        })
                        .flex_row()
                        .items_center()
                        .gap(px(14.0))
                        .child(
                            view()
                                .size(px(40.0))
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .border_2()
                                .border_color(move || tokens::hue(hue))
                                .bg(move || tokens::hue_soft(hue))
                                .child(
                                    text(num.to_string())
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(move || tokens::hue(hue)),
                                ),
                        )
                        .child(
                            view()
                                .flex_1()
                                .min_w(px(0.0))
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    text(s.name.clone())
                                        .text_base()
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate(),
                                )
                                .child(
                                    view()
                                        .w_full()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .gap_3()
                                        .child(
                                            text(s.meaning.clone())
                                                .text_xs()
                                                .truncate()
                                                .text_color(tokens::muted_text),
                                        )
                                        .child(
                                            text(s.arabic.clone())
                                                .text_lg()
                                                .text_color(tokens::foreground_secondary),
                                        ),
                                )
                                .child(
                                    text(t_args(keys::LANDING_SURAH_AYAHS, move || {
                                        vec![("count", count.into())]
                                    }))
                                    .text_xs()
                                    .text_color(tokens::muted_text),
                                ),
                        ),
                    )
                    .into_element()
            }),
        ))
}

pub fn page() -> impl IntoElement {
    crate::features::downloads::dev_capture();
    follow_notes_owner();
    let data = home_data();
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(
                view()
                    .w_full()
                    .flex_col()
                    .items_center()
                    .child(hero())
                    .child(cards_strip(data)),
            )
            .child(surah_grid())
            .child(footer())
            .text_color(move || theme().foreground),
    )
}
