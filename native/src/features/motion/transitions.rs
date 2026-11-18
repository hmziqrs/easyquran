//! The transitions demo: two local "routes"; leaving plays the exit motion
//! and waits for it, the entering route plays its entrance where it
//! mounts. Motions follow the system's reduced-motion setting on their own.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{api_line, section};

/// One route's titles: its name and what a tap does.
const ROUTES: [(&str, &str); 2] = [
    ("Route A — alpha", "Tap to leave through the left"),
    ("Route B — beta", "Tap to leave through the right"),
];

/// One route's pane. It owns its motion, plays its exit when pressed, and
/// only then flips the route, so the view leaves after its exit finishes.
fn pane(index: usize, route: Signal<usize>) -> impl IntoElement {
    let (title, hint) = ROUTES[index];
    let forward = index == 0;
    let next = 1 - index;
    let m = motion().entering(if forward {
        MotionPreset::FadeInRight
    } else {
        MotionPreset::FadeInLeft
    });
    view()
        .bind_motion(&m)
        .on_press(move |_: &()| {
            let exit = if forward {
                MotionPreset::FadeOutLeft
            } else {
                MotionPreset::FadeOutRight
            };
            spawn(async move {
                m.play(exit).await;
                route.update(|r| *r = next);
            });
        })
        .active(|s| s.opacity(0.85))
        .role(AccessibilityRole::Button)
        .label(title)
        .h(px(120.0))
        .rounded(move || theme().radius)
        .bg(move || theme().secondary)
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .child(
            text(title)
                .text_size(px(17.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().secondary_foreground),
        )
        .child(
            text(hint)
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
}

/// A local two-route flip: the exit is awaited, the entrance plays on
/// mount.
pub(crate) fn route_flip() -> impl IntoElement {
    let route = signal(0_usize);
    section(
        "Transitions",
        "The exit is awaited before the route flips; the entrance plays on mount",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(when(move || route.get() == 0, move || pane(0, route)))
            .child(when(move || route.get() == 1, move || pane(1, route)))
            .child(api_line(
                "Motions on both sides of the flip",
                "motion().entering / play",
            )),
    )
}
