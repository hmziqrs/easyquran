//! Storage (PARITY J4, K1; web `StorageSection.svelte`): the link to the
//! downloads screen, usage, the Arabic text the reader needs, the stored
//! translations with Remove / Remove all, and the offline pack.
//!
//! Browser-only rows have no native counterpart and are left out:
//! the quota, persistent storage, the IndexedDB note and "Clear cached
//! pages" (native caches no pages).

use eq_core::i18n::keys;
use nsh_core::{element::AnyElement, prelude::*};

use super::section_frame;
use crate::{
    app::quran_store,
    features::downloads::{
        model::{format_bytes, is_arabic, size_of},
        pack_panel,
        rows::{caption, list_card, row_frame, row_label, translation_row, usage_block},
    },
    shell::{
        nav,
        routes::{Route, SettingsSection},
    },
    stores::locale::{locale, t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{BadgeTone, badge},
        text::{TextRole, role},
    },
};

fn fmt(key: eq_core::i18n::Key, args: &[(&str, eq_core::i18n::Arg<'_>)]) -> String {
    eq_core::i18n::tr_args(locale(), key, args)
}

fn required_rows() -> Vec<AnyElement> {
    let quran = quran_store();
    let usage = quran.usage();
    let mut rows = vec![
        row_frame()
            .child(row_label(t(keys::SETTINGS_STORAGE_REQUIRED_GROUP)))
            .child(caption(t(keys::SETTINGS_STORAGE_REQUIRED_NOTE)))
            .into_element(),
    ];
    for id in quran.installed().iter().filter(|id| is_arabic(id)) {
        let bytes = size_of(id, &usage);
        let name = crate::features::downloads::model::artifact_name(id);
        rows.push(
            row_frame()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_1p5()
                        .child(role(SharedString::from(name), TextRole::Body))
                        .child(badge(
                            t(keys::SETTINGS_STORAGE_STORE_OPFS),
                            BadgeTone::Neutral,
                        )),
                )
                .child(caption(SharedString::from(format_bytes(bytes))).tabular_nums())
                .into_element(),
        );
    }
    rows
}

fn downloads_rows(notice: Signal<Option<String>>, confirming_all: Signal<bool>) -> Vec<AnyElement> {
    let quran = quran_store();
    let usage = quran.usage();
    let mut stored: Vec<(String, u64)> = quran
        .installed()
        .into_iter()
        .filter(|id| !is_arabic(id))
        .map(|id| {
            let bytes = size_of(&id, &usage);
            (id, bytes)
        })
        .collect();
    stored.sort_by_key(|s| std::cmp::Reverse(s.1));
    let mut rows = vec![
        row_frame()
            .child(row_label(t(keys::SETTINGS_STORAGE_DOWNLOADS_HEADING)))
            .into_element(),
    ];
    let has_any = !stored.is_empty();
    if has_any {
        let removed = move |name: String| {
            notice.set(Some(fmt(
                keys::SETTINGS_STORAGE_REMOVED,
                &[("name", name.into())],
            )));
        };
        for (id, _) in stored.iter().cloned() {
            rows.push(translation_row(id, removed));
        }
    } else {
        rows.push(
            row_frame()
                .child(caption(t(keys::SETTINGS_STORAGE_EMPTY)))
                .into_element(),
        );
    }
    let total: u64 = stored.iter().map(|(_, b)| b).sum();
    let ids: Vec<String> = stored.into_iter().map(|(id, _)| id).collect();
    let footer = dynamic(move || {
        if confirming_all.get() {
            let ids = ids.clone();
            return view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(caption(t(keys::SETTINGS_STORAGE_REMOVE_ALL_CONFIRM)))
                .child(button_with(
                    t(keys::SETTINGS_STORAGE_REMOVE_CONFIRM_ACTION),
                    ButtonVariant::Danger,
                    ButtonSize::Small,
                    false,
                    None,
                    move || {
                        for id in &ids {
                            quran.delete(id);
                        }
                        confirming_all.set(false);
                        let message = if total > 0 {
                            fmt(
                                keys::SETTINGS_STORAGE_FREED,
                                &[("size", format_bytes(total).into())],
                            )
                        } else {
                            tr(keys::SETTINGS_STORAGE_REMOVED_ALL).to_owned()
                        };
                        notice.set(Some(message));
                    },
                ))
                .child(button_with(
                    t(keys::SETTINGS_STORAGE_REMOVE_CANCEL),
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    false,
                    None,
                    move || confirming_all.set(false),
                ))
                .into_element();
        }
        let mut bar = view().flex_row().flex_wrap().items_center().gap_3();
        if has_any {
            bar = bar.child(button_with(
                t(keys::SETTINGS_STORAGE_REMOVE_ALL),
                ButtonVariant::Danger,
                ButtonSize::Small,
                false,
                None,
                move || confirming_all.set(true),
            ));
        }
        if let Some(text) = notice.get() {
            bar = bar.child(caption(SharedString::from(text)));
        }
        bar.into_element()
    });
    rows.push(row_frame().child(footer).into_element());
    rows
}

pub fn section() -> AnyElement {
    let quran = quran_store();
    quran.refresh();
    let notice = signal(None::<String>);
    let confirming_all = signal(false);
    let link = view().flex_row().child(button_with(
        t(keys::SETTINGS_STORAGE_DOWNLOADS_LINK),
        ButtonVariant::Ghost,
        ButtonSize::Small,
        false,
        None,
        || nav::push(Route::Downloads),
    ));
    let card = dynamic(move || {
        let mut rows = required_rows();
        rows.extend(downloads_rows(notice, confirming_all));
        rows.push(
            row_frame()
                .child(pack_panel(false))
                .child(caption(t(keys::SETTINGS_STORAGE_OFFLINE_PACK_NOTE)))
                .into_element(),
        );
        list_card(rows).into_element()
    });
    section_frame(
        SettingsSection::Storage,
        keys::SETTINGS_STORAGE_INTRO,
        view()
            .w_full()
            .flex_col()
            .gap_4()
            .child(link)
            .child(usage_block())
            .child(card)
            .child(caption(t(keys::SETTINGS_STORAGE_RETENTION_NOTE))),
    )
}
