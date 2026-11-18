//! The status bar at the bottom of the desktop shell: the app, the route
//! on top and the lifecycle phase. Phones leave the room to the tab bar.

use nsh_base::theme;
use nsh_core::prelude::*;

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use crate::shell::{lifecycle, nav};

pub fn status_bar() -> impl IntoElement {
    // macOS: the window's title bar material behind the row, as AppKit's
    // own bars; elsewhere the card fill.
    let material = crate::ui::native_kit::native_material("titlebar");
    let native = material.is_some();
    view()
        .children(material)
        .flex_row()
        .items_center()
        .justify_between()
        .px_3()
        .py_1()
        // A plus, not a bare inset: where the inset is 0 (desktops, the
        // web) a bare one replaces the row's own 4-point bottom pad and
        // the label sat cropped at the viewport edge.
        .pb_safe_area_plus(px(4.0))
        .border_t_1()
        .border_color(move || theme().border)
        .when(!native, |bar| bar.bg(move || theme().card))
        .child(
            text(t!("app.name").to_string())
                .text_xs()
                .text_color(move || theme().muted_foreground),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    text(move || nav::current().label())
                        .text_xs()
                        .number_of_lines(1)
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    text(move || lifecycle::phase().get().name())
                        .text_xs()
                        .text_color(move || theme().muted_foreground),
                ),
        )
}
