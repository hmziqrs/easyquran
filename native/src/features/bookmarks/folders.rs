//! The folders panel (web `FoldersPanel.svelte`): add a folder, then the
//! list, default first (never deletable), then manual order. Each row's
//! commands sit in one menu: rename, move up/down, delete. Deleting asks
//! once, with the count, and the page offers Undo right after.

use eq_core::{
    i18n::keys,
    store::bookmarks::{self, BookmarkFolder, FOLDER_NAME_MAX},
};
use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    data::{Shelf, fmt, offer, write},
    row::BOOKMARK_ICON,
};
use crate::{
    stores::locale::{t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        dialog::dialog_card,
        field::text_field,
        layout::{BadgeVariant, badge_of},
        popups::{destructive_menu_entry, dropdown_menu, menu_entry},
        text::section_title,
        tokens,
    },
};

/// `maxlength` of a folder name field.
const NAME_MAX: u32 = FOLDER_NAME_MAX as u32;

/// Creates a folder.
pub(crate) fn create_folder(name: String) {
    write(move |s| bookmarks::create_folder(s, &name), |_| ());
}

/// `reorderFolder`: the non-default folders in their shown order, one
/// moved by `direction`, then a position write per changed slot.
fn reorder(shelf: Signal<Shelf>, id: String, direction: i64) {
    let mut movable: Vec<BookmarkFolder> = shelf.with_untracked(|s| {
        s.folders
            .iter()
            .filter(|f| !f.is_default)
            .cloned()
            .collect()
    });
    let Some(index) = movable.iter().position(|f| f.id == id) else {
        return;
    };
    let target = index as i64 + direction;
    if target < 0 || target >= movable.len() as i64 {
        return;
    }
    let moved = movable.remove(index);
    movable.insert(target as usize, moved);
    let changes: Vec<(String, i64)> = movable
        .iter()
        .enumerate()
        .filter(|(slot, f)| f.position != Some(*slot as i64))
        .map(|(slot, f)| (f.id.clone(), slot as i64))
        .collect();
    write(
        move |s| {
            for (id, slot) in &changes {
                bookmarks::set_folder_position(s, id, Some(*slot))?;
            }
            Ok(())
        },
        |_| (),
    );
}

/// `deleteFolderConfirm`: the count picks the sentence.
fn delete_title(name: &str, count: usize) -> String {
    match count {
        0 => fmt(
            keys::READER_BOOKMARKS_DELETE_FOLDER_CONFIRM_EMPTY,
            &[("name", name.into())],
        ),
        1 => fmt(
            keys::READER_BOOKMARKS_DELETE_FOLDER_CONFIRM_ONE,
            &[("name", name.into())],
        ),
        n => fmt(
            keys::READER_BOOKMARKS_DELETE_FOLDER_CONFIRM,
            &[("name", name.into()), ("count", n.into())],
        ),
    }
}

/// The folder's row commands.
fn folder_menu(
    shelf: Signal<Shelf>,
    folder: ReadSignal<BookmarkFolder>,
    renaming: Signal<Option<String>>,
    rename_value: Signal<String>,
    deleting: Signal<Option<BookmarkFolder>>,
) -> impl IntoElement {
    dynamic(move || {
        let f = folder.get();
        let movable: Vec<String> = shelf.with(|s| {
            s.folders
                .iter()
                .filter(|x| !x.is_default)
                .map(|x| x.id.clone())
                .collect()
        });
        let slot = movable.iter().position(|id| *id == f.id);
        let mut items: Vec<AnyElement> = Vec::new();
        let (id, name) = (f.id.clone(), f.name.clone());
        items.push(
            menu_entry(t(keys::READER_BOOKMARKS_RENAME), move || {
                rename_value.set(name.clone());
                renaming.set(Some(id.clone()));
            })
            .into_element(),
        );
        if let Some(slot) = slot {
            if slot > 0 {
                let id = f.id.clone();
                let label = fmt(
                    keys::READER_BOOKMARKS_MOVE_UP,
                    &[("name", (&f.name).into())],
                );
                items.push(
                    menu_entry(SharedString::from(label), move || {
                        reorder(shelf, id.clone(), -1);
                    })
                    .into_element(),
                );
            }
            if slot + 1 < movable.len() {
                let id = f.id.clone();
                let label = fmt(
                    keys::READER_BOOKMARKS_MOVE_DOWN,
                    &[("name", (&f.name).into())],
                );
                items.push(
                    menu_entry(SharedString::from(label), move || {
                        reorder(shelf, id.clone(), 1);
                    })
                    .into_element(),
                );
            }
        }
        if !f.is_default {
            let target = f.clone();
            items.push(
                destructive_menu_entry(t(keys::READER_BOOKMARKS_DELETE_FOLDER), move || {
                    deleting.set(Some(target.clone()));
                })
                .into_element(),
            );
        }
        dropdown_menu("⋯", items)
    })
}

