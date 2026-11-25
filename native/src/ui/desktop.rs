//! Desktop window helpers (`nsh/desktop`): the title bar of the app's
//! own, a window's title and mode, and the application menu, so the shell
//! draws its chrome from here. A window's handle is `current_window()`,
//! read while rendering.

use nsh_core::Points;
use nsh_core::desktop::{TitleBarOptions, TitleBarStyle, WindowAttributes, WindowMode};
use nsh_core::element::desktop::{MenuItem, WindowRef, set_application_menu, title_bar};

/// Hide the system title bar: the app fills the window and draws its own
/// bar in the area [`title_bar_inset`] reports. Desktops only.
pub fn use_custom_title_bar(window: &WindowRef) {
    set_title_bar(window, TitleBarStyle::Hidden, None);
}

/// `window`'s title bar: `style`, and the app's bar `height` the
/// window controls center in (`None`: the platform's). Desktops only.
pub fn set_title_bar(window: &WindowRef, style: TitleBarStyle, height: Option<Points>) {
    // Both structs are non-exhaustive: start from their defaults.
    let mut bar = TitleBarOptions::default();
    bar.style = style;
    bar.height = height;
    let mut attributes = WindowAttributes::default();
    attributes.title_bar = Some(bar);
    window.set(attributes);
}

/// Sets `window`'s title (title bar, Dock or taskbar, switcher).
pub fn set_window_title(window: &WindowRef, title: impl Into<String>) {
    let mut attributes = WindowAttributes::default();
    attributes.title = Some(title.into());
    window.set(attributes);
}

/// Maximizes `window`, or restores it when it is maximized.
pub fn toggle_maximize(window: &WindowRef) {
    let maximized = window
        .state()
        .is_some_and(|state| state.mode == WindowMode::Maximized);
    window.set_mode(if maximized {
        WindowMode::Normal
    } else {
        WindowMode::Maximized
    });
}

/// The desktop's title-bar insets, so the app's bar keeps clear of the
/// window controls; zero on platforms without one.
pub fn title_bar_inset() -> nsh_core::Edges<nsh_core::Points> {
    title_bar()
}

/// The application menu, macOS's menu bar and the window's menu bar
/// elsewhere; `None` restores the platform's default.
pub fn set_app_menu(items: Option<Vec<MenuItem>>) {
    set_application_menu(items);
}

/// A menu item that runs `on_select`, labeled `label`.
pub fn menu_action(label: &'static str, on_select: impl Fn() + 'static) -> MenuItem {
    MenuItem::new(label).on_select(on_select)
}
