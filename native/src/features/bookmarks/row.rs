//! One row for every bookmark surface (web `BookmarkRow.svelte`): the
//! verse link (surah · verse, range suffix), the folder pill, the note mark
//! and "Saved …", then "Move to…" and Remove. The bulk-select checkbox shows
//! where the page passes a selection.

use std::collections::BTreeSet;

use eq_core::{i18n::keys, store::bookmarks::Bookmark};
use nsh_base::theme;
use nsh_core::prelude::*;

use super::data::{Shelf, fmt, open_verse, saved_label, surah_name};
use crate::{
    stores::{
        locale::{t, tr},
        notes::notes_store,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        controls::checkbox_row,
        icons::lucide,
        layout::pressable_card,
        popups::{dropdown_menu, menu_entry},
        tokens,
    },
};

/// `bookmark`, stroked.
pub(crate) const BOOKMARK_ICON: &str =
    lucide!(r#"<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>"#);
/// `notebook-pen`-like note mark.
pub(crate) const NOTE_ICON: &str =
    lucide!(r#"<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/>"#);

/// "–286" / "–3:1" for a range; "" for one verse.
pub(crate) fn range_suffix(b: &Bookmark) -> String {
    let Some(end_ayah) = b.end_ayah else {
        return String::new();
    };
    match b.end_surah {
        Some(end_surah) if end_surah != b.surah => format!("–{end_surah}:{end_ayah}"),
        _ if end_ayah == b.ayah => String::new(),
        _ => format!("–{end_ayah}"),
    }
}

/// "verse 255–257".
fn ayah_label(b: &Bookmark) -> String {
    format!(
        "{}{}",
        fmt(keys::READER_BOOKMARKS_AYAH, &[("ayah", b.ayah.into())]),
        range_suffix(b)
    )
}

/// "Al-Baqarah 2:255–257" (screen reader reference).
fn reference(b: &Bookmark) -> String {
    format!(
        "{} {}:{}{}",
        surah_name(b.surah),
        b.surah,
        b.ayah,
        range_suffix(b)
    )
}

/// What a row may do, from its page.
#[derive(Clone, Copy)]
pub(crate) struct RowActions {
    pub shelf: Signal<Shelf>,
    /// Show the folder pill.
    pub show_folder: Signal<bool>,
    /// Offer "Move to…".
    pub movable: bool,
    /// Bulk selection, when the page has one.
    pub selected: Option<Signal<BTreeSet<String>>>,
    pub on_move: fn(String, String),
    pub on_remove: fn(String),
}

/// The checkbox of one row, bound to the page's selection by id.
fn select_box(item: ReadSignal<Bookmark>, selected: Signal<BTreeSet<String>>) -> impl IntoElement {
    let checked = signal(false);
    // Selection -> box.
    effect(move || {
        let id = item.with(|b| b.id.clone());
        let on = selected.with(|s| s.contains(&id));
        if checked.get_untracked() != on {
            checked.set(on);
        }
    });
    // Box -> selection.
    effect(move || {
        let on = checked.get();
        let id = untracked(|| item.with(|b| b.id.clone()));
        let has = selected.with_untracked(|s| s.contains(&id));
        if has != on {
            selected.update(|s| {
                if on {
                    s.insert(id);
                } else {
                    s.remove(&id);
                }
            });
        }
    });
    let label = item.with_untracked(|b| {
        fmt(
            keys::READER_BOOKMARKS_SELECT_LABEL,
            &[("reference", reference(b).into())],
        )
    });
    checkbox_row(label, checked)
}

/// The "Move to…" menu of a row; rebuilt when the folders change.
fn move_menu(item: ReadSignal<Bookmark>, actions: RowActions) -> impl IntoElement {
    dynamic(move || {
        let folders = actions.shelf.with(|s| s.folders.clone());
        let current = untracked(|| item.with(|b| b.folder_id.clone()));
        let entries: Vec<AnyElement> = folders
            .into_iter()
            .map(|f| {
                let name = if f.id == current {
                    format!("✓ {}", f.name)
                } else {
                    f.name.clone()
                };
                let folder_id = f.id.clone();
                menu_entry(SharedString::from(name), move || {
                    let id = item.with_untracked(|b| b.id.clone());
                    (actions.on_move)(id, folder_id.clone());
                })
                .into_element()
            })
            .collect();
        dropdown_menu(t(keys::READER_BOOKMARKS_MOVE_TO), entries)
    })
}

/// One bookmark row.
pub(crate) fn bookmark_row(item: ReadSignal<Bookmark>, actions: RowActions) -> impl IntoElement {
    let name_line = move || {
        item.with(|b| SharedString::from(format!("{} · {}", surah_name(b.surah), ayah_label(b))))
    };
    let folder_name = move || {
        if !actions.show_folder.get() {
            return None;
        }
        let folder_id = item.with(|b| b.folder_id.clone());
        actions
            .shelf
            .with(|s| s.folder(&folder_id).map(|f| f.name.clone()))
    };
    let has_note = move || item.with(|b| notes_store().has_note(b.surah, b.ayah));
    // The web shows the creation day as "Saved …".
    let saved = move || item.with(|b| saved_label(&b.created_at));
    let open = move || {
        item.with_untracked(|b| open_verse(b.surah, b.ayah, b.source_id.as_deref()));
    };

    let link = pressable_card("", open)
        .accessibility_label(move || SharedString::from(item.with(reference)))
        .flex_1()
        .min_w(px(0.0))
        .gap_0p5()
        .p_0()
        .border_none()
        .bg(AdaptiveColor::TRANSPARENT)
        .child(
            text(name_line)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .text_color(move || theme().foreground),
        )
        .child(dynamic(move || {
            folder_name().map(|name| {
                view()
                    .self_start()
                    .px_2()
                    .py_0p5()
                    .rounded(tokens::radius_pill)
                    .border_1()
                    .border_color(tokens::border_strong)
                    .bg(tokens::background_subtle)
                    .child(text(name).text_xs().text_color(tokens::muted_text))
            })
        }))
        .child(when(
            move || has_note() || saved().is_some(),
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(when(has_note, || {
                        svg(nsh_core::element::ImageSrc::svg_markup(NOTE_ICON))
                            .size(px(12.0))
                            .label(tr(keys::READER_BOOKMARKS_NOTE_LABEL))
                            .text_color(tokens::muted_text)
                    }))
                    .child(
                        text(move || SharedString::from(saved().unwrap_or_default()))
                            .text_xs()
                            .text_color(tokens::muted_text),
                    )
            },
        ));

    let remove = button_with(
        t(keys::READER_BOOKMARKS_REMOVE),
        ButtonVariant::Outline,
        ButtonSize::Small,
        false,
        Some(tr(keys::READER_BOOKMARKS_REMOVE_LABEL)),
        move || {
            let id = item.with_untracked(|b| b.id.clone());
            (actions.on_remove)(id);
        },
    );

    view()
        .w_full()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(move || theme().border)
        .child(actions.selected.map(|sel| select_box(item, sel)))
        .child(
            svg(nsh_core::element::ImageSrc::svg_markup(BOOKMARK_ICON))
                .size(px(15.0))
                .flex_none()
                .text_color(move || theme().primary),
        )
        .child(link)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_1p5()
                .child(actions.movable.then(|| move_menu(item, actions)))
                .child(remove),
        )
}
