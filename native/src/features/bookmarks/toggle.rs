//! One-tap bookmarking for the reader (web `lib/bookmarks/feedback.svelte.ts`
//! + `SaveToSheet.svelte`): a tap saves to the last-used folder and says
//! where ("Saved to Hifz · Change"); a second tap removes the verse from
//! every folder with an Undo. "Change" opens the folder picker, where a
//! verse may sit in many folders and a new folder also files it.

use std::collections::HashSet;

use eq_core::{
    i18n::keys,
    store::bookmarks::{self, ToggleResult},
};
use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    data::{Shelf, fmt, offer, shelf, surah_name, write},
    row::BOOKMARK_ICON,
};
use crate::{
    stores::{
        locale::{t, tr},
        sync::Area,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        controls::checkbox_row,
        field::text_field,
        icons::lucide,
        layout::{BadgeVariant, badge_of},
        sheet::sheet_of,
        text::{muted, section_title},
        tokens,
    },
};

/// `bookmark`, filled: the verse is saved.
const BOOKMARK_FILLED: &str = lucide!(
    r#"<path fill="currentColor" d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>"#
);

/// Which verses have a bookmark, read once per bookmark-area version and
/// shared by every toggle on the surface.
#[derive(Default, Clone)]
struct Marks {
    version: Option<u64>,
    loading: Option<u64>,
    keys: HashSet<(i64, i64)>,
}

fn marks() -> Signal<Marks> {
    global::<Marks>()
}

fn request_marks(version: u64) {
    let current =
        marks().with_untracked(|m| m.version == Some(version) || m.loading == Some(version));
    if current {
        return;
    }
    marks().update(|m| m.loading = Some(version));
    crate::features::bookmarks::store().run(
        |store| {
            Ok(bookmarks::bookmarks(store)?
                .into_iter()
                .map(|b| (b.surah, b.ayah))
                .collect::<HashSet<_>>())
        },
        move |out| {
            let Ok(keys) = out else {
                return;
            };
            marks().update(|m| {
                if m.loading == Some(version) {
                    m.loading = None;
                }
                if m.version.is_none_or(|v| v <= version) {
                    m.version = Some(version);
                    m.keys = keys;
                }
            });
        },
    );
}

/// Whether the verse has a bookmark (tracked).
fn is_marked(surah: i64, ayah: i64) -> bool {
    marks().with(|m| m.keys.contains(&(surah, ayah)))
}

/// Tap: save to the last-used folder or remove from every folder, with
/// the feedback offer; `picker` opens on "Change".
fn toggle(surah: i64, ayah: i64, source_id: Option<String>, picker: Signal<bool>) {
    write(
        move |s| bookmarks::toggle(s, surah, ayah, source_id),
        move |result| match result {
            ToggleResult::Added { folder, .. } => offer(
                fmt(
                    keys::READER_BOOKMARKS_SAVED_TO,
                    &[("folder", folder.name.into())],
                ),
                tr(keys::READER_BOOKMARKS_CHANGE),
                move || {
                    let _ = picker.try_set(true);
                },
            ),
            ToggleResult::Removed { bookmarks: removed } if !removed.is_empty() => offer(
                tr(keys::READER_BOOKMARKS_REMOVED),
                tr(keys::READER_BOOKMARKS_UNDO),
                move || {
                    let rows = removed.clone();
                    write(move |s| bookmarks::restore(s, &rows), |_| ());
                },
            ),
            ToggleResult::Removed { .. } => {}
        },
    );
}

/// The verse's bookmark button for the reader: reflects and toggles the
/// verse's bookmark (Arabic reader context). Mount
/// [`super::feedback_bar`] once on the same surface for "Change" / Undo.
pub fn bookmark_toggle(surah: u16, ayah: u16) -> impl IntoElement {
    bookmark_toggle_in(surah, ayah, None)
}

/// [`bookmark_toggle`] on a translation route: `source_id` is saved with
/// the bookmark so it reopens in that translation.
pub fn bookmark_toggle_in(surah: u16, ayah: u16, source_id: Option<String>) -> impl IntoElement {
    let (surah, ayah) = (i64::from(surah), i64::from(ayah));
    let sync = crate::features::bookmarks::store();
    effect(move || {
        let version = sync.version(Area::Bookmarks);
        if sync.ready() {
            untracked(|| request_marks(version));
        }
    });
    let picker = signal(false);
    let source = source_id.clone();
    view()
        .child(dynamic(move || {
            let on = is_marked(surah, ayah);
            let source = source.clone();
            icon_button_for(on, move || toggle(surah, ayah, source.clone(), picker))
        }))
        .child(when(picker, move || {
            save_to_sheet(picker, surah, ayah, source_id.clone())
        }))
}

