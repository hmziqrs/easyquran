//! The app's link, for external resources only: a URL, a web page, an
//! email address. In-app navigation is a button or a row, never a link.

use nsh_base::{link, theme};
use nsh_core::prelude::*;

/// An underlined, pointing-hand link to `href`, opened by the platform.
pub fn link_out(
    href: impl Into<SharedString>,
    label: impl IntoReactive<SharedString>,
) -> impl IntoElement {
    link()
        .href(href.into())
        .underline()
        .cursor_pointer()
        .transition_colors()
        .text_color(move || theme().primary)
        .hover(|s| s.opacity(0.8))
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .child(text(label).text_sm().text_color(move || theme().primary))
}
