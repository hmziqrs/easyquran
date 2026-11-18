//! View pieces shared by the downloads screen and the settings storage
//! section: the grouped list card, a row, the progress bar, the usage bar
//! (web `UsageBar.svelte`), a database row and the stored-translation row
//! with its inline remove confirmation (web `StorageArtifactRow.svelte`).

use eq_core::i18n::keys;
use nsh_base::theme;
use nsh_core::{element::AnyElement, prelude::*};

use super::model::{
    RowState, artifact_language, artifact_name, format_bytes, is_arabic, row_state, size_of,
};
use crate::{
    app::quran_store,
    stores::locale::{t, t_args, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{BadgeTone, badge},
        text::{TextRole, role},
        tokens,
    },
};

/// A grouped list: rows on the surface, hairlines between them.
pub fn list_card(rows: Vec<AnyElement>) -> View {
    let mut card = view()
        .w_full()
        .flex_col()
        .overflow_hidden()
        .rounded(tokens::radius_xl)
        .border_1()
        .border_color(tokens::border_strong)
        .bg(tokens::surface);
    for (i, row) in rows.into_iter().enumerate() {
        if i > 0 {
            card = card.child(view().w_full().h(px(1.0)).bg(move || theme().border));
        }
        card = card.child(row);
    }
    card
}

/// One row's frame inside [`list_card`].
pub fn row_frame() -> View {
    view().w_full().flex_col().gap_2().px_4().py(px(14.0))
}

/// A label and a value on one row.
pub fn label_value(
    label: impl IntoReactive<SharedString>,
    value: impl IntoReactive<SharedString>,
) -> AnyElement {
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(row_label(label))
        .child(caption(value).tabular_nums())
        .into_element()
}

/// The row's label (14.5, medium).
pub fn row_label(label: impl IntoReactive<SharedString>) -> Text {
    role(label, TextRole::Body).font_weight(FontWeight::MEDIUM)
}

/// A muted caption.
pub fn caption(label: impl IntoReactive<SharedString>) -> Text {
    role(label, TextRole::Caption).text_color(tokens::muted_text)
}

/// A section heading on a page (17, medium).
pub fn heading(label: impl IntoReactive<SharedString>) -> Text {
    role(label, TextRole::H3)
        .text_size(px(17.0))
        .role(nsh_core::element::AccessibilityRole::Header)
}

/// A thin determinate bar, 0-100.
pub fn progress_bar(pct: impl Fn() -> u32 + 'static) -> impl IntoElement {
    view()
        .w_full()
        .h(px(6.0))
        .flex_row()
        .overflow_hidden()
        .rounded(tokens::radius_pill)
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::background_subtle)
        .child(
            view()
                .h_full()
                .rounded(tokens::radius_pill)
                .bg(move || theme().primary)
                .w(move || relative(pct().min(100) as f32 / 100.0)),
        )
}

/// One layer of the usage bar.
#[derive(Clone, Copy, PartialEq)]
enum Layer {
    Arabic,
    Translations,
    Other,
}

impl Layer {
    fn key(self) -> eq_core::i18n::Key {
        match self {
            Layer::Arabic => keys::SETTINGS_STORAGE_LAYER_ARABIC,
            Layer::Translations => keys::SETTINGS_STORAGE_LAYER_TRANSLATIONS,
            Layer::Other => keys::SETTINGS_STORAGE_LAYER_OTHER,
        }
    }

    fn fill(self) -> AdaptiveColor {
        match self {
            Layer::Arabic => theme().primary,
            Layer::Translations => tokens::primary_soft(),
            Layer::Other => tokens::muted_text(),
        }
    }
}

fn layers() -> Vec<(Layer, u64)> {
    let usage = quran_store().usage();
    let (mut arabic, mut translations) = (0, 0);
    for (id, bytes) in &usage.artifacts {
        if is_arabic(id) {
            arabic += bytes;
        } else {
            translations += bytes;
        }
    }
    vec![
        (Layer::Arabic, arabic),
        (Layer::Translations, translations),
        (Layer::Other, usage.partial_bytes),
    ]
}

/// "{used} used" over the stacked bar and its legend.
pub fn usage_block() -> impl IntoElement {
    let quran = quran_store();
    let used = t_args(keys::SETTINGS_STORAGE_USED_OF, move || {
        vec![("used", format_bytes(quran.usage().total_bytes).into())]
    });
    view()
        .w_full()
        .flex_col()
        .gap_2p5()
        .child(
            role(used, TextRole::Body)
                .font_weight(FontWeight::MEDIUM)
                .tabular_nums(),
        )
        .child(dynamic(move || {
            let layers = layers();
            let total: u64 = layers.iter().map(|(_, b)| b).sum();
            if total == 0 {
                return view().into_element();
            }
            let bar = view()
                .w_full()
                .h(px(14.0))
                .flex_row()
                .overflow_hidden()
                .rounded(tokens::radius_sm)
                .bg(tokens::background_subtle)
                .children(layers.iter().filter(|(_, b)| *b > 0).map(|(layer, bytes)| {
                    let layer = *layer;
                    view()
                        .h_full()
                        .flex_grow(*bytes as f32)
                        .bg(move || layer.fill())
                }));
            let legend = view().flex_row().flex_wrap().gap_x_4().gap_y_1().children(
                layers
                    .into_iter()
                    .filter(|(_, b)| *b > 0)
                    .map(move |(layer, bytes)| {
                        let pct = ((bytes as f64 / total as f64) * 100.0).round();
                        view()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(view().size(px(8.0)).rounded_full().bg(move || layer.fill()))
                            .child(caption(move || {
                                SharedString::from(format!(
                                    "{} · {} · {pct}%",
                                    tr(layer.key()),
                                    format_bytes(bytes)
                                ))
                            }))
                    }),
            );
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(bar)
                .child(legend)
                .into_element()
        }))
}

