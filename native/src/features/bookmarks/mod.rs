//! Bookmarks, folders, sync indicator and notices (PARITY F1, F4), and the
//! verse-level pieces the reader mounts (bookmark toggle, note editor, pins
//! drawer). Contract: `pub fn page() -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/(application)/bookmarks/+page.svelte`: header
//! with the sync line (signed in) or "Saved on this device"; the empty
//! state; the folders panel; the bulk-selection bar; sort (date / folder /
//! surah) and folder filter; rows grouped by folder or flat; the sign-in
//! note when signed out. Removals and folder deletes are optimistic with
//! one Undo (the feedback bar stands in for the web's action toast).
//!
//! Public API for the reader:
//! - [`bookmark_toggle`] / [`bookmark_toggle_in`]: the verse's bookmark button
//!   with the "Save to…" folder picker.
//! - [`note_editor`] / [`note_editor_in`] / [`note_sheet`]: the verse note.
//! - [`pins_drawer`]: the pinned verses trigger + sheet.
//! - [`feedback_bar`]: mount once per surface that toggles bookmarks.
//! - [`sync_notices`]: the app-wide sync notices.

mod data;
mod folders;
mod note;
mod pins;
mod row;
mod sync_status;
mod toggle;

use std::collections::BTreeSet;

use eq_core::{
    api::AuthPhase,
    i18n::keys,
    store::bookmarks::{self, Bookmark},
};
use nsh_base::theme;
use nsh_core::prelude::*;

pub use data::feedback_bar;
pub(crate) use data::{
    Shelf, ayah_count, fmt, medium_date, now_ms, offer, open_verse, shelf, source_label, store,
    surah_name, write,
};
pub use note::{follow_notes_owner, note_editor, note_editor_in, note_sheet};
pub use pins::pins_drawer;
pub(crate) use row::{BOOKMARK_ICON, RowActions, bookmark_row};
pub use sync_status::sync_notices;
pub use toggle::{bookmark_toggle, bookmark_toggle_in};

use crate::{
    app::auth_store,
    shell::{nav, routes::Route},
    stores::locale::{t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{page_body, page_insets, skeleton_of},
        popups::{dropdown_menu, menu_entry},
        select::{choice, select_field},
        text::{body, muted, page_heading},
        tokens,
    },
};

/// Authed with nothing yet: the skeleton waits at most this long for the
/// first round.
const FIRST_SYNC_SKELETON: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Date,
    Folder,
    Surah,
}

fn sort_mode(value: Option<SharedString>) -> SortMode {
    match value.as_deref() {
        Some("date") => SortMode::Date,
        Some("surah") => SortMode::Surah,
        _ => SortMode::Folder,
    }
}

/// One folder's section in the grouped view.
#[derive(Clone, PartialEq)]
struct Group {
    id: String,
    label: String,
    rows: Vec<Bookmark>,
}

fn by_newest(a: &Bookmark, b: &Bookmark) -> std::cmp::Ordering {
    b.updated_at.cmp(&a.updated_at)
}

fn by_verse(a: &Bookmark, b: &Bookmark) -> std::cmp::Ordering {
    (a.surah, a.ayah).cmp(&(b.surah, b.ayah))
}

/// Optimistic removal with one Undo (rapid removes share it).
pub(crate) fn remove_rows(ids: Vec<String>) {
    write(
        move |s| bookmarks::remove_many(s, &ids),
        |removed| {
            if removed.is_empty() {
                return;
            }
            let message = if removed.len() == 1 {
                tr(keys::READER_BOOKMARKS_REMOVED).to_owned()
            } else {
                fmt(
                    keys::READER_BOOKMARKS_REMOVED_MANY,
                    &[("count", removed.len().into())],
                )
            };
            offer(message, tr(keys::READER_BOOKMARKS_UNDO), move || {
                let rows = removed.clone();
                write(move |s| bookmarks::restore(s, &rows), |_| ());
            });
        },
    );
}

fn move_one(id: String, folder_id: String) {
    write(
        move |s| bookmarks::move_to_folder(s, &id, &folder_id),
        |_| (),
    );
}

fn remove_one(id: String) {
    remove_rows(vec![id]);
}

/// Bulk move with one Undo: Undo puts every moved row back in its folder.
fn move_selected(selected: Signal<BTreeSet<String>>, folder_id: String, folder_name: String) {
    let ids: Vec<String> = selected.get_untracked().into_iter().collect();
    selected.set(BTreeSet::new());
    write(
        move |s| bookmarks::move_many(s, &ids, &folder_id),
        move |moved| {
            if moved.is_empty() {
                return;
            }
            let message = fmt(
                keys::READER_BOOKMARKS_MOVED_MANY,
                &[
                    ("count", moved.len().into()),
                    ("folder", folder_name.into()),
                ],
            );
            offer(message, tr(keys::READER_BOOKMARKS_UNDO), move || {
                let rows = moved.clone();
                write(move |s| bookmarks::restore(s, &rows), |_| ());
            });
        },
    );
}

