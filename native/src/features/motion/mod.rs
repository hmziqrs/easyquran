//! Motion and gestures: motions, transitions, layout animations, springs,
//! swipe rows, drag and drop, pinch and rotate, the scroll-linked header
//! on `diff_clamp`, `pan_activation`, reduced-motion paths. Owned by the
//! motion area. The shell mounts `motion_page`; keep its name and
//! signature.
//!
//! Every section is one demo of a tool from the animation and gesture
//! guides, run natively (no signal is written per frame to animate). The
//! look is `crate::ui`'s; only the gesture surfaces draw their own views.

mod composite;
mod drag_drop;
mod header;
mod reduced;
mod reorder;
mod scrub;
mod springs;
mod swipe;
mod transform;
mod transitions;

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui;
use composite::composites;
use reduced::calm_motion;
use springs::spring_tween;

/// The page: one scroll of sections, each a self-contained demo.
pub fn motion_page() -> impl IntoElement {
    // The page's reduced-motion preference, local until the foundation's
    // typed config lands; the ambient `reduce_motion()` is honored beside it.
    let calm = signal(false);
    scroll()
        .flex_1()
        .content_inset(crate::ui::layout::page_insets(0.0))
        .test_id("motion-page-scroll")
        .bg(move || theme().background)
        // The sides only: UIKit insets the scroll under the header and the
        // content inset clears the tab bar, so a top and bottom safe-area
        // padding would leave the space twice.
        .pl_safe_area()
        .pr_safe_area()
        .child(
            ui::layout::page_body()
                .child(ui::text::page_intro(
                    "Motion and gestures",
                    "Springs, tweens, layout animations and native gestures",
                ))
                .child(calm_motion(calm))
                .child(spring_tween(calm))
                .child(composites())
                .child(transitions::route_flip())
                .child(reorder::layout_reorder())
                .child(swipe::swipe_rows())
                .child(transform::pinch_rotate())
                .child(header::collapsing_header())
                .child(scrub::scrubber())
                .child(drag_drop::drop_reorder()),
        )
}

/// A section: the ui section header over a card around one demo's body.
pub(crate) fn section(
    title: &'static str,
    subtitle: &'static str,
    body: impl IntoElement,
) -> impl IntoElement {
    // The ui section carries the title; the body keeps its own surface.
    ui::layout::section(
        title,
        view()
            .flex_col()
            .gap_2()
            .child(view().flex_col().gap_1().child(ui::text::muted(subtitle)))
            .child(
                view()
                    .flex_col()
                    .gap(px(12.0))
                    .p(px(14.0))
                    .rounded(move || theme().radius)
                    .border_1()
                    .border_color(move || theme().border)
                    .bg(move || theme().card)
                    .child(body),
            ),
    )
}

/// A caption naming the API a demo uses. Stacked, not a flex row: the
/// label's measure in a grow row collapses to its minimum width.
pub(crate) fn api_line(label: &'static str, api: &'static str) -> impl IntoElement {
    view()
        .flex_col()
        .gap_1()
        .child(ui::text::muted(label))
        .child(ui::text::code(api))
}

/// A labeled status line a demo writes its end states to. `test_id` keeps
/// the sections' lines apart for the steps; the ui text is opaque, so the
/// id lands on a wrapper view.
pub(crate) fn status_line(
    test_id: &'static str,
    value: impl IntoReactive<SharedString>,
) -> impl IntoElement {
    view().test_id(test_id).child(ui::text::body(value))
}
