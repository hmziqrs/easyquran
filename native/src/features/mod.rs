//! The feature screens, one module per area. Each area's page fns are the
//! shell's mounting contract (`shell::routes::Route::page`): keep their
//! names and signatures, replace their bodies. Every page fn returns
//! `impl IntoElement` and is a plain fn (make a `#[component]` inside when
//! the page needs an owner). Screens import controls from `crate::ui` only.
//!
//! Until an area lands, its pages render [`placeholder`].

pub mod account;
pub mod appeal;
pub mod audio;
pub mod auth;
pub mod azkar;
pub mod bookmarks;
pub mod daily_ayah;
pub mod downloads;
pub mod home;
pub mod index;
pub mod marketing;
pub mod newsletter;
pub mod reader;
pub mod report;
pub mod search;
pub mod settings;
pub mod yours;

use nsh_core::{element::AnyElement, prelude::*};

use crate::ui::{
    layout::{page_body, page_insets},
    text::{muted, page_intro},
};

/// A stub page: its heading (where the platform shows one), "Not built
/// yet", and `detail` (the route's arguments) under it. Removed once the
/// last area lands.
// i18n: F-i18n
pub fn placeholder(title: &'static str, detail: Option<String>) -> AnyElement {
    scroll()
        .flex_1()
        .content_inset(page_insets(0.0))
        .child(
            page_body()
                .child(page_intro(title, "Not built yet"))
                .children(detail.map(|d| muted(d).into_element())),
        )
        .into_element()
}