fn state_badge(state: RowState) -> impl IntoElement {
    let tone = match state {
        RowState::Downloading(_) => BadgeTone::Primary,
        RowState::Failed => BadgeTone::Danger,
        RowState::Queued | RowState::Ready | RowState::Missing => BadgeTone::Neutral,
    };
    badge(t(state.key()), tone)
}

fn row_percent(state: RowState, size: u64) -> u32 {
    let RowState::Downloading(p) = state else {
        return 0;
    };
    let total = if p.total > 0 { p.total } else { size };
    if total == 0 {
        return 0;
    }
    (((p.loaded as f64 / total as f64) * 100.0).round() as u32).min(100)
}

/// One database: name, state badge, size or progress, the bar while it
/// downloads, Retry after a failure. Rebuilt when its state moves.
pub fn database_row(id: String) -> AnyElement {
    let quran = quran_store();
    dynamic(move || {
        let id = id.clone();
        let id = id.as_str();
        let state = row_state(quran, id);
        let size = size_of(id, &quran.usage());
        let pct = row_percent(state, size);
        let detail = match state {
            RowState::Downloading(p) => {
                let total = if p.total > 0 { p.total } else { size };
                eq_core::i18n::tr_args(
                    crate::stores::locale::locale(),
                    keys::DOWNLOADS_PROGRESS,
                    &[
                        ("pct", i64::from(pct).into()),
                        ("loaded", format_bytes(p.loaded).into()),
                        ("total", format_bytes(total).into()),
                    ],
                )
            }
            _ => format_bytes(size),
        };
        let mut frame = row_frame().child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_1p5()
                        .child(
                            role(SharedString::from(artifact_name(id)), TextRole::Body)
                                .number_of_lines(1),
                        )
                        .child(state_badge(state)),
                )
                .child(caption(SharedString::from(detail)).tabular_nums()),
        );
        if matches!(state, RowState::Downloading(_)) {
            frame = frame.child(progress_bar(move || pct));
        }
        if state == RowState::Failed {
            frame = frame.child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        caption(t(keys::DOWNLOADS_QURAN_ERROR))
                            .flex_1()
                            .text_color(tokens::danger),
                    )
                    .child(button_with(
                        t(keys::SETTINGS_STORAGE_PACK_RETRY),
                        ButtonVariant::Outline,
                        ButtonSize::Small,
                        false,
                        None,
                        {
                            let id = id.to_owned();
                            move || quran.download(&id)
                        },
                    )),
            );
        }
        frame.into_element()
    })
    .into_element()
}

/// A stored translation: name, language, where it lives, size, and Remove
/// with an inline confirmation. `on_removed` gets the removed name.
pub fn translation_row(id: String, on_removed: impl Fn(String) + Copy + 'static) -> AnyElement {
    let quran = quran_store();
    let confirming = signal(false);
    let name = {
        let id = id.clone();
        move || SharedString::from(artifact_name(&id))
    };
    let size = {
        let id = id.clone();
        move || SharedString::from(format_bytes(size_of(&id, &quran.usage())))
    };
    let language = artifact_language(&id);
    let title_id = id.clone();
    let remove = {
        let id = id.clone();
        move || {
            let name = artifact_name(&id);
            quran.delete(&id);
            confirming.set(false);
            on_removed(name);
        }
    };
    let actions = dynamic(move || {
        if confirming.get() {
            let remove = remove.clone();
            let title_id = title_id.clone();
            let title = t_args(keys::SETTINGS_STORAGE_REMOVE_CONFIRM_TITLE, move || {
                vec![("name", artifact_name(&title_id).into())]
            });
            return view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap_2()
                .child(caption(title))
                .child(button_with(
                    t(keys::SETTINGS_STORAGE_REMOVE_CONFIRM_ACTION),
                    ButtonVariant::Danger,
                    ButtonSize::Small,
                    false,
                    None,
                    remove,
                ))
                .child(button_with(
                    t(keys::SETTINGS_STORAGE_REMOVE_CANCEL),
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    false,
                    None,
                    move || confirming.set(false),
                ))
                .into_element();
        }
        button_with(
            t(keys::SETTINGS_STORAGE_REMOVE),
            ButtonVariant::Outline,
            ButtonSize::Small,
            false,
            None,
            move || confirming.set(true),
        )
        .into_element()
    });
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_4()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_0p5()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_1p5()
                        .child(role(name, TextRole::Body).number_of_lines(1))
                        .children(
                            language.map(|l| badge(SharedString::from(l), BadgeTone::Neutral)),
                        )
                        .child(badge(
                            t(keys::SETTINGS_STORAGE_STORE_OPFS),
                            BadgeTone::Neutral,
                        )),
                )
                .child(caption(size).tabular_nums()),
        )
        .child(actions)
        .into_element()
}
