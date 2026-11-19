//! "Yours" hub and card detail (PARITY G3, G2, F2, F3). Contract:
//! `pub fn page() -> impl IntoElement`,
//! `pub fn card_page(id: String) -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/(application)/yours/+page.svelte`: the title,
//! the index links and Edit / Done, Overview / Notes tabs, the first-run
//! reading choice, the hidden widgets while editing. Overview: the
//! reading place, then the widget column in the stored layout's order
//! (`widgets`). Notes: the full library.

pub(crate) mod cards;
pub(crate) mod continue_threads;
pub(crate) mod goals;
pub(crate) mod history;
pub(crate) mod insights;
pub(crate) mod notes;
pub(crate) mod reading_bookmark;
mod widgets;

use eq_core::i18n::keys;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    features::bookmarks::{
        BOOKMARK_ICON, RowActions, Shelf, bookmark_row, feedback_bar, follow_notes_owner,
        sync_notices, write,
    },
    shell::{nav, routes::Route},
    stores::{
        home::{HomeData, home_data},
        locale::t,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{card, eyebrow, page_body, page_insets, skeleton_of},
        text::{body, page_heading},
        tokens,
    },
};

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

/// The Bookmarks preview: the count (and the widget's edit controls), the
/// empty state, the first five rows (folder pill, no move), "View all".
pub(crate) fn bookmarks_preview(
    shelf: Signal<Shelf>,
    controls: Option<AnyElement>,
) -> impl IntoElement {
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
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        text(move || SharedString::from(count().to_string()))
                            .text_xs()
                            .text_color(tokens::muted_text),
                    )
                    .child(controls)
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

fn edit_toggle(editing: Signal<bool>) -> impl IntoElement {
    widgets::pill(
        move || {
            if editing.get() {
                return t(keys::READER_HOME_DONE)();
            }
            t(keys::READER_HOME_EDIT)()
        },
        move || editing.set(!editing.get_untracked()),
    )
}

fn overview(
    data: Signal<HomeData>,
    editing: Signal<bool>,
    notes_view: Signal<bool>,
) -> impl IntoElement {
    view()
        .w_full()
        .flex_col()
        .gap_6()
        .child(reading_bookmark::reading_bookmark_card())
        .child(widgets::widget_column(data, editing, notes_view))
}

pub fn page() -> impl IntoElement {
    follow_notes_owner();
    let data = home_data();
    let editing = signal(false);
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
                    .child(
                        view()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap_1p5()
                            .child(index_links())
                            .child(edit_toggle(editing)),
                    ),
            )
            .child(view_tabs(notes_view))
            .child(widgets::reading_choice_line())
            .child(widgets::hidden_widgets(data, editing))
            .child(feedback_bar())
            .child(
                when(notes_view, notes::notes_library)
                    .otherwise(move || overview(data, editing, notes_view)),
            )
            .text_color(move || theme().foreground),
    )
}

pub fn card_page(id: String) -> impl IntoElement {
    cards::card_page(id)
}
