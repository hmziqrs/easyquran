//! The notes library (web `NotesLibrary.svelte` + `lib/home/notes-library.ts`,
//! PARITY F2): search (note text, `S:A`, surah name), sort (verse order,
//! recently / least recently edited), open, edit, delete with Undo. Notes
//! stay on this device. Also the Overview's notes preview.

use eq_core::i18n::keys;
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    features::bookmarks::{fmt, medium_date, offer, open_verse, source_label, surah_name},
    stores::{
        locale::{t, tr},
        notes::{NoteEntry, notes_store},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{multiline_field, text_field},
        layout::pressable_card,
        select::{choice, select_field},
        text::muted,
        tokens,
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoteSort {
    Verse,
    Newest,
    Oldest,
}

fn note_sort(value: Option<SharedString>) -> NoteSort {
    match value.as_deref() {
        Some("newest") => NoteSort::Newest,
        Some("oldest") => NoteSort::Oldest,
        _ => NoteSort::Verse,
    }
}

/// `filterNotes`: case-insensitive match on text, `S:A`, or surah name.
fn filter_notes(entries: Vec<NoteEntry>, query: &str) -> Vec<NoteEntry> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return entries;
    }
    entries
        .into_iter()
        .filter(|e| {
            e.text.to_lowercase().contains(&needle)
                || format!("{}:{}", e.surah, e.ayah).contains(&needle)
                || surah_name(e.surah).to_lowercase().contains(&needle)
        })
        .collect()
}

/// `sortNotes`: mushaf order, or by last edit (undated count as oldest;
/// the sort is stable, so ties keep mushaf order).
fn sort_notes(mut entries: Vec<NoteEntry>, sort: NoteSort) -> Vec<NoteEntry> {
    entries.sort_by(|a, b| (a.surah, a.ayah).cmp(&(b.surah, b.ayah)));
    match sort {
        NoteSort::Verse => {}
        NoteSort::Newest => entries.sort_by_key(|e| std::cmp::Reverse(e.updated_at.unwrap_or(0))),
        NoteSort::Oldest => entries.sort_by_key(|e| e.updated_at.unwrap_or(0)),
    }
    entries
}

fn heading(e: &NoteEntry) -> String {
    format!(
        "{} · {}",
        surah_name(e.surah),
        fmt(keys::READER_BOOKMARKS_AYAH, &[("ayah", e.ayah.into())])
    )
}

/// Optimistic delete with one Undo (restores text and timestamp).
fn remove_note(entry: &NoteEntry) {
    let (surah, ayah) = (entry.surah, entry.ayah);
    let Some(snapshot) = notes_store().remove_note(surah, ayah) else {
        return;
    };
    offer(
        tr(keys::READER_NOTES_REMOVED),
        tr(keys::READER_BOOKMARKS_UNDO),
        move || notes_store().restore_note(surah, ayah, snapshot.clone()),
    );
}

fn note_row(
    item: ReadSignal<NoteEntry>,
    editing: Signal<Option<String>>,
    draft: Signal<String>,
) -> impl IntoElement {
    let is_editing = move || editing.get().is_some_and(|k| item.with(|e| e.key == k));
    let meta_line = move || {
        item.with(|e| {
            let source = source_label(e.source_id.as_deref());
            let edited = e.updated_at.map(|ms| {
                fmt(
                    keys::READER_NOTES_EDITED,
                    &[("date", medium_date(ms).into())],
                )
            });
            SharedString::from(match edited {
                Some(edited) => format!("{source} · {edited}"),
                None => source,
            })
        })
    };
    let save = move || {
        let e = item.get_untracked();
        let text = draft.get_untracked();
        if text.trim().is_empty() {
            remove_note(&e);
        } else {
            notes_store().set_note(e.surah, e.ayah, &text, e.source_id.clone());
        }
        editing.set(None);
    };
    view()
        .w_full()
        .flex_row()
        .items_start()
        .gap_3()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(move || theme().border)
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_col()
                .gap_1()
                .child(
                    pressable_card("", move || {
                        item.with_untracked(|e| {
                            open_verse(e.surah, e.ayah, e.source_id.as_deref())
                        });
                    })
                    .accessibility_label(move || SharedString::from(item.with(heading)))
                    .gap_0p5()
                    .p_0()
                    .border_none()
                    .bg(AdaptiveColor::TRANSPARENT)
                    .child(
                        text(move || SharedString::from(item.with(heading)))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .text_color(move || theme().foreground),
                    )
                    .child(text(meta_line).text_xs().text_color(tokens::muted_text)),
                )
                .child(
                    when(is_editing, move || {
                        view()
                            .flex_col()
                            .gap_2()
                            .child(
                                multiline_field(draft, tr(keys::READER_NOTE_PLACEHOLDER))
                                    .label(tr(keys::READER_YOUR_NOTE))
                                    .min_h(px(88.0)),
                            )
                            .child(
                                view()
                                    .flex_row()
                                    .gap_2()
                                    .child(button_with(
                                        t(keys::READER_NOTES_SAVE),
                                        ButtonVariant::Primary,
                                        ButtonSize::Small,
                                        false,
                                        None,
                                        save,
                                    ))
                                    .child(button_with(
                                        t(keys::READER_NOTES_CANCEL),
                                        ButtonVariant::Ghost,
                                        ButtonSize::Small,
                                        false,
                                        None,
                                        move || editing.set(None),
                                    )),
                            )
                    })
                    .otherwise(move || {
                        text(move || SharedString::from(item.with(|e| e.text.clone())))
                            .number_of_lines(3)
                            .text_sm()
                            .text_color(tokens::foreground_secondary)
                    }),
                ),
        )
        .child(when(
            move || !is_editing(),
            move || {
                view()
                    .flex_none()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(button_with(
                        t(keys::READER_NOTES_EDIT),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || {
                            let e = item.get_untracked();
                            draft.set(e.text.clone());
                            editing.set(Some(e.key));
                        },
                    ))
                    .child(button_with(
                        t(keys::READER_NOTES_DELETE),
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        false,
                        None,
                        move || {
                            let e = item.get_untracked();
                            if editing.get_untracked().as_deref() == Some(e.key.as_str()) {
                                editing.set(None);
                            }
                            remove_note(&e);
                        },
                    ))
            },
        ))
}

