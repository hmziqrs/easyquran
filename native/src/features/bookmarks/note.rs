//! The verse note for the reader (PARITY F2): a button that shows whether
//! the verse has a note and opens the note sheet. Notes stay on this
//! device and save as you type; clearing the text removes the note.

use eq_core::i18n::keys;
use nsh_base::theme;
use nsh_core::prelude::*;

use super::{
    data::{fmt, surah_name},
    row::NOTE_ICON,
};
use crate::{
    app::auth_store,
    stores::{
        locale::{t, tr},
        notes::notes_store,
    },
    ui::{
        button::{IconButtonSize, IconButtonVariant, icon_button_with},
        field::multiline_field,
        sheet::sheet_of,
        text::{muted, section_title},
        tokens,
    },
};

/// Keeps the notes owner on the signed-in account (`startNotesOwnerSync`):
/// call while building any surface that reads or writes notes.
pub fn follow_notes_owner() {
    let auth = auth_store();
    effect(move || {
        let phase = auth.phase();
        untracked(|| notes_store().sync_owner(&phase));
    });
}

/// The verse's note button (Arabic reader context): primary while the
/// verse has a note; opens [`note_sheet`].
pub fn note_editor(surah: u16, ayah: u16) -> impl IntoElement {
    note_editor_in(surah, ayah, None)
}

/// [`note_editor`] on a translation route: the note records `source_id`.
pub fn note_editor_in(surah: u16, ayah: u16, source_id: Option<String>) -> impl IntoElement {
    follow_notes_owner();
    let open = signal(false);
    let (s, a) = (i64::from(surah), i64::from(ayah));
    view()
        .child(dynamic(move || {
            let variant = if notes_store().has_note(s, a) {
                IconButtonVariant::Primary
            } else {
                IconButtonVariant::Ghost
            };
            icon_button_with(
                NOTE_ICON,
                tr(keys::READER_YOUR_NOTE),
                variant,
                IconButtonSize::Small,
                move || open.set(true),
            )
        }))
        .child(note_sheet(open, surah, ayah, source_id))
}

/// The note sheet of one verse, shown while `open`: the text saves on this
/// device as it changes.
pub fn note_sheet(
    open: Signal<bool>,
    surah: u16,
    ayah: u16,
    source_id: Option<String>,
) -> impl IntoElement {
    let (s, a) = (i64::from(surah), i64::from(ayah));
    let source = stored(source_id);
    when(open, move || {
        sheet_of(open, Placement::Bottom, move || {
            let draft = signal(notes_store().note(s, a));
            effect(move || {
                let text = draft.get();
                if untracked(|| notes_store().note(s, a)) != text {
                    notes_store().set_note(s, a, &text, source.get());
                }
            });
            let reference = format!(
                "{} · {}",
                surah_name(s),
                fmt(keys::READER_BOOKMARKS_AYAH, &[("ayah", a.into())])
            );
            view()
                .flex_col()
                .gap_3()
                .child(section_title(t(keys::READER_YOUR_NOTE)))
                .child(muted(reference))
                .child(
                    multiline_field(draft, tr(keys::READER_NOTE_PLACEHOLDER))
                        .label(tr(keys::READER_YOUR_NOTE))
                        .min_h(px(120.0)),
                )
                .child(
                    text(t(keys::READER_NOTE_SAVED))
                        .text_xs()
                        .text_color(tokens::muted_text),
                )
                .text_color(move || theme().popover_foreground)
                .into_element()
        })
    })
}