fn icon_button_for(on: bool, press: impl Fn() + 'static) -> impl IntoElement {
    let (icon, variant) = if on {
        (
            BOOKMARK_FILLED,
            crate::ui::button::IconButtonVariant::Primary,
        )
    } else {
        (BOOKMARK_ICON, crate::ui::button::IconButtonVariant::Ghost)
    };
    crate::ui::button::icon_button_with(
        icon,
        tr(keys::READER_BOOKMARK_VERSE),
        variant,
        crate::ui::button::IconButtonSize::Small,
        press,
    )
}

/// Whether the verse sits in `folder_id` on this shelf.
fn in_folder(shelf: &Shelf, surah: i64, ayah: i64, folder_id: &str) -> bool {
    shelf
        .bookmarks
        .iter()
        .any(|b| b.surah == surah && b.ayah == ayah && b.folder_id == folder_id)
}

/// One folder's checkbox in the picker, two-way with the store.
fn folder_check(
    shelf: Signal<Shelf>,
    folder_id: String,
    label: String,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
) -> impl IntoElement {
    let initial = shelf.with_untracked(|s| in_folder(s, surah, ayah, &folder_id));
    let checked = signal(initial);
    let id = folder_id.clone();
    effect(move || {
        let on = shelf.with(|s| in_folder(s, surah, ayah, &id));
        if checked.get_untracked() != on {
            checked.set(on);
        }
    });
    effect(move || {
        let on = checked.get();
        let stored = shelf.with_untracked(|s| in_folder(s, surah, ayah, &folder_id));
        if !shelf.with_untracked(|s| s.loaded) || stored == on {
            return;
        }
        let (folder_id, source_id) = (folder_id.clone(), source_id.clone());
        write(
            move |s| bookmarks::set_in_folder(s, surah, ayah, &folder_id, on, source_id),
            |_| (),
        );
    });
    checkbox_row(label, checked)
}

/// The "Save to…" picker of one verse.
fn save_to_sheet(
    open: Signal<bool>,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
) -> impl IntoElement {
    sheet_of(open, Placement::Bottom, move || {
        let shelf = shelf();
        let new_name = signal(String::new());
        let source_cell = stored(source_id.clone());
        let add = move || {
            let name = new_name.get_untracked();
            if name.trim().is_empty() {
                return;
            }
            let source = source_cell.get();
            write(
                move |s| {
                    if let Some(folder) = bookmarks::create_folder(s, &name)? {
                        bookmarks::set_in_folder(s, surah, ayah, &folder.id, true, source)?;
                    }
                    Ok(())
                },
                |_| (),
            );
            new_name.set(String::new());
        };
        let reference = format!(
            "{} · {}",
            surah_name(surah),
            fmt(keys::READER_BOOKMARKS_AYAH, &[("ayah", ayah.into())])
        );
        view()
            .flex_col()
            .gap_3()
            .child(section_title(t(keys::READER_BOOKMARKS_SAVE_TO)))
            .child(muted(reference))
            .child(
                scroll()
                    .max_h(px(320.0))
                    .child(view().flex_col().gap_1().child(each(
                        move || shelf.with(|s| s.folders.clone()),
                        |f| f.id.clone(),
                        move |folder, _| {
                            let f = folder.get_untracked();
                            let count = move || {
                                let id = folder.with(|f| f.id.clone());
                                SharedString::from(shelf.with(|s| s.count_in(&id)).to_string())
                            };
                            view()
                                .min_h(px(44.0))
                                .flex_row()
                                .items_center()
                                .gap_3()
                                .px_2()
                                .child(view().flex_1().min_w(px(0.0)).child(folder_check(
                                    shelf,
                                    f.id.clone(),
                                    f.name.clone(),
                                    surah,
                                    ayah,
                                    source_cell.get(),
                                )))
                                .child(f.is_default.then(|| {
                                    badge_of(
                                        t(keys::READER_BOOKMARKS_DEFAULT_BADGE),
                                        BadgeVariant::Secondary,
                                    )
                                }))
                                .child(text(count).text_xs().text_color(tokens::muted_text))
                        },
                    ))),
            )
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        text_field(new_name, tr(keys::READER_BOOKMARKS_NEW_FOLDER))
                            .label(tr(keys::READER_BOOKMARKS_NEW_FOLDER_LABEL))
                            .max_length(eq_core::store::bookmarks::FOLDER_NAME_MAX as u32)
                            .on_submit(move |_: &str| add())
                            .flex_1(),
                    )
                    .child(button_with(
                        t(keys::READER_BOOKMARKS_CREATE_FOLDER),
                        ButtonVariant::Secondary,
                        ButtonSize::Medium,
                        move || new_name.get().trim().is_empty(),
                        None,
                        add,
                    )),
            )
            .text_color(move || theme().popover_foreground)
            .into_element()
    })
}
