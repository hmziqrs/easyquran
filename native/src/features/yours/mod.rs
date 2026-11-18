//! "Yours" hub and card detail (PARITY G3 partial, G2, F2, F3). Contract:
//! `pub fn page() -> impl IntoElement`,
//! `pub fn card_page(id: String) -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/(application)/yours/+page.svelte`: the title,
//! the index links, Overview / Notes tabs. Overview: reading place,
//! Continue, History, Bookmarks (first five), Notes (first five), then the
//! goals, insights and cards sections the next slice fills. Notes: the
//! full library. The web's widget layout editing (`homeLayout`) and the
//! first-run reading choice are not here (see the slice report).

pub(crate) mod cards;
pub(crate) mod continue_threads;
pub(crate) mod goals;
pub(crate) mod history;
pub(crate) mod insights;
pub(crate) mod notes;
pub(crate) mod reading_bookmark;

use eq_core::i18n::keys;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    features::bookmarks::{
        BOOKMARK_ICON, RowActions, Shelf, bookmark_row, feedback_bar, follow_notes_owner, shelf,
        sync_notices, write,
    },
    shell::{nav, routes::Route},
    stores::locale::t,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{card, eyebrow, page_body, page_insets, skeleton_of},
        text::{body, page_heading},
        tokens,
    },
};

/// History shows this many rows until "Show more" (the web's default
/// `recents` widget count).
const HISTORY_LIMIT: usize = 5;

/// Reading goals (G4): filled by the goals slice.
pub fn goals_section() -> impl IntoElement {
    goals::reading_goals()
}

/// Reading insights (G5): filled by the insights slice.
pub fn insights_section() -> impl IntoElement {
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(section_head(t(keys::READER_WIDGET_MOST_READ_TITLE), None))
        .child(insights::reading_insights())
}

/// Home cards (G6): filled by the cards slice.
pub fn cards_section() -> impl IntoElement {
    cards::cards_section(None)
}

fn section_head(
    title: impl IntoReactive<SharedString>,
    trailing: Option<AnyElement>,
) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(eyebrow(title))
        .child(trailing)
}

fn remove_now(id: String) {
    // The overview removes without Undo, as the web's preview does.
    write(move |s| eq_core::store::bookmarks::remove(s, &id), |_| ());
}

fn move_none(_: String, _: String) {}

/// The Bookmarks preview: the count, the empty state, the first five rows
/// (folder pill, no move), "View all".
pub(crate) fn bookmarks_preview(shelf: Signal<Shelf>) -> impl IntoElement {
    let count = move || shelf.with(|s| s.bookmarks.len());
    let actions = RowActions {
        shelf,
        show_folder: signal(true),
        movable: false,
        selected: None,
        on_move: move_none,
        on_remove: remove_now,
    };
    let rows = move || shelf.with(|s| s.bookmarks.iter().take(5).cloned().collect::<Vec<_>>());
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(section_head(
            t(keys::NAV_BOOKMARKS),
            Some(
                text(move || SharedString::from(count().to_string()))
                    .text_xs()
                    .text_color(tokens::muted_text)
                    .into_element(),
            ),
        ))
        .child(dynamic(move || {
            if !shelf.with(|s| s.loaded) {
                return view()
                    .flex_col()
                    .gap_2()
                    .child(skeleton_of(44.0))
                    .child(skeleton_of(44.0))
                    .into_element();
            }
            if count() == 0 {
                return card()
                    .flex_row()
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        view()
                            .size(px(44.0))
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .rounded(tokens::radius_sm)
                            .bg(move || tokens::hue_soft(2))
                            .child(
                                svg(nsh_core::element::ImageSrc::svg_markup(BOOKMARK_ICON))
                                    .size(px(20.0))
                                    .text_color(move || tokens::hue_legible(2)),
                            ),
                    )
                    .child(
                        view()
                            .flex_1()
                            .child(body(t(keys::READER_BOOKMARKS_EMPTY_PAGE))),
                    )
                    .into_element();
            }
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(
                    view()
                        .w_full()
                        .flex_col()
                        .overflow_hidden()
                        .rounded(tokens::radius_md)
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(tokens::surface)
                        .child(each(
                            rows,
                            |b| b.id.clone(),
                            move |b, _| bookmark_row(b, actions),
                        )),
                )
                .child(view().flex_row().child(button_with(
                    t(keys::READER_INDEX_YOURS_VIEW_ALL),
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    false,
                    None,
                    || nav::push(Route::Bookmarks),
                )))
                .into_element()
        }))
}

/// The index links (surahs, juz, pages, bookmarks).
fn index_links() -> impl IntoElement {
    let link = |label, route: fn() -> Route| {
        button_with(
            t(label),
            ButtonVariant::Outline,
            ButtonSize::Small,
            false,
            None,
            move || nav::push(route()),
        )
    };
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1p5()
        .child(link(keys::READER_INDEX_SURAHS_TITLE, || Route::SurahIndex))
        .child(link(keys::READER_INDEX_JUZ_TITLE, || Route::JuzIndex))
        .child(link(keys::READER_INDEX_PAGES_TITLE, || Route::PagesIndex))
        .child(link(keys::NAV_BOOKMARKS, || Route::Bookmarks))
}

/// Overview / Notes.
fn view_tabs(notes_view: Signal<bool>) -> impl IntoElement {
    let tab = move |label, notes: bool| {
        dynamic(move || {
            let variant = if notes_view.get() == notes {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            };
            button_with(
                t(label),
                variant,
                ButtonSize::Small,
                false,
                None,
                move || notes_view.set(notes),
            )
        })
    };
    view()
        .flex_row()
        .gap_1()
        .child(tab(keys::READER_YOURS_TAB_OVERVIEW, false))
        .child(tab(keys::READER_YOURS_TAB_NOTES, true))
}

fn overview(notes_view: Signal<bool>) -> impl IntoElement {
    let shelf = shelf();
    view()
        .w_full()
        .flex_col()
        .gap_6()
        .child(reading_bookmark::reading_bookmark_card())
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(section_head(t(keys::READER_INDEX_YOURS_CONTINUE), None))
                .child(continue_threads::continue_threads()),
        )
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(section_head(t(keys::READER_THREADS_HISTORY), None))
                .child(history::thread_history(HISTORY_LIMIT)),
        )
        .child(bookmarks_preview(shelf))
        .child(notes::notes_preview(move || notes_view.set(true)))
        .child(insights_section())
        .child(goals_section())
        .child(cards_section())
}

pub fn page() -> impl IntoElement {
    follow_notes_owner();
    let notes_view = signal(false);
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(sync_notices())
            .child(
                view()
                    .w_full()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(page_heading(t(keys::READER_INDEX_YOURS_TITLE)))
                    .child(index_links()),
            )
            .child(view_tabs(notes_view))
            .child(feedback_bar())
            .child(when(notes_view, notes::notes_library).otherwise(move || overview(notes_view)))
            .text_color(move || theme().foreground),
    )
}

pub fn card_page(id: String) -> impl IntoElement {
    cards::card_page(id)
}
