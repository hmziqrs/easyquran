//! The foundation: errors, ids, time, validation, typed config with schema
//! migrations, the capability registry, the first-run flow, file logging,
//! the diagnostics page and the central error surface. Owned by the
//! foundation area, which also owns `src/state` and the service seams in
//! `src/services` (except `fake_api.rs`, owned by the data area).
//!
//! Everything public here is a cross-area contract: other areas consume
//! these APIs by their doc comments.
//!
//! How the pieces wire together (the shell's one call site per piece):
//!
//! 1. The root mounts [`error_surface`] once, above the rest of the app;
//!    any page raises with [`error::report_error`].
//! 2. The root shows [`first_run::first_run_page`] while
//!    [`first_run::is_done`] is false, and at startup calls
//!    `logging::init_file(dir)` with `logging::default_dir()`'s directory
//!    when it resolves one, so diagnostics' log tail also lands on disk.
//! 3. The shell routes to [`diagnostics_page`], and calls
//!    [`state::tasks::cancel_all`] on the quit lifecycle.
//! 4. Services ([`crate::services`]) consume [`error::AppError`] so one
//!    error type flows app-wide.

pub mod capabilities;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod first_run;
pub mod id;
pub mod logging;
pub mod time;
pub mod validation;

use std::sync::LazyLock;

use nsh_base::{nav_page, nav_stack, theme};
use nsh_core::element::{AccessibilityRole, Screen, SharedString, Signal};
use nsh_core::prelude::*;
use serde_json::Value;

pub use self::diagnostics::{diagnostics_content, diagnostics_page};
pub use self::error::error_surface;

use crate::state::event_bus;

/// The shared placeholder the feature stubs render until their area lands
/// the real page. `pub` so it never reads as dead code while stubs remain.
pub fn page_placeholder(title: &'static str) -> impl IntoElement {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .bg(move || theme().background)
        .child(
            text(title)
                .text_size(px(22.0))
                .text_color(move || theme().muted_foreground),
        )
}

/// The app's own header for platforms without a system header (desktops
/// and the web): the page title at the top. iOS and Android draw theirs
/// from `nav_page().title` instead. Feature pages wrap their content with
/// this while `ui` builds its own; the shell area may move it there.
pub fn page_header(title: &'static str) -> impl IntoElement {
    view()
        .h(px(52.0))
        .flex_row()
        .items_center()
        .px_4()
        .border_b_1()
        .border_color(move || theme().border)
        .bg(move || theme().background)
        .child(
            text(title)
                .role(AccessibilityRole::Header)
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .into_element()
}

/// The app's identity from `nsh.json` (read at compile time): name, id and
/// version, for the diagnostics page and bug reports.
pub fn manifest_info() -> (SharedString, SharedString, SharedString) {
    static INFO: LazyLock<(SharedString, SharedString, SharedString)> = LazyLock::new(|| {
        let json: Value = serde_json::from_str(include_str!("../../nsh.json")).unwrap_or_default();
        let field = |key: &str| {
            json.get(key)
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_owned()
                .into()
        };
        (field("name"), field("id"), field("version"))
    });
    INFO.clone()
}

/// The demo root's routes. `FirstRun` is the bottom of the stack until the
/// flow completes, then it is replaced by the index — a `nav_page` lays
/// out inside a `nav_stack`, so the flow is one of its routes.
#[derive(Clone, Debug, PartialEq)]
enum DemoRoute {
    FirstRun,
    Index,
    Diagnostics,
}

/// The foundation's composition root: the first-run gate, the index with
/// Diagnostics on a stack, and the central error surface over everything.
/// It is the wiring example the shell's root follows, and the host the
/// foundation's headless tests and captures mount until the shell lands.
#[component]
pub fn DemoRoot() -> impl IntoElement {
    let routes = signal(vec![if first_run::is_done() {
        DemoRoute::Index
    } else {
        DemoRoute::FirstRun
    }]);
    // The root owns the app-wide subscription: config changes leave a
    // trace in the log, named by this engine's saved config, so parallel
    // engines sharing the process-global ring never cross-match a line.
    event_bus::subscribe(event_bus::TOPIC_CONFIG_SAVED, |event| {
        let name = config::load().display_name;
        let who = if name.is_empty() {
            "(unnamed)"
        } else {
            name.as_str()
        };
        logging::log_info(&format!("config saved ({}) for {who}", event.payload));
    });
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(demo_stack(routes))
        .child(error_surface())
}

/// The demo root's stack: first-run or the index at the bottom, Diagnostics
/// pushed over it, popped by the system's back through `on_dismiss`.
fn demo_stack(routes: Signal<Vec<DemoRoute>>) -> impl IntoElement {
    nav_stack().flex_1().child(each(
        move || routes.get(),
        |route: &DemoRoute| format!("{route:?}"),
        move |route, depth| match route.get_untracked() {
            DemoRoute::FirstRun => first_run::first_run_page(move || {
                // The flow is done: it leaves, the index replaces it.
                routes.set(vec![DemoRoute::Index]);
            }),
            DemoRoute::Index => demo_index(routes),
            DemoRoute::Diagnostics => diagnostics_page()
                .on_dismiss(move || routes.update(|r| r.truncate(depth.get_untracked()))),
        },
    ))
}

/// The demo root's index: the page the shell's real index replaces.
fn demo_index(routes: Signal<Vec<DemoRoute>>) -> Screen {
    nav_page()
        .title("Foundation")
        .header(page_header("Foundation"))
        .child(
            view()
                .flex_1()
                .p_safe_area()
                .p_4()
                .gap_3()
                .child(
                    text("Foundation and state")
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || theme().foreground),
                )
                .child(
                    view()
                        .role(AccessibilityRole::Button)
                        .label("Open Diagnostics")
                        .test_id("open-diagnostics")
                        .w_full()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(move || theme().card)
                        .active(|s| s.opacity(0.85))
                        .on_press(move || {
                            routes.update(|r| {
                                if !r.contains(&DemoRoute::Diagnostics) {
                                    r.push(DemoRoute::Diagnostics);
                                }
                            })
                        })
                        .child(
                            view().flex_1().gap_1().child(
                                text("Diagnostics")
                                    .text_base()
                                    .text_color(move || theme().foreground),
                            ),
                        ),
                ),
        )
}
