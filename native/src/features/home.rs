//! Home (PARITY A3; web `(marketing)/+page.svelte`, `lib/home/LandingCards.svelte`,
//! and the Yours widget column it shares, `layout.svelte.ts`,
//! `reading-choice.svelte.ts`). Contract: `pub fn page() -> impl IntoElement`.
//!
//! The landing hero (bismillah, iqra, the search pill that opens the
//! palette, often-opened surahs), the first-run reading choice, the widget
//! column in the stored layout's order (Customize: move, hide, show, the
//! recents count), the surah index and the footer. The cards widget is the
//! landing strip: the next card to recite, then up to four tiles.

use eq_core::{
    home::next_zikr::next_zikr_card,
    i18n::keys,
    quran::{catalogue, meta::meta},
    routes::ReaderScope,
    store::home::{HomeWidget, RECENTS_COUNT_CHOICES},
};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    features::{
        bookmarks::{follow_notes_owner, shelf},
        daily_ayah::daily_widget,
        index::reader_route,
        marketing::footer,
        yours::{
            self,
            cards::{card_grid, card_link, open_link, strip_more, tile_copy},
            continue_threads::continue_threads,
            goals::reading_goals,
            history::thread_history,
            insights::reading_insights,
            notes::notes_preview,
            reading_bookmark::reading_bookmark_card,
        },
    },
    shell::{nav, palette, routes::Route},
    stores::{
        home::{
            self as home_store, HomeData, ReadingChoice, choose_reading, home_data, now_minute,
            reading_choice, suggested_choice, translation_for_language,
        },
        locale::{t, t_args, tr},
    },
    ui::{
        arabic::{ArabicStyle, QuranScript, plain_ayah},
        brand::bismillah,
        button::{ButtonSize, ButtonVariant, button_with, choice_button},
        layout::{eyebrow, page_body, page_insets, pressable_card},
        text::muted,
        tokens,
    },
};

const OFTEN_OPENED: [u16; 5] = [1, 18, 36, 55, 67];
const STRIP_LIMIT: usize = 4;

fn widget_title(id: &str) -> eq_core::i18n::Key {
    match id {
        "continue-reading" => keys::READER_INDEX_YOURS_CONTINUE,
        "recents" => keys::READER_THREADS_HISTORY,
        "notes" => keys::READER_WIDGET_NOTES_TITLE,
        "most-read" => keys::READER_WIDGET_MOST_READ_TITLE,
        "daily-ayah" => keys::READER_WIDGET_DAILY_AYAH_TITLE,
        "cards" => keys::READER_CARDS_TITLE,
        _ => keys::NAV_BOOKMARKS,
    }
}

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

// --- first-run reading choice ---------------------------------------------------

fn choice_options() -> Vec<(ReadingChoice, String)> {
    let mut out = vec![(
        ReadingChoice::Arabic,
        tr(keys::READER_HOME_READ_IN_ARABIC).to_owned(),
    )];
    let mut push = |choice: ReadingChoice| {
        let ReadingChoice::Translation(id) = &choice else {
            return;
        };
        if out.iter().any(|(c, _)| *c == choice) {
            return;
        }
        let language = catalogue::translation(id)
            .map(|t| t.language.clone())
            .unwrap_or_default();
        let label = crate::features::bookmarks::fmt(
            keys::READER_HOME_READ_IN_WITH,
            &[("language", language.into())],
        );
        out.push((choice, label));
    };
    push(suggested_choice());
    if let Some(en) = translation_for_language("en") {
        push(ReadingChoice::Translation(en));
    }
    out
}

fn reading_choice_line() -> impl IntoElement {
    let options = choice_options();
    when(move || reading_choice().is_none() && options.len() > 1, {
        let options = choice_options();
        move || {
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(eyebrow(t(keys::READER_HOME_READ_IN)))
                .children(options.iter().cloned().map(|(choice, label)| {
                    pill(SharedString::from(label), move || {
                        choose_reading(choice.clone())
                    })
                    .into_element()
                }))
                .into_element()
        }
    })
}

// --- widgets ---------------------------------------------------------------------

fn edit_controls(id: &'static str, index: usize, last: usize) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(button_with(
            "↑",
            ButtonVariant::Outline,
            ButtonSize::Small,
            index == 0,
            Some(tr(keys::READER_HOME_MOVE_UP)),
            move || {
                home_store::move_widget(id, -1);
            },
        ))
        .child(button_with(
            "↓",
            ButtonVariant::Outline,
            ButtonSize::Small,
            index == last,
            Some(tr(keys::READER_HOME_MOVE_DOWN)),
            move || {
                home_store::move_widget(id, 1);
            },
        ))
        .child(pill(t(keys::READER_HOME_HIDE), move || {
            home_store::toggle_widget(id)
        }))
}

fn section(
    id: &'static str,
    controls: Option<AnyElement>,
    trailing: Option<AnyElement>,
    content: AnyElement,
) -> AnyElement {
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(
            view()
                .w_full()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(eyebrow(t(widget_title(id))))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(trailing)
                        .child(controls),
                ),
        )
        .child(content)
        .into_element()
}

