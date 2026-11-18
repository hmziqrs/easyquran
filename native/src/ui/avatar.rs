//! The avatar: a person's picture with initials as the fallback, and the
//! group that stacks them.

use nsh_base::{avatar, avatar_group, theme};
use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

/// A round avatar for `src`, falling back to the name's initials while it
/// loads, when it fails, or without one.
pub fn avatar_image(
    src: impl Fn() -> Option<String> + 'static,
    name: impl Into<SharedString>,
    size: f32,
) -> impl IntoElement {
    avatar()
        .size(px(size))
        .src(move || src().map(ImageSrc::from))
        .name(name.into())
        .image_style(|s| {
            s.size_full()
                .rounded_full()
                .bg(move || theme().muted)
                .object_fit(ResizeMode::Cover)
        })
        .fallback_style(|s| {
            s.size_full()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(move || theme().muted)
                .text_color(move || theme().muted_foreground)
        })
}

/// A group of overlapping avatars, showing `+n` past `limit`.
pub fn avatar_group_of(
    avatars: impl IntoIterator<Item = impl IntoElement>,
    limit: u32,
) -> impl IntoElement {
    avatar_group()
        .limit(limit)
        .flex_row()
        .items_center()
        .children(avatars)
        .overflow_style(|s| {
            s.size(px(32.0))
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(move || theme().muted)
                .text_color(move || theme().muted_foreground)
        })
}
