//! The bottom progress popup (web `DownloadBar.svelte`): one row for the
//! Quran download run and one for the offline pack, "View details" to the
//! downloads screen, and a hide button that holds it back for the current
//! runs only. Not shown over the downloads screen, which shows the same
//! progress in full. Also hosts the pack driver and run counting, so it is
//! mounted once for the app's life.

use eq_core::i18n::keys;
use nsh_base::theme;
use nsh_core::{element::AnyElement, prelude::*};

use super::{
    model::{
        PackStatus, drive_pack, hide_popup, pack_hidden, pack_percent, pack_status,
        popup_script_key, quran_hidden, quran_run_fraction, running, to_percent, track_runs,
    },
    rows::progress_bar,
};
use crate::{
    app::quran_store,
    shell::{nav, routes::Route},
    stores::locale::{t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with, icon_button},
        layout::pulse,
        text::{TextRole, role},
        tokens,
    },
};

#[derive(Clone, PartialEq)]
struct PopupRow {
    label: SharedString,
    detail: Option<SharedString>,
    pct: u32,
}

fn visible_rows() -> Vec<PopupRow> {
    let quran = quran_store();
    if nav::current() == Route::Downloads {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let pack_working = pack_status(quran) == PackStatus::Downloading;
    if pack_working {
        if !pack_hidden() {
            rows.push(PopupRow {
                label: tr(keys::READER_PREPARING_OFFLINE_PACK).into(),
                detail: None,
                pct: pack_percent(quran),
            });
        }
        return rows;
    }
    if let Some((id, progress)) = running(quran)
        && !quran_hidden()
    {
        rows.push(PopupRow {
            label: tr(keys::READER_DOWNLOADING_QURAN).into(),
            detail: Some(tr(popup_script_key(&id)).into()),
            pct: to_percent(quran_run_fraction(&id, progress)),
        });
    }
    rows
}

fn popup_row(row: PopupRow) -> AnyElement {
    let pct = row.pct;
    let label = match &row.detail {
        Some(d) => format!("{} · {d}", row.label),
        None => row.label.to_string(),
    };
    view()
        .w_full()
        .flex_col()
        .gap_1p5()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    pulse()
                        .size(px(6.0))
                        .rounded_full()
                        .bg(move || theme().primary),
                )
                .child(
                    role(SharedString::from(label), TextRole::Caption)
                        .flex_1()
                        .number_of_lines(1),
                )
                .child(
                    role(SharedString::from(format!("{pct}%")), TextRole::Caption)
                        .tabular_nums()
                        .text_color(tokens::muted_text),
                ),
        )
        .child(progress_bar(move || pct))
        .into_element()
}

/// The dismissible progress popup. The shell mounts it once, over the
/// pages, at the bottom of the window.
pub fn download_bar() -> impl IntoElement {
    let quran = quran_store();
    effect(move || track_runs(quran));
    effect(move || drive_pack(quran));
    let rows = computed(visible_rows);
    view()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .items_center()
        .px_3()
        .pb_safe_area_plus(px(12.0))
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        .child(dynamic(move || {
            let rows = rows.get();
            if rows.is_empty() {
                return view().into_element();
            }
            view()
                .w_full()
                .max_w(px(384.0))
                .flex_row()
                .items_start()
                .gap_2()
                .py(px(10.0))
                .pl(px(14.0))
                .pr_2()
                .rounded(tokens::radius_xl)
                .border_1()
                .border_color(move || theme().border)
                .bg(tokens::surface)
                .shadow_md()
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .flex_col()
                        .gap_2()
                        .children(rows.into_iter().map(popup_row))
                        .child(view().flex_row().child(button_with(
                            t(keys::READER_DOWNLOAD_DETAILS),
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            || nav::push(Route::Downloads),
                        ))),
                )
                .child(icon_button("✕", tr(keys::READER_DOWNLOAD_HIDE), hide_popup))
                .into_element()
        }))
}