/// The landing strip: next card, tiles, "{n} more on Yours".
fn cards_strip(data: Signal<HomeData>) -> AnyElement {
    dynamic(move || {
        let cards = data.with(|d| d.cards.clone());
        if cards.is_empty() {
            return muted(t(keys::READER_CARDS_EMPTY)).into_element();
        }
        let extra = cards.len().saturating_sub(STRIP_LIMIT);
        let next = next_zikr_card(&cards, "", "", now_minute()).and_then(|c| {
            let link = card_link(c)?;
            Some((tile_copy(c).title, link))
        });
        view()
            .w_full()
            .flex_col()
            .gap_2()
            .child(next.map(|(title, link)| {
                button_with(
                    t_args(keys::READER_ZIKR_NEXT, move || {
                        vec![("title", title.clone().into())]
                    }),
                    ButtonVariant::Secondary,
                    ButtonSize::Medium,
                    false,
                    None,
                    move || open_link(link.clone()),
                )
            }))
            .child(card_grid(data, STRIP_LIMIT, false))
            .child(view().flex_row().child(pill(
                move || {
                    if extra > 0 {
                        return SharedString::from(format!("{} →", strip_more(extra)()));
                    }
                    SharedString::from(format!("{} →", tr(keys::READER_CARDS_STRIP_TITLE)))
                },
                || nav::push(Route::Yours),
            )))
            .into_element()
    })
    .into_element()
}

fn widget_body(w: &HomeWidget, data: Signal<HomeData>, editing: bool) -> AnyElement {
    match w.id.as_str() {
        "continue-reading" => view()
            .w_full()
            .flex_col()
            .gap_3()
            .child(reading_bookmark_card())
            .child(continue_threads())
            .into_element(),
        "recents" => {
            let count = usize::try_from(w.count.unwrap_or(5)).unwrap_or(5);
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(editing.then(|| {
                    view()
                        .flex_row()
                        .gap_1()
                        .children(RECENTS_COUNT_CHOICES.map(|c| {
                            choice_button(
                                SharedString::from(c.to_string()),
                                tr(keys::READER_THREADS_HISTORY),
                                usize::try_from(c).ok() == Some(count),
                                false,
                                move || home_store::set_widget_count("recents", c),
                            )
                            .into_element()
                        }))
                }))
                .child(thread_history(count))
                .into_element()
        }
        "bookmarks" => yours::bookmarks_preview(shelf()).into_element(),
        "notes" => notes_preview(|| nav::push(Route::Yours)).into_element(),
        "most-read" => view()
            .w_full()
            .flex_col()
            .gap_4()
            .child(reading_insights())
            .child(reading_goals())
            .into_element(),
        "daily-ayah" => daily_widget().into_element(),
        _ => cards_strip(data),
    }
}

fn widgets(data: Signal<HomeData>, editing: Signal<bool>) -> impl IntoElement {
    dynamic(move || {
        let layout = data.with(|d| d.layout.clone());
        let editing_now = editing.get();
        let last = layout.len().saturating_sub(1);
        let sections: Vec<AnyElement> = layout
            .iter()
            .enumerate()
            .filter(|(_, w)| w.visible)
            .filter_map(|(index, w)| {
                let id: &'static str = eq_core::store::home::HOME_WIDGETS
                    .iter()
                    .find(|s| s.0 == w.id)?
                    .0;
                let controls = editing_now.then(|| edit_controls(id, index, last).into_element());
                // The previews that carry their own heading render bare.
                let body = widget_body(w, data, editing_now);
                Some(match id {
                    "bookmarks" | "notes" => view()
                        .w_full()
                        .flex_col()
                        .gap_1()
                        .child(controls.map(|c| view().flex_row().justify_end().child(c)))
                        .child(body)
                        .into_element(),
                    _ => section(id, controls, None, body),
                })
            })
            .collect();
        let hidden: Vec<AnyElement> = if editing_now {
            layout
                .iter()
                .filter(|w| !w.visible)
                .filter_map(|w| {
                    eq_core::store::home::HOME_WIDGETS
                        .iter()
                        .find(|s| s.0 == w.id)
                        .map(|s| s.0)
                })
                .map(|id| {
                    pill(
                        SharedString::from(format!("{} +", tr(widget_title(id)))),
                        move || home_store::toggle_widget(id),
                    )
                    .into_element()
                })
                .collect()
        } else {
            Vec::new()
        };
        view()
            .w_full()
            .flex_col()
            .gap_6()
            .child(editing_now.then(|| {
                view()
                    .w_full()
                    .flex_col()
                    .gap_2()
                    .child(eyebrow(t(keys::READER_HOME_HIDDEN)))
                    .child(view().flex_row().flex_wrap().gap_1p5().children(hidden))
            }))
            .children(sections)
            .into_element()
    })
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
    let editing = signal(false);
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(hero())
            .child(reading_choice_line())
            .child(view().w_full().flex_row().justify_end().child(pill(
                move || {
                    t(if editing.get() {
                        keys::READER_HOME_DONE
                    } else {
                        keys::READER_HOME_EDIT
                    })()
                },
                move || editing.set(!editing.get_untracked()),
            )))
            .child(widgets(data, editing))
            .child(surah_grid())
            .child(footer())
            .text_color(move || theme().foreground),
    )
}
