//! Downloads screen and progress (PARITY K1, K2, B5). Contract:
//! `pub fn page() -> impl IntoElement`, and [`download_bar`], the
//! dismissible bottom progress popup the shell mounts once.
//!
//! Mirrors `web/src/routes/(application)/downloads/+page.svelte`: the
//! "popup hidden" note, the Qur'an databases (overall progress, one row per
//! script), the translations on the device, the offline pack, the app
//! versions and the storage use. The settings storage section reuses the
//! rows and the pack panel from here.

mod bar;
pub(crate) mod model;
pub(crate) mod rows;

use eq_core::{i18n::keys, quran::types::ArabicSource};
use nsh_core::{element::AnyElement, prelude::*};

pub use bar::download_bar;
use model::{
    PackStatus, arabic_targets, is_arabic, pack_hidden, pack_percent, pack_status, quran_hidden,
    quran_run_fraction, running, show_popup, to_percent, toggle_pack,
};
use rows::{
    caption, database_row, heading, label_value, list_card, progress_bar, row_frame, row_label,
    translation_row, usage_block,
};

use crate::{
    app::quran_store,
    features::settings::{installed_version, latest_version},
    stores::locale::{t, t_args},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{page_body, page_insets},
        text::page_intro,
    },
};

/// The offline pack panel (web `OfflinePack.svelte`): heading, status,
/// the toggle; `with_progress` adds the bar while it downloads.
pub(crate) fn pack_panel(with_progress: bool) -> AnyElement {
    let quran = quran_store();
    let status = move || pack_status(quran);
    let status_key = move || match status() {
        PackStatus::Active => keys::SETTINGS_STORAGE_PACK_STATUS_ON,
        PackStatus::Downloading => keys::SETTINGS_STORAGE_PACK_STATUS_DOWNLOADING,
        PackStatus::Error => keys::SETTINGS_STORAGE_PACK_STATUS_ERROR,
        PackStatus::Off => keys::SETTINGS_STORAGE_PACK_STATUS_OFF,
    };
    let toggle_key = move || match status() {
        PackStatus::Active => keys::SETTINGS_STORAGE_PACK_TOGGLE_ON,
        PackStatus::Error => keys::SETTINGS_STORAGE_PACK_RETRY,
        PackStatus::Downloading => keys::SETTINGS_STORAGE_PACK_BUSY,
        PackStatus::Off => keys::SETTINGS_STORAGE_PACK_TOGGLE_OFF,
    };
    let mut panel = view()
        .w_full()
        .flex_col()
        .gap_1p5()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .child(caption(t(keys::SETTINGS_STORAGE_PACK_HEADING)))
                .child(caption(move || {
                    SharedString::from(crate::stores::locale::tr(status_key()))
                })),
        )
        .child(view().flex_row().child(button_with(
            move || SharedString::from(crate::stores::locale::tr(toggle_key())),
            ButtonVariant::Outline,
            ButtonSize::Medium,
            move || status() == PackStatus::Downloading,
            None,
            move || toggle_pack(quran),
        )));
    if with_progress {
        panel = panel.child(when(
            move || status() == PackStatus::Downloading,
            move || {
                view()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        caption(move || SharedString::from(format!("{}%", pack_percent(quran))))
                            .tabular_nums()
                            .text_right(),
                    )
                    .child(progress_bar(move || pack_percent(quran)))
                    .into_element()
            },
        ));
    }
    panel.into_element()
}

fn overall_row() -> AnyElement {
    let quran = quran_store();
    let total = arabic_targets().len();
    let ready = move || {
        arabic_targets()
            .into_iter()
            .filter(|s| quran.is_installed(s.id()))
            .count()
    };
    let overall = move || match running(quran) {
        Some((id, progress)) if is_arabic(&id) => to_percent(quran_run_fraction(&id, progress)),
        _ if total == 0 => 0,
        _ => ((ready() as f64 / total as f64) * 100.0).round() as u32,
    };
    row_frame()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_4()
                .child(row_label(t_args(keys::DOWNLOADS_OVERALL, move || {
                    vec![("ready", ready().into()), ("total", total.into())]
                })))
                .child(
                    caption(move || SharedString::from(format!("{}%", overall()))).tabular_nums(),
                ),
        )
        .child(progress_bar(overall))
        .into_element()
}

