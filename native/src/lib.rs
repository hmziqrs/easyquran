//! The app's Rust UI: its root component, which `nsh run`, `nsh package`
//! and `cargo run` mount. Owned by the shell area.

use nsh_base::theme;
use nsh_core::prelude::*;

/// The product name (`SITE.name` in `web/src/lib/config/site.ts`): a
/// brand, never translated.
pub const APP_NAME: &str = "EasyQuran";

pub mod app;
pub mod features;
pub mod foundation;
pub mod platform;
pub mod services;
pub mod shell;
pub mod state;
pub mod stores;
pub mod ui;

/// The app's UI set (`nsh.json` `"rust"`): what a TSX entry imports from
/// `easyquran/native`. The shell area keeps this block current.
#[native_module("easyquran/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
    // The crate's shipped files under assets/ (fonts; provisioned Quran
    // DBs under assets/quran/), so nsh run serves them and nsh package
    // copies them.
    registry.assets(nsh_core::element::assets!());
}

/// The iOS views the crate draws itself: UIKit's soft scroll edges under
/// the transparent header (`shell::soft_edges`).
#[native_components(ios)]
fn ios_components(components: &nsh_platform_ios::Components) {
    shell::soft_edges::register(components);
    ui::native::register_ios(components);
    ui::native_kit::register_ios(components);
}

/// The system's own controls on macOS (`ui::native`).
#[native_components(macos)]
fn macos_components(components: &nsh_platform_macos::Components) {
    ui::native::register_macos(components);
    ui::native_kit::register_macos(components);
    shell::command_bar::register_macos(components);
}

/// The application's root: the surface's background and text color, the
/// shell's assembly inside it. The safe area is left to the shell's pages
/// and bars, which know where their headers and tab bars sit.
#[component]
pub fn App() -> impl IntoElement {
    // The UI locale before anything reads it; the root lays out in its
    // direction (NSH's own surface direction follows the system language).
    stores::locale::install();
    view()
        .flex_1()
        .direction(stores::locale::layout_direction)
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(app::root())
}