/// The bulk-selection bar: count, "Move selected to…", remove, clear.
fn selection_bar(shelf: Signal<Shelf>, selected: Signal<BTreeSet<String>>) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .px_4()
        .py(px(10.0))
        .rounded(tokens::radius_xl)
        .border_1()
        .border_color(tokens::border_strong)
        .bg(tokens::background_subtle)
        .child(
            text(move || {
                SharedString::from(fmt(
                    keys::READER_BOOKMARKS_SELECTED,
                    &[("count", selected.with(BTreeSet::len).into())],
                ))
            })
            .text_sm()
            .text_color(tokens::foreground_secondary),
        )
        .child(dynamic(move || {
            let entries: Vec<AnyElement> = shelf.with(|s| {
                s.folders
                    .iter()
                    .map(|f| {
                        let (id, name) = (f.id.clone(), f.name.clone());
                        menu_entry(SharedString::from(f.name.clone()), move || {
                            move_selected(selected, id.clone(), name.clone());
                        })
                        .into_element()
                    })
                    .collect()
            });
            dropdown_menu(t(keys::READER_BOOKMARKS_MOVE_SELECTED_TO), entries)
        }))
        .child(button_with(
            t(keys::READER_BOOKMARKS_REMOVE_SELECTED),
            ButtonVariant::Danger,
            ButtonSize::Small,
            false,
            None,
            move || {
                let ids: Vec<String> = selected.get_untracked().into_iter().collect();
                selected.set(BTreeSet::new());
                remove_rows(ids);
            },
        ))
        .child(button_with(
            t(keys::READER_BOOKMARKS_CLEAR_SELECTION),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            false,
            None,
            move || selected.set(BTreeSet::new()),
        ))
}

/// A framed list of rows.
fn row_list(rows: impl Fn() -> Vec<Bookmark> + 'static, actions: RowActions) -> impl IntoElement {
    view()
        .w_full()
        .flex_col()
        .overflow_hidden()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(tokens::border_strong)
        .bg(tokens::surface)
        .child(each(
            rows,
            |b| b.id.clone(),
            move |b, _| bookmark_row(b, actions),
        ))
}

/// The folder filter: "All folders" or one folder.
fn filter_menu(
    shelf: Signal<Shelf>,
    filter: Signal<Option<String>>,
    selected: Signal<BTreeSet<String>>,
) -> impl IntoElement {
    dynamic(move || {
        let folders = shelf.with(|s| s.folders.clone());
        let current = filter
            .get()
            .and_then(|id| folders.iter().find(|f| f.id == id).map(|f| f.name.clone()))
            .unwrap_or_else(|| tr(keys::READER_BOOKMARKS_FILTER_ALL).to_owned());
        // Selection is for the rows on screen: a new filter starts clean.
        let mut entries: Vec<AnyElement> = vec![
            menu_entry(t(keys::READER_BOOKMARKS_FILTER_ALL), move || {
                filter.set(None);
                selected.set(BTreeSet::new());
            })
            .into_element(),
        ];
        entries.extend(folders.into_iter().map(|f| {
            let id = f.id.clone();
            menu_entry(SharedString::from(f.name), move || {
                filter.set(Some(id.clone()));
                selected.set(BTreeSet::new());
            })
            .into_element()
        }));
        dropdown_menu(SharedString::from(current), entries)
    })
}

