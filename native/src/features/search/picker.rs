//! The translation picker (`TranslationPicker.svelte`, `TranslationRow.svelte`):
//! the catalogue filtered by language, translator, id or name and grouped
//! by language; each row toggles the search selection, shows whether its
//! database is on the device and downloads it. Right-to-left translations
//! cannot be searched yet and say so.

use eq_core::{
    i18n::keys,
    quran::catalogue::{self, Direction, TranslationEntry},
};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    app::quran_store,
    shell::{nav, routes::Route, routes::SettingsSection},
    stores::{
        locale::{t, tr},
        quran::DownloadStatus,
        search::SearchStore,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::text_field,
        layout::pressable_card,
        text::muted,
        tokens,
    },
};

fn matches(entry: &TranslationEntry, q: &str) -> bool {
    q.is_empty()
        || entry.language.to_lowercase().contains(q)
        || entry
            .translator
            .as_deref()
            .is_some_and(|t| t.to_lowercase().contains(q))
        || entry.id.to_lowercase().contains(q)
        || entry.name.to_lowercase().contains(q)
}

fn display_name(entry: &TranslationEntry) -> String {
    entry
        .translator
        .clone()
        .unwrap_or_else(|| entry.name.clone())
}

fn status(entry: &TranslationEntry) -> AnyElement {
    let id = entry.id.clone();
    if quran_store().is_installed(&id) {
        return muted(t(keys::SEARCH_CACHED)).into_element();
    }
    if matches!(
        quran_store().downloads().get(&id),
        Some(DownloadStatus::Running(_))
    ) {
        return muted(t(keys::SEARCH_DOWNLOADING)).into_element();
    }
    button_with(
        t(keys::SEARCH_DOWNLOAD),
        ButtonVariant::Outline,
        ButtonSize::Small,
        false,
        None,
        move || quran_store().download(&id),
    )
    .into_element()
}

fn row(entry: &TranslationEntry, selected: bool) -> AnyElement {
    let rtl = entry.direction == Direction::Rtl;
    let id = entry.id.clone();
    let name = display_name(entry);
    let mark = if selected { "✓" } else { "" };
    let card = pressable_card(name.clone(), move || {
        if !rtl {
            SearchStore::get().toggle(&id);
        }
    })
    .flex_row()
    .items_center()
    .gap_3()
    .p_3()
    .selected(selected)
    .style_when(move || rtl, |s| s.opacity(0.6))
    .child(
        view()
            .size_5()
            .items_center()
            .justify_center()
            .rounded(tokens::radius_sm)
            .border_1()
            .border_color(move || theme().border)
            .bg(move || {
                if selected {
                    theme().primary
                } else {
                    tokens::surface()
                }
            })
            .child(
                text(mark)
                    .text_xs()
                    .text_color(move || theme().primary_foreground),
            ),
    )
    .child(
        view()
            .flex_1()
            .flex_col()
            .gap_0p5()
            .child(
                text(name)
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(move || theme().foreground),
            )
            .child(
                text(format!("{} · {}", entry.name, entry.id))
                    .text_xs()
                    .text_color(tokens::muted_text),
            )
            .child(rtl.then(|| muted(t(keys::SEARCH_RTL_DISABLED)))),
    )
    .child(status(entry));
    card.into_element()
}

/// The picker panel.
pub(super) fn picker() -> AnyElement {
    let filter = signal(String::new());
    view()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::surface)
        .child(
            text(t(keys::SEARCH_FILTER_TITLE))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(text_field(filter, tr(keys::SEARCH_FILTER_PLACEHOLDER)))
        .child(dynamic(move || {
            let q = filter.get().trim().to_lowercase();
            let selection = SearchStore::get().selection();
            let mut groups: Vec<(String, Vec<&'static TranslationEntry>)> = Vec::new();
            for entry in catalogue::translations().iter().filter(|e| matches(e, &q)) {
                match groups.iter_mut().find(|(l, _)| *l == entry.language) {
                    Some((_, list)) => list.push(entry),
                    None => groups.push((entry.language.clone(), vec![entry])),
                }
            }
            if groups.is_empty() {
                return vec![muted(t(keys::SEARCH_FILTER_NONE)).into_element()];
            }
            groups
                .into_iter()
                .map(|(language, entries)| {
                    view()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            text(language)
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tokens::muted_text),
                        )
                        .children(
                            entries
                                .into_iter()
                                .map(|e| row(e, selection.contains(&e.id))),
                        )
                        .into_element()
                })
                .collect::<Vec<_>>()
        }))
        .child(
            button_with(
                t(keys::SEARCH_MANAGE_STORAGE),
                ButtonVariant::Ghost,
                ButtonSize::Small,
                false,
                None,
                || nav::open(Route::Settings(SettingsSection::Storage)),
            )
            .self_start(),
        )
        .into_element()
}
