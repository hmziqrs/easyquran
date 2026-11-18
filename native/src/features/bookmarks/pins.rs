//! The pinned verses drawer (web `_reader/PinsDrawer.svelte`): a trigger
//! with the count (only while something is pinned) and a bottom sheet with
//! the pins in order (move up/down, remove), a "Compare with" selection
//! (Arabic, then each translation the pins carry), the compare cells, and
//! "Save as bookmark folder". Pinned texts show verbatim as stored.

use eq_core::{i18n::keys, store::bookmarks};
use nsh_base::theme;
use nsh_core::prelude::*;

use super::data::{fmt, write};
use crate::{
    stores::{
        locale::{t, tr},
        notes::parse_verse_key,
        pins::{ARABIC_SELECTION, PinnedVerse, compare_cell, compare_selections, pins_store},
    },
    ui::{
        arabic::{ArabicStyle, QuranScript, plain_ayah},
        button::{
            ButtonSize, ButtonVariant, IconButtonSize, IconButtonVariant, button_with,
            icon_button_with,
        },
        icons::lucide,
        popups::{dropdown_menu, menu_entry},
        sheet::sheet_of,
        text::{body, muted, section_title},
        tokens,
    },
};

const PIN_ICON: &str = lucide!(
    r#"<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>"#
);
const UP_ICON: &str = lucide!(r#"<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>"#);
const DOWN_ICON: &str = lucide!(r#"<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>"#);
const X_ICON: &str = lucide!(r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#);

/// The selection's label: Arabic, the catalogue name, a lane's own name,
/// else the id.
fn selection_label(id: &str, pins: &[PinnedVerse]) -> String {
    if id == ARABIC_SELECTION {
        return tr(keys::READER_PINS_ARABIC_OPTION).to_owned();
    }
    if let Some(entry) = eq_core::quran::catalogue::translation(id) {
        return entry.name.clone();
    }
    pins.iter()
        .flat_map(|p| p.lanes.iter())
        .find(|l| l.source_id == id && !l.name.is_empty())
        .map_or_else(|| id.to_owned(), |l| l.name.clone())
}

/// One folder from the pinned set, each verse saved with the translation it
/// was pinned in. The pins stay.
fn save_as_folder(saved: Signal<Option<usize>>) {
    let name = tr(keys::READER_PINS_FOLDER_NAME).to_owned();
    let pins = pins_store().list();
    write(
        move |s| {
            let Some(folder) = bookmarks::create_folder(s, &name)? else {
                return Ok(None);
            };
            let mut count = 0;
            for pin in &pins {
                let Some((surah, ayah)) = parse_verse_key(&pin.key) else {
                    continue;
                };
                bookmarks::set_in_folder(s, surah, ayah, &folder.id, true, pin.source_id.clone())?;
                count += 1;
            }
            Ok(Some(count))
        },
        move |count| {
            if let Some(count) = count {
                let _ = saved.try_set(Some(count));
            }
        },
    );
}

fn pin_row(pin: ReadSignal<PinnedVerse>, index: ReadSignal<usize>) -> impl IntoElement {
    let key = move || pin.with(|p| p.key.clone());
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .child(
            text(move || SharedString::from(key()))
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_sm()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(when(
            move || index.get() > 0,
            move || {
                icon_button_with(
                    UP_ICON,
                    tr(keys::READER_PINS_MOVE_UP),
                    IconButtonVariant::Ghost,
                    IconButtonSize::Small,
                    move || pins_store().move_by(&key(), -1),
                )
            },
        ))
        .child(when(
            move || index.get() + 1 < pins_store().count(),
            move || {
                icon_button_with(
                    DOWN_ICON,
                    tr(keys::READER_PINS_MOVE_DOWN),
                    IconButtonVariant::Ghost,
                    IconButtonSize::Small,
                    move || pins_store().move_by(&key(), 1),
                )
            },
        ))
        .child(icon_button_with(
            X_ICON,
            tr(keys::READER_PINS_REMOVE),
            IconButtonVariant::Ghost,
            IconButtonSize::Small,
            move || pins_store().remove(&key()),
        ))
}

fn compare_card(pin: ReadSignal<PinnedVerse>, selection: Signal<String>) -> impl IntoElement {
    let cell = move || pin.with(|p| compare_cell(p, &selection.get()));
    let arabic_view = move || selection.get() == ARABIC_SELECTION;
    view()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .child(
            text(move || SharedString::from(pin.with(|p| p.key.clone())))
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(tokens::foreground_secondary),
        )
        .child(dynamic(move || {
            let cell = cell();
            match (cell.arabic, arabic_view()) {
                (Some(verse), _) => Some(
                    plain_ayah(verse, None, || ArabicStyle::of(QuranScript::Uthmani))
                        .text_color(tokens::quran_foreground)
                        .into_element(),
                ),
                (None, true) => Some(muted(t(keys::READER_PINS_NO_ARABIC)).into_element()),
                (None, false) => None,
            }
        }))
        .child(dynamic(move || {
            if arabic_view() {
                return None;
            }
            Some(match cell().translation {
                Some(line) => body(line).into_element(),
                None => muted(t(keys::READER_PINS_NO_TEXT)).into_element(),
            })
        }))
}

/// The drawer: its trigger (hidden while nothing is pinned) and sheet.
pub fn pins_drawer() -> impl IntoElement {
    let open = signal(false);
    let selection = signal(ARABIC_SELECTION.to_owned());
    let saved = signal(None::<usize>);
    // A selection whose translation was unpinned falls back to Arabic.
    effect(move || {
        let ids = compare_selections(&pins_store().list());
        if !ids.contains(&selection.get()) {
            selection.set(ARABIC_SELECTION.to_owned());
        }
    });
    view()
        .child(when(
            move || pins_store().count() > 0,
            move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(icon_button_with(
                        PIN_ICON,
                        tr(keys::READER_PINS_TRIGGER),
                        IconButtonVariant::Ghost,
                        IconButtonSize::Small,
                        move || open.set(true),
                    ))
                    .child(
                        text(move || SharedString::from(pins_store().count().to_string()))
                            .text_sm()
                            .label(move || {
                                SharedString::from(fmt(
                                    keys::READER_PINS_TRIGGER_COUNT,
                                    &[("count", pins_store().count().into())],
                                ))
                            }),
                    )
            },
        ))
        .child(when(open, move || {
            sheet_of(open, Placement::Bottom, move || {
                scroll()
                    .flex_1()
                    .child(
                        view()
                            .flex_col()
                            .gap_3()
                            .child(
                                view()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        svg(nsh_core::element::ImageSrc::svg_markup(PIN_ICON))
                                            .size(px(16.0))
                                            .text_color(move || theme().foreground),
                                    )
                                    .child(section_title(t(keys::READER_PINS_TITLE))),
                            )
                            .child(muted(t(keys::READER_PINS_DESCRIPTION)))
                            .child(
                                when(
                                    move || pins_store().count() == 0,
                                    || muted(t(keys::READER_PINS_EMPTY)),
                                )
                                .otherwise(move || pins_body(selection, saved)),
                            ),
                    )
                    .into_element()
            })
        }))
}

fn pins_body(selection: Signal<String>, saved: Signal<Option<usize>>) -> impl IntoElement {
    view()
        .flex_col()
        .gap_3()
        .child(view().flex_col().gap_1().child(each(
            move || pins_store().list(),
            |p| p.key.clone(),
            pin_row,
        )))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    text(t(keys::READER_PINS_COMPARE_WITH))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(dynamic(move || {
                    let pins = pins_store().list();
                    let current = selection_label(&selection.get(), &pins);
                    let entries: Vec<AnyElement> = compare_selections(&pins)
                        .into_iter()
                        .map(|id| {
                            let label = selection_label(&id, &pins);
                            menu_entry(SharedString::from(label), move || selection.set(id.clone()))
                                .into_element()
                        })
                        .collect();
                    dropdown_menu(SharedString::from(current), entries)
                })),
        )
        .child(view().flex_col().gap(px(10.0)).child(each(
            move || pins_store().list(),
            |p| p.key.clone(),
            move |pin, _| compare_card(pin, selection),
        )))
        .child(when(
            move || saved.get().is_some(),
            move || {
                text(move || {
                    SharedString::from(fmt(
                        keys::READER_PINS_SAVED,
                        &[("count", saved.get().unwrap_or(0).into())],
                    ))
                })
                .text_sm()
                .text_color(tokens::foreground_secondary)
            },
        ))
        .child(button_with(
            t(keys::READER_PINS_SAVE_FOLDER),
            ButtonVariant::Secondary,
            ButtonSize::Medium,
            false,
            None,
            move || save_as_folder(saved),
        ))
}
