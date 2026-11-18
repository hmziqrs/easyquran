//! The layout animation demo: reorder in one commit, with
//! `animate_layout` called right before the change, so the frames it moves
//! animate natively.

use std::rc::Rc;

use nsh_base::theme;
use nsh_core::{
    element::{LayoutAnimation, LayoutCurve, LayoutTransition},
    prelude::*,
};

use crate::ui;

use super::{api_line, section, status_line};

/// The tiles: a fixed palette keeps each id's color stable while it moves.
const TILES: [u32; 5] = [1, 2, 3, 4, 5];

/// The reorder layouts to pick from, and their tab labels.
const PRESETS: [LayoutAnimation; 3] = [
    LayoutAnimation::all(LayoutTransition::new(
        LayoutCurve::FittedSpring { damping_ratio: 0.7 },
        420.0,
    )),
    LayoutAnimation::EASE_IN_EASE_OUT,
    LayoutAnimation::LINEAR,
];
const PRESET_LABELS: &[&str] = &["Spring", "Ease in out", "Linear"];

/// The tile color, stable per id.
fn tile_color(id: u32) -> AdaptiveColor {
    match id {
        1 => theme().chart_1,
        2 => theme().chart_2,
        3 => theme().chart_3,
        4 => theme().chart_4,
        _ => theme().chart_5,
    }
}

/// The order as the data holds it, so a test can read what a shuffle did.
fn order_line(items: &[u32]) -> String {
    items
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A tile of the row: its place is whatever the list gives it.
fn tile(id: u32) -> impl IntoElement {
    view()
        .key(id.to_string())
        .size(px(44.0))
        .rounded(px(12.0))
        .items_center()
        .justify_center()
        .bg(tile_color(id))
        .child(
            text(id.to_string())
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().card_foreground),
        )
}

pub(crate) fn layout_reorder() -> impl IntoElement {
    let (items, preset) = (signal(TILES.to_vec()), signal(0_usize));
    let status = signal(String::from("Idle"));
    let reorder = Rc::new(move |reorder: fn(&mut Vec<u32>), what: &'static str| {
        // The animation is set before the change, for the commit it makes.
        animate_layout(PRESETS[preset.get_untracked()]);
        items.update(reorder);
        status.set(format!(
            "{} — order {}",
            what,
            order_line(&items.get_untracked())
        ));
    });
    let press = move |label: &'static str,
                      variant: ui::button::ButtonVariant,
                      f: fn(&mut Vec<u32>),
                      what: &'static str| {
        let reorder = reorder.clone();
        ui::button::button_with(
            label,
            variant,
            ui::button::ButtonSize::Medium,
            false,
            Some(label),
            move || reorder(f, what),
        )
    };
    section(
        "Layout animations",
        "Frames, inserts and removals of the next commit animate",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(ui::tabs::tab_bar(
                move || preset.get(),
                PRESET_LABELS,
                move |at: usize| preset.set(at),
            ))
            .child(each(
                items,
                |id| *id,
                move |id: ReadSignal<u32>, _: ReadSignal<usize>| tile(id.get()),
            ))
            .child(
                view()
                    .flex_row()
                    .gap(px(8.0))
                    .child(press(
                        "Reverse",
                        ui::button::ButtonVariant::Primary,
                        |v| v.reverse(),
                        "Reversed",
                    ))
                    .child(press(
                        "Shuffle",
                        ui::button::ButtonVariant::Secondary,
                        |v| shuffle(v.as_mut_slice()),
                        "Shuffled",
                    )),
            )
            .child(status_line("layout-status", move || status.get()))
            .child(api_line("Set right before the change", "animate_layout")),
    )
}

/// A Fisher-Yates pass with a fixed stride: a deterministic shuffle, so
/// tests and screenshots can chase it.
fn shuffle(items: &mut [u32]) {
    for i in (1..items.len()).rev() {
        items.swap(i, (i * 7 + 3) % (i + 1));
    }
}
