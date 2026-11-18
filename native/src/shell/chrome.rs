//! Desktop chrome: the hidden system title bar, the sidebar's open state
//! and the application menu (`nsh/desktop`'s `Window` and `Menu`). Everything here no-ops off the
//! desktops, so it is installed unconditionally. One owner per keystroke:
//! the shortcuts are keymap bindings (`shell::install`); the menu items
//! show them and carry no `shortcut` of their own.

use nsh_core::{
    desktop::{MenuRole, TitleBarStyle},
    element::desktop::{
        MenuItem, main_window, set_application_menu, title_bar as title_bar_insets,
    },
    prelude::*,
};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use crate::shell::{nav, palette, routes::Route, undo};
use crate::ui::desktop::set_title_bar;

/// The unified toolbar's height, as macOS draws one: the window controls
/// center in it, and the sidebar's top and each page's toolbar match it.
pub const TITLE_BAR: f32 = 52.0;

/// Whether the desktop sidebar shows; the toolbar toggle and `ctrl+mod+s`
/// flip it.
#[derive(Clone, Copy, PartialEq)]
pub struct SidebarOpen(pub bool);

impl Default for SidebarOpen {
    fn default() -> Self {
        Self(true)
    }
}

/// Whether the sidebar shows. Tracked.
pub fn sidebar_open() -> bool {
    global::<SidebarOpen>().get().0
}

/// Shows or hides the sidebar.
pub fn toggle_sidebar() {
    global::<SidebarOpen>().update(|open| open.0 = !open.0);
}

/// The window controls' lane on the toolbar's leading edge, for a toolbar
/// with no sidebar beside it. Tracked: full screen gives the lane back.
pub fn controls_inset() -> nsh_core::Points {
    title_bar_insets().left
}

/// The desktop shell's wiring: the hidden system title bar under the one
/// the shell draws, and the application menu. Undone when the desktop
/// shell goes (a window narrowed to the phone shell).
pub fn install() {
    effect(move || {
        set_title_bar(TitleBarStyle::Hidden, Some(px(TITLE_BAR)));
        cleanup(|| set_title_bar(TitleBarStyle::Default, None));
    });
    effect(move || {
        // Read so the menu (its appearance item's check mark) follows.
        let dark = dark();
        set_application_menu(Some(menus(dark)));
    });
}

/// Closes the window as the platform does (the exit ask's command on the
/// desktops).
pub fn close() {
    main_window().close();
}

/// The application menu: the shell's commands, every route, the window.
fn menus(is_dark: bool) -> Vec<MenuItem> {
    let go = |route: Route| MenuItem::new(route.label()).on_select(move || nav::open(route));
    vec![
        MenuItem::submenu(
            t!("app.name"),
            [
                MenuItem::role(MenuRole::About),
                MenuItem::separator(),
                MenuItem::new(format!("Command Palette  {}", key_label("mod+k")))
                    .on_select(palette::toggle),
                MenuItem::new(format!("Undo  {}", key_label("mod+z"))).on_select(undo::undo),
                MenuItem::new(format!("Redo  {}", key_label("mod+shift+z"))).on_select(undo::redo),
                MenuItem::separator(),
                MenuItem::new("Toggle Appearance")
                    .checked(is_dark)
                    .on_select(crate::shell::toggle_appearance),
                MenuItem::separator(),
                MenuItem::role(MenuRole::Quit),
            ],
        ),
        MenuItem::submenu(
            "View",
            [
                MenuItem::new(format!("Toggle Sidebar  {}", key_label("ctrl+mod+s")))
                    .on_select(toggle_sidebar),
            ],
        ),
        MenuItem::submenu("Go", Route::ALL.iter().copied().map(go).collect::<Vec<_>>()),
        MenuItem::submenu(
            "Window",
            [
                MenuItem::role(MenuRole::Minimize),
                MenuItem::role(MenuRole::Zoom),
                MenuItem::role(MenuRole::ToggleFullScreen),
            ],
        ),
    ]
}