fn translations_card() -> AnyElement {
    let quran = quran_store();
    dynamic(move || {
        let pending = running(quran).map(|(id, _)| id).filter(|id| !is_arabic(id));
        let usage = quran.usage();
        let mut stored: Vec<(String, u64)> = quran
            .installed()
            .into_iter()
            .filter(|id| !is_arabic(id) && Some(id) != pending.as_ref())
            .map(|id| {
                let bytes = model::size_of(&id, &usage);
                (id, bytes)
            })
            .collect();
        stored.sort_by_key(|s| std::cmp::Reverse(s.1));
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(id) = pending {
            rows.push(database_row(id));
        }
        for (id, _) in stored {
            rows.push(translation_row(id, |_| ()));
        }
        if rows.is_empty() {
            rows.push(
                row_frame()
                    .child(caption(t(keys::DOWNLOADS_TRANSLATIONS_EMPTY)))
                    .into_element(),
            );
        }
        list_card(rows).into_element()
    })
    .into_element()
}

fn popup_hidden_note() -> AnyElement {
    let quran = quran_store();
    let hidden = move || {
        let quran_active = running(quran).is_some();
        let pack_working = pack_status(quran) == PackStatus::Downloading;
        (quran_active && quran_hidden()) || (pack_working && pack_hidden())
    };
    when(hidden, move || {
        list_card(vec![
            row_frame()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(caption(t(keys::DOWNLOADS_POPUP_HIDDEN)))
                .child(button_with(
                    t(keys::DOWNLOADS_POPUP_SHOW),
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    false,
                    None,
                    show_popup,
                ))
                .into_element(),
        ])
        .into_element()
    })
    .into_element()
}

fn section(title: eq_core::i18n::Key, note: Option<eq_core::i18n::Key>, body: AnyElement) -> View {
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(heading(t(title)))
                .children(note.map(|n| caption(t(n)))),
        )
        .child(body)
}

/// Debug builds on macOS: `EQ_SHOT_DIR` starts the window capture hook
/// (`ui::specimen::capture`) for visual review.
pub(crate) fn dev_capture() {
    #[cfg(all(target_os = "macos", debug_assertions))]
    if let Ok(dir) = std::env::var("EQ_SHOT_DIR") {
        crate::ui::specimen::capture::start(dir.into());
    }
}

/// The downloads screen.
pub fn page() -> impl IntoElement {
    dev_capture();
    let quran = quran_store();
    quran.refresh();
    let databases: Vec<AnyElement> = std::iter::once(overall_row())
        .chain(
            arabic_targets()
                .into_iter()
                .map(|s: ArabicSource| database_row(s.id().to_owned())),
        )
        .collect();
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(page_intro(
                t(keys::DOWNLOADS_TITLE),
                t(keys::DOWNLOADS_INTRO),
            ))
            .child(popup_hidden_note())
            .child(
                section(
                    keys::DOWNLOADS_QURAN_HEADING,
                    Some(keys::DOWNLOADS_QURAN_NOTE),
                    list_card(databases).into_element(),
                )
                .child(row_label(t(keys::DOWNLOADS_TRANSLATIONS_HEADING)))
                .child(translations_card()),
            )
            .child(section(
                keys::DOWNLOADS_PACK_HEADING,
                Some(keys::SETTINGS_STORAGE_OFFLINE_PACK_NOTE),
                list_card(vec![row_frame().child(pack_panel(true)).into_element()]).into_element(),
            ))
            .child(section(
                keys::DOWNLOADS_APP_HEADING,
                None,
                list_card(vec![
                    label_value(t(keys::SETTINGS_APP_INSTALLED), installed_version()),
                    label_value(t(keys::SETTINGS_APP_LATEST), latest_version),
                ])
                .into_element(),
            ))
            .child(section(
                keys::SETTINGS_SECTION_STORAGE,
                None,
                list_card(vec![row_frame().child(usage_block()).into_element()]).into_element(),
            )),
    )
}