/// The Notes tab.
pub(crate) fn notes_library() -> impl IntoElement {
    let query = signal(String::new());
    let sort = signal(Some(SharedString::from("verse")));
    let editing = signal(None::<String>);
    let draft = signal(String::new());
    let all_count = move || notes_store().count();
    let visible = move || {
        sort_notes(
            filter_notes(notes_store().entries(), &query.get()),
            note_sort(sort.get()),
        )
    };
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text_field(query, tr(keys::READER_NOTES_SEARCH_PLACEHOLDER))
                        .label(tr(keys::READER_NOTES_SEARCH_LABEL))
                        .flex_1()
                        .min_w(px(160.0)),
                )
                .child(select_field(
                    tr(keys::READER_NOTES_SORT_LABEL),
                    vec![
                        choice("verse", tr(keys::READER_NOTES_SORT_VERSE)),
                        choice("newest", tr(keys::READER_NOTES_SORT_NEWEST)),
                        choice("oldest", tr(keys::READER_NOTES_SORT_OLDEST)),
                    ],
                    sort,
                    tr(keys::READER_NOTES_SORT_VERSE),
                )),
        )
        .child(dynamic(move || {
            if all_count() == 0 {
                return muted(t(keys::READER_WIDGET_NOTES_EMPTY)).into_element();
            }
            if visible().is_empty() {
                return muted(t(keys::READER_NOTES_EMPTY_SEARCH)).into_element();
            }
            view()
                .w_full()
                .flex_col()
                .overflow_hidden()
                .rounded(tokens::radius_md)
                .border_1()
                .border_color(move || theme().border)
                .bg(tokens::surface)
                .child(each(
                    visible,
                    |e| e.key.clone(),
                    move |item, _| note_row(item, editing, draft),
                ))
                .into_element()
        }))
        .child(
            text(t(keys::READER_NOTES_DEVICE))
                .text_xs()
                .text_color(tokens::muted_text),
        )
}

/// The Overview's notes preview: the first five, first line each.
pub(crate) fn notes_preview(on_view_all: impl Fn() + Copy + 'static) -> impl IntoElement {
    let rows = move || {
        notes_store()
            .entries()
            .into_iter()
            .take(5)
            .collect::<Vec<_>>()
    };
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(crate::ui::layout::eyebrow(t(
                    keys::READER_WIDGET_NOTES_TITLE,
                )))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            text(move || SharedString::from(notes_store().count().to_string()))
                                .text_xs()
                                .text_color(tokens::muted_text),
                        )
                        .child(button_with(
                            t(keys::READER_NOTES_VIEW_ALL),
                            ButtonVariant::Outline,
                            ButtonSize::Small,
                            false,
                            None,
                            on_view_all,
                        )),
                ),
        )
        .child(
            when(
                move || notes_store().count() == 0,
                || muted(t(keys::READER_WIDGET_NOTES_EMPTY)),
            )
            .otherwise(move || {
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
                        |e| e.key.clone(),
                        |item, _| {
                            let first_line = move || {
                                item.with(|e| {
                                    e.text.split('\n').next().unwrap_or_default().to_owned()
                                })
                            };
                            pressable_card("", move || {
                                // The web opens notes from the overview in the Arabic reader.
                                item.with_untracked(|e| open_verse(e.surah, e.ayah, None));
                            })
                            .accessibility_label(move || SharedString::from(item.with(heading)))
                            .gap_0p5()
                            .px_4()
                            .py_3()
                            .rounded(px(0.0))
                            .border_none()
                            .bg(AdaptiveColor::TRANSPARENT)
                            .child(
                                text(move || SharedString::from(item.with(heading)))
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .text_color(move || theme().foreground),
                            )
                            .child(
                                text(move || SharedString::from(first_line()))
                                    .text_xs()
                                    .truncate()
                                    .text_color(tokens::foreground_secondary),
                            )
                        },
                    ))
            }),
        )
}
