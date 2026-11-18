//! The pan-activation demo: a scrubber inside a vertical scroll. The claim
//! is a prop, not a callback: past a horizontal active offset the platform
//! hands the touches over and the scroll views around it stand down; past
//! a vertical fail offset first they keep them.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{api_line, section, status_line};

/// The scrubber's playfield.
const HEIGHT: f32 = 150.0;

pub(crate) fn scrubber() -> impl IntoElement {
    // Where the playhead sits: while a finger holds it and after, like a
    // player's, so the end state keeps its scrub visible.
    let at = signal(None::<f32>);
    // A finger holds the playhead now.
    let held = signal(false);
    // Grants survive the release, so the count shows the gate decided.
    let grants = signal(0_u32);
    let pad = view()
        .test_id("scrub-pad")
        .h(px(HEIGHT))
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .items_center()
        .justify_center()
        .overflow_hidden()
        .claim_touches(TouchClaim::on_pan(PanActivation::horizontal(
            px(10.0),
            px(10.0),
        )))
        .on_responder_grant(move |e: &ResponderEvent| {
            batch(|| {
                at.set(Some(e.location.x.0));
                held.set(true);
                grants.update(|n| *n += 1);
            });
        })
        .on_responder_move(move |e: &ResponderEvent| {
            if held.get_untracked() {
                at.set(Some(e.location.x.0));
            }
        })
        .on_responder_release(move |_: &ResponderEvent| held.set(false))
        .on_responder_terminate(move |_: &ResponderEvent| held.set(false))
        // The scrubbed fill, from the left edge to the playhead.
        .child(when(
            move || at.get().is_some(),
            move || {
                view()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(move || px(at.get().unwrap_or(0.0)))
                    .bg(move || theme().primary.opacity(0.2))
            },
        ))
        // The playhead: a line and a thumb, larger while the finger holds it.
        .child(when(
            move || at.get().is_some(),
            move || {
                view()
                    .absolute()
                    .left(move || px(at.get().unwrap_or(0.0) - 2.0))
                    .top_0()
                    .bottom_0()
                    .w(px(4.0))
                    .bg(move || theme().primary)
            },
        ))
        .child(when(
            move || at.get().is_some(),
            move || {
                view()
                    .absolute()
                    .left(move || px(at.get().unwrap_or(0.0) - 9.0))
                    .top_0()
                    .bottom_0()
                    .w(px(18.0))
                    .items_center()
                    .child(
                        view()
                            .size(move || px(if held.get() { 18.0 } else { 12.0 }))
                            .rounded(px(9.0))
                            .bg(move || theme().primary)
                            .border_2()
                            .border_color(move || theme().background),
                    )
            },
        ))
        .child(when(
            move || at.get().is_none(),
            move || {
                text("Drag sideways to scrub")
                    .text_size(px(14.0))
                    .text_color(move || theme().muted_foreground)
            },
        ));
    section(
        "Pan activation",
        "The platform decides as the finger moves; the code only hears the grant",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(pad)
            .child(status_line("scrub-status", move || {
                // One grant, two grants: the count word follows the number.
                let grants = match grants.get() {
                    1 => String::from("1 grant"),
                    n => format!("{n} grants"),
                };
                match (at.get(), held.get()) {
                    (Some(x), true) => format!("{grants} · scrubbing at {}", x.round()),
                    (Some(x), false) => format!("{grants} · scrubbed to {}", x.round()),
                    (None, _) => format!("{grants} · idle"),
                }
            }))
            .child(api_line(
                "A scrubber in a scrolling page",
                "claim_touches(TouchClaim::on_pan(..))",
            )),
    )
}