/// One folder row: its name, the default badge, its count, its menu; or,
/// while renaming, the rename form.
fn folder_row(
    shelf: Signal<Shelf>,
    folder: ReadSignal<BookmarkFolder>,
    renaming: Signal<Option<String>>,
    rename_value: Signal<String>,
    deleting: Signal<Option<BookmarkFolder>>,
) -> impl IntoElement {
    let is_renaming = move || renaming.get().is_some_and(|id| folder.with(|f| f.id == id));
    let submit = move || {
        let name = rename_value.get_untracked();
        if name.trim().is_empty() {
            return;
        }
        let id = folder.with_untracked(|f| f.id.clone());
        write(move |s| bookmarks::rename_folder(s, &id, &name), |_| ());
        renaming.set(None);
    };
    view()
        .w_full()
        .min_h(px(56.0))
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(move || theme().border)
        .child(
            when(is_renaming, move || {
                view()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        text_field(rename_value, "")
                            .label(tr(keys::READER_BOOKMARKS_RENAME_LABEL))
                            .max_length(NAME_MAX)
                            .on_submit(move |_: &str| submit())
                            .flex_1(),
                    )
                    .child(button_with(
                        t(keys::READER_BOOKMARKS_SAVE),
                        ButtonVariant::Primary,
                        ButtonSize::Small,
                        false,
                        None,
                        submit,
                    ))
                    .child(button_with(
                        t(keys::READER_BOOKMARKS_CANCEL),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || renaming.set(None),
                    ))
            })
            .otherwise(move || {
                let count = move || {
                    let id = folder.with(|f| f.id.clone());
                    SharedString::from(shelf.with(|s| s.count_in(&id)).to_string())
                };
                vec![
                    svg(nsh_core::element::ImageSrc::svg_markup(BOOKMARK_ICON))
                        .size(px(15.0))
                        .flex_none()
                        .text_color(tokens::muted_text)
                        .into_element(),
                    text(move || SharedString::from(folder.with(|f| f.name.clone())))
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_sm()
                        .text_color(move || theme().foreground)
                        .into_element(),
                    when(
                        move || folder.with(|f| f.is_default),
                        || {
                            badge_of(
                                t(keys::READER_BOOKMARKS_DEFAULT_BADGE),
                                BadgeVariant::Secondary,
                            )
                        },
                    )
                    .into_element(),
                    text(count)
                        .text_xs()
                        .flex_none()
                        .text_color(tokens::muted_text)
                        .into_element(),
                    folder_menu(shelf, folder, renaming, rename_value, deleting).into_element(),
                ]
            }),
        )
}

/// The delete confirmation: title with the count, body, Cancel / Delete.
/// Confirming deletes, then offers Undo; `on_deleted` gets the id.
fn delete_dialog(
    shelf: Signal<Shelf>,
    deleting: Signal<Option<BookmarkFolder>>,
    on_deleted: impl Fn(&str) + Copy + 'static,
) -> impl IntoElement {
    dynamic(move || {
        let folder = deleting.get()?;
        let open = signal(true);
        let count = untracked(|| shelf.with(|s| s.count_in(&folder.id)));
        let title = SharedString::from(delete_title(&folder.name, count));
        let cancel = move || {
            open.set(false);
            deleting.set(None);
        };
        let id = folder.id.clone();
        Some(dialog_card(
            open,
            title,
            cancel,
            || {
                text(t(keys::READER_BOOKMARKS_DELETE_FOLDER_BODY))
                    .text_sm()
                    .text_color(tokens::foreground_secondary)
                    .into_element()
            },
            move || {
                let id = id.clone();
                view()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(button_with(
                        t(keys::READER_BOOKMARKS_CANCEL),
                        ButtonVariant::Ghost,
                        ButtonSize::Medium,
                        false,
                        None,
                        cancel,
                    ))
                    .child(button_with(
                        t(keys::READER_BOOKMARKS_DELETE_FOLDER),
                        ButtonVariant::Danger,
                        ButtonSize::Medium,
                        false,
                        None,
                        move || {
                            let id = id.clone();
                            deleting.set(None);
                            on_deleted(&id);
                            write(
                                move |s| bookmarks::delete_folder(s, &id),
                                |deleted| {
                                    let Some(deleted) = deleted else {
                                        return;
                                    };
                                    offer(
                                        tr(keys::READER_BOOKMARKS_FOLDER_DELETED),
                                        tr(keys::READER_BOOKMARKS_UNDO),
                                        move || {
                                            let deleted = deleted.clone();
                                            write(
                                                move |s| bookmarks::restore_folder(s, &deleted),
                                                |_| (),
                                            );
                                        },
                                    );
                                },
                            );
                        },
                    ))
                    .into_element()
            },
        ))
    })
}

/// The panel. `on_deleted` runs when a folder goes (the page resets its
/// filter when it pointed there).
pub(crate) fn folders_panel(
    shelf: Signal<Shelf>,
    on_deleted: impl Fn(&str) + Copy + 'static,
) -> impl IntoElement {
    let new_name = signal(String::new());
    let renaming = signal(None::<String>);
    let rename_value = signal(String::new());
    let deleting = signal(None::<BookmarkFolder>);
    let create = move || {
        let name = new_name.get_untracked();
        if name.trim().is_empty() {
            return;
        }
        create_folder(name);
        new_name.set(String::new());
    };
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .child(section_title(t(keys::READER_BOOKMARKS_FOLDERS_HEADING)))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text_field(new_name, tr(keys::READER_BOOKMARKS_NEW_FOLDER_PLACEHOLDER))
                        .label(tr(keys::READER_BOOKMARKS_NEW_FOLDER_LABEL))
                        .max_length(NAME_MAX)
                        .on_submit(move |_: &str| create())
                        .flex_1(),
                )
                .child(button_with(
                    t(keys::READER_BOOKMARKS_CREATE_FOLDER),
                    ButtonVariant::Secondary,
                    ButtonSize::Small,
                    move || new_name.get().trim().is_empty(),
                    None,
                    create,
                )),
        )
        .child(
            view()
                .w_full()
                .flex_col()
                .overflow_hidden()
                .rounded(tokens::radius_md)
                .border_1()
                .border_color(tokens::border_strong)
                .bg(tokens::surface)
                .child(each(
                    move || shelf.with(|s| s.folders.clone()),
                    |f| f.id.clone(),
                    move |folder, _| folder_row(shelf, folder, renaming, rename_value, deleting),
                )),
        )
        .child(delete_dialog(shelf, deleting, on_deleted))
}