pub fn page() -> impl IntoElement {
    follow_notes_owner();
    let auth = auth_store();
    let sync = crate::features::bookmarks::store();
    let shelf = shelf();
    let sort = signal(Some(SharedString::from("folder")));
    let filter = signal(None::<String>);
    let selected = signal(BTreeSet::<String>::new());
    let window_over = signal(false);
    let _ = set_timeout(FIRST_SYNC_SKELETON, move || {
        let _ = window_over.try_set(true);
    });

    let auth_unknown = move || matches!(auth.phase(), AuthPhase::Unknown);
    let authed = move || auth.user().is_some();
    let has_any = move || shelf.with(|s| !s.bookmarks.is_empty());
    let loading = move || {
        let waiting_first_sync =
            authed() && sync.status().last_sync_at.is_none() && !has_any() && !window_over.get();
        auth_unknown() || !shelf.with(|s| s.loaded) || waiting_first_sync
    };

    let filtered = move || {
        let f = filter.get();
        shelf.with(|s| {
            s.bookmarks
                .iter()
                .filter(|b| f.as_ref().is_none_or(|id| b.folder_id == *id))
                .cloned()
                .collect::<Vec<_>>()
        })
    };
    let flat_rows = move || {
        let mut rows = filtered();
        match sort_mode(sort.get()) {
            SortMode::Date => rows.sort_by(by_newest),
            SortMode::Surah => rows.sort_by(by_verse),
            SortMode::Folder => rows.clear(),
        }
        rows
    };
    let groups = move || {
        let f = filter.get();
        let rows = filtered();
        shelf.with(|s| {
            s.folders
                .iter()
                .filter(|folder| f.as_ref().is_none_or(|id| folder.id == *id))
                .map(|folder| {
                    let mut in_folder: Vec<Bookmark> = rows
                        .iter()
                        .filter(|b| b.folder_id == folder.id)
                        .cloned()
                        .collect();
                    in_folder.sort_by(by_newest);
                    Group {
                        id: folder.id.clone(),
                        label: folder.name.clone(),
                        rows: in_folder,
                    }
                })
                .collect::<Vec<_>>()
        })
    };

    // The folder pill shows on flat lists over every folder.
    let show_folder = signal(false);
    effect(move || {
        show_folder.set(filter.get().is_none() && sort_mode(sort.get()) != SortMode::Folder);
    });
    let actions = RowActions {
        shelf,
        show_folder,
        movable: true,
        selected: Some(selected),
        on_move: move_one,
        on_remove: remove_one,
    };
    let in_folder_actions = RowActions {
        show_folder: signal(false),
        ..actions
    };

    let header = view()
        .w_full()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .child(page_heading(t(keys::READER_BOOKMARKS_TITLE)))
        .child(dynamic(move || {
            if auth_unknown() {
                return None;
            }
            if authed() {
                return Some(sync_status::sync_indicator(shelf).into_element());
            }
            Some(muted(t(keys::READER_BOOKMARKS_LOCAL_ONLY)).into_element())
        }));

    let empty = move || {
        when(
            move || !has_any(),
            || {
                view()
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
            },
        )
    };

    let controls = move || {
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(muted(t(keys::READER_BOOKMARKS_SORT_LABEL)))
                    .child(select_field(
                        tr(keys::READER_BOOKMARKS_SORT_LABEL),
                        vec![
                            choice("date", tr(keys::READER_BOOKMARKS_SORT_DATE)),
                            choice("folder", tr(keys::READER_BOOKMARKS_SORT_FOLDER)),
                            choice("surah", tr(keys::READER_BOOKMARKS_SORT_SURAH)),
                        ],
                        sort,
                        tr(keys::READER_BOOKMARKS_SORT_FOLDER),
                    )),
            )
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(muted(t(keys::READER_BOOKMARKS_FILTER_LABEL)))
                    .child(filter_menu(shelf, filter, selected)),
            )
    };

    let list = move || {
        dynamic(move || {
            if sort_mode(sort.get()) == SortMode::Folder {
                return view()
                    .w_full()
                    .flex_col()
                    .gap_6()
                    .child(each(
                        groups,
                        |g| g.id.clone(),
                        move |group, _| {
                            let rows = move || group.with(|g| g.rows.clone());
                            view()
                                .w_full()
                                .flex_col()
                                .gap_2()
                                .child(
                                    view()
                                        .flex_row()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            text(move || {
                                                SharedString::from(group.with(|g| g.label.clone()))
                                            })
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(tokens::muted_text),
                                        )
                                        .child(
                                            text(move || {
                                                SharedString::from(
                                                    group.with(|g| g.rows.len()).to_string(),
                                                )
                                            })
                                            .text_xs()
                                            .text_color(tokens::muted_text),
                                        ),
                                )
                                .child(
                                    when(
                                        move || group.with(|g| g.rows.is_empty()),
                                        || muted(t(keys::READER_BOOKMARKS_FOLDER_EMPTY)),
                                    )
                                    .otherwise(move || row_list(rows, in_folder_actions)),
                                )
                        },
                    ))
                    .into_element();
            }
            row_list(flat_rows, actions).into_element()
        })
    };

    let sign_in_note = || {
        view()
            .w_full()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py(px(14.0))
            .rounded(tokens::radius_xl)
            .border_1()
            .border_color(tokens::border_strong)
            .bg(tokens::background_subtle)
            .child(
                text(t(keys::READER_BOOKMARKS_SIGN_IN_NOTE))
                    .text_sm()
                    .flex_1()
                    .text_color(tokens::foreground_secondary),
            )
            .child(button_with(
                t(keys::READER_BOOKMARKS_SIGN_IN),
                ButtonVariant::Primary,
                ButtonSize::Small,
                false,
                None,
                || nav::push(Route::Login),
            ))
    };

    let content = move || {
        view()
            .w_full()
            .flex_col()
            .gap_6()
            .child(empty())
            .child(folders::folders_panel(shelf, move |id| {
                if filter.get_untracked().as_deref() == Some(id) {
                    filter.set(None);
                }
            }))
            .child(when(
                move || selected.with(|s| !s.is_empty()),
                move || selection_bar(shelf, selected),
            ))
            .child(when(has_any, move || {
                view()
                    .w_full()
                    .flex_col()
                    .gap_6()
                    .child(controls())
                    .child(list())
            }))
            .child(when(move || !authed() && !auth_unknown(), sign_in_note))
    };

    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(sync_notices())
            .child(header)
            .child(feedback_bar())
            .child(
                when(loading, || {
                    view()
                        .flex_col()
                        .gap_3()
                        .label(tr(keys::READER_BOOKMARKS_LOADING))
                        .child(skeleton_of(64.0))
                        .child(skeleton_of(64.0))
                        .child(skeleton_of(64.0))
                })
                .otherwise(content),
            )
            .text_color(move || theme().foreground),
    )
}
