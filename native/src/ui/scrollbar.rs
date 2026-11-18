//! The app's own scroll bar: a rail beside a `scroll` view, its thumb
//! styled here.

use nsh_base::{ScrollbarEntrance, ScrollbarMode, ScrollbarMotion, scrollbar, theme};
use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

/// A vertical scroll bar over the `scroll` view `target` points at; the
/// scroll hides its own, and this sits at its trailing edge.
pub fn scrollbar_for(
    target: nsh_core::element::ElementRef,
    name: &'static str,
) -> impl IntoElement {
    scrollbar()
        .scroll(target)
        .axis(Axis::Vertical)
        .mode(ScrollbarMode::Hover)
        .motion({
            let mut motion = ScrollbarMotion::default();
            motion.enter = 150.0;
            motion.exit = 300.0;
            motion.entrance = ScrollbarEntrance::Fade;
            motion
        })
        .accessibility_label(name)
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .w(px(8.0))
        .thumb_style(|s| {
            s.w_full()
                .mx_1()
                .rounded_full()
                .bg(move || theme().muted_foreground.opacity(0.5))
                .transition_all()
                .duration(150.0)
        })
}
