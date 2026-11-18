//! The reduced-motion demo: a preference that swaps springs for instant
//! states, honored beside the system's own `reduce_motion()`. Motions
//! follow the setting on their own; everything else checks it here.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui;

use super::{api_line, section, status_line};

/// Where the drawer rests open and shut (its own width).
const DRAWER: f32 = 220.0;

pub(crate) fn calm_motion(calm: Signal<bool>) -> impl IntoElement {
    // The system's setting counts beside the page's own preference.
    let effective = computed(move || calm.get() || reduce_motion());
    let open = signal(false);
    let status = signal(String::from("Idle"));
    let drawer = animated_value(DRAWER);
    // Android lets a natively translated child escape its parent's clip,
    // so the drawer is hidden outright while it rests shut.
    let shown = signal(false);
    let reveal = move || {
        let to = if open.get_untracked() { DRAWER } else { 0.0 };
        open.update(|o| *o = !*o);
        let opening = to == 0.0;
        if opening {
            shown.set(true);
        }
        if effective.get_untracked() {
            drawer.set(to);
            shown.set(opening);
            status.set(format!("Instant {}", to.round()));
            return;
        }
        let end = drawer.spring(to, Spring::default());
        spawn(async move {
            let finished = end.await;
            if finished && !opening {
                shown.set(false);
            }
            status.set(if finished {
                "Sprung · finished".to_owned()
            } else {
                "Sprung · interrupted".to_owned()
            });
        });
    };
    section(
        "Reduced motion",
        "The calm path swaps springs for instant states and keeps every end state",
        view()
            .flex_col()
            .gap(px(12.0))
            // The switch writes the preference; the system setting is read
            // beside it in `effective`.
            .child(ui::controls::switch_row("Calm motion on this page", calm))
            .child(
                view()
                    .h(px(64.0))
                    .rounded(move || theme().radius)
                    .overflow_hidden()
                    .bg(move || theme().muted)
                    .child(
                        view()
                            .absolute()
                            // Anchored at the right edge: shut, its own width
                            // moves it fully past the track's clip.
                            .right_0()
                            .top_0()
                            .bottom_0()
                            .w(px(DRAWER))
                            .items_center()
                            .justify_center()
                            .rounded(move || theme().radius)
                            .bg(move || theme().primary)
                            .bind_translate_x(&drawer)
                            .opacity(move || if shown.get() { 1.0_f32 } else { 0.0 })
                            .child(
                                text("Drawer")
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(move || theme().primary_foreground),
                            ),
                    ),
            )
            .child(
                ui::button::button(
                    move || {
                        if open.get() {
                            "Shut the drawer"
                        } else {
                            "Reveal the drawer"
                        }
                    },
                    reveal,
                )
                .self_start(),
            )
            .child(status_line("calm-status", move || status.get()))
            .child(api_line(
                "Checked beside the page's own preference",
                "reduce_motion()",
            )),
    )
}
