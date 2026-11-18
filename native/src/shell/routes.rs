//! The shell's typed routes: one page each, their keys and labels, and the
//! deep-link paths they answer to. Every page is mounted through
//! [`Route::page`], so a new route is one variant plus its arm here.

use nsh_core::{element::AnyElement, prelude::*};

use crate::shell::home;

/// The routes the shell answers to, `Home` the stack's root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    UiGallery,
    NativeVsApp,
    Forms,
    Data,
    DataDetail(u64),
    QueryDevtools,
    Lists,
    Canvas,
    Motion,
    Overlays,
    Platform,
    Device,
    RemoteScreen,
    Diagnostics,
}

impl Route {
    /// Every root route, in the order Home lists them. Detail pages are
    /// pushed, never destinations.
    pub const ALL: &'static [Route] = &[
        Route::Home,
        Route::UiGallery,
        Route::NativeVsApp,
        Route::Forms,
        Route::Data,
        Route::QueryDevtools,
        Route::Lists,
        Route::Canvas,
        Route::Motion,
        Route::Overlays,
        Route::Platform,
        Route::Device,
        Route::RemoteScreen,
        Route::Diagnostics,
    ];

    /// The stack key: the same route is the same page, so the pages below
    /// keep their state.
    pub fn key(self) -> String {
        match self {
            Route::Home => "home".into(),
            Route::UiGallery => "ui".into(),
            Route::NativeVsApp => "native".into(),
            Route::Forms => "forms".into(),
            Route::Data => "data".into(),
            Route::DataDetail(id) => format!("data-item:{id}"),
            Route::QueryDevtools => "query-devtools".into(),
            Route::Lists => "lists".into(),
            Route::Canvas => "canvas".into(),
            Route::Motion => "motion".into(),
            Route::Overlays => "overlays".into(),
            Route::Platform => "platform".into(),
            Route::Device => "device".into(),
            Route::RemoteScreen => "remote".into(),
            Route::Diagnostics => "diagnostics".into(),
        }
    }

    /// What the sidebar, the tabs, the palette and the tests call it.
    pub fn label(self) -> String {
        match self {
            Route::DataDetail(id) => format!("Data item {id}"),
            route => route.label_str().into(),
        }
    }

    /// The label of a root route, as a literal (static contexts: the
    /// palette's entries, the smoke suite's markers).
    pub fn label_str(self) -> &'static str {
        match self {
            Route::Home => "Home",
            Route::UiGallery => "UI gallery",
            Route::NativeVsApp => "Native vs app",
            Route::Forms => "Forms",
            Route::Data => "Data",
            Route::QueryDevtools => "Query devtools",
            Route::Lists => "Lists",
            Route::Canvas => "Canvas",
            Route::Motion => "Motion",
            Route::Overlays => "Overlays",
            Route::Platform => "Platform",
            Route::Device => "Device",
            Route::RemoteScreen => "Remote screen",
            Route::Diagnostics => "Diagnostics",
            Route::DataDetail(_) => "Data item",
        }
    }

    /// The key of a root route, as a literal (static contexts).
    /// The tab bar's icon: an SF Symbol on iOS, a system drawable on
    /// Android (`nsh package` cannot add app drawables); none elsewhere.
    pub fn tab_icon(self) -> Option<&'static str> {
        use nsh_core::platform::Os;
        let (ios, android) = match self {
            Route::Home => ("house", "android:ic_menu_compass"),
            Route::Forms => ("square.and.pencil", "android:ic_menu_edit"),
            Route::Data => ("tray.full", "android:ic_menu_agenda"),
            Route::Lists => ("list.bullet", "android:ic_menu_sort_by_size"),
            Route::Motion => ("wind", "android:ic_menu_rotate"),
            _ => return None,
        };
        match os() {
            Os::Ios => Some(ios),
            Os::Android => Some(android),
            _ => None,
        }
    }

    /// The route's Lucide icon (ISC), drawn in `currentColor`: the desktop
    /// sidebar's rows, and the themed tab bar Android shows in place of the
    /// system's Material bar.
    pub fn tab_svg(self) -> &'static str {
        use crate::ui::icons::lucide;
        match self {
            Route::Home => lucide!(
                r#"<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#
            ),
            Route::UiGallery => lucide!(
                r#"<rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/>"#
            ),
            Route::NativeVsApp => lucide!(
                r#"<path d="M8 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h3M16 3h3a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-3M12 20v2M12 14v2M12 8v2M12 2v2"/>"#
            ),
            Route::Forms => lucide!(
                r#"<path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z"/>"#
            ),
            Route::Data | Route::DataDetail(_) => lucide!(
                r#"<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11"/>"#
            ),
            Route::QueryDevtools => lucide!(
                r#"<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>"#
            ),
            Route::Lists => {
                lucide!(r#"<path d="M3 5h.01M3 12h.01M3 19h.01M8 5h13M8 12h13M8 19h13"/>"#)
            }
            Route::Canvas => lucide!(
                r#"<path d="M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.25a1.75 1.75 0 0 0-1.4 2.8l.3.4a1.75 1.75 0 0 1-1.4 2.8z"/><circle cx="13.5" cy="6.5" r=".5"/><circle cx="17.5" cy="10.5" r=".5"/><circle cx="6.5" cy="12.5" r=".5"/><circle cx="8.5" cy="7.5" r=".5"/>"#
            ),
            Route::Motion => lucide!(
                r#"<path d="M12.8 19.6A2 2 0 1 0 14 16H2m15.5-8a2.5 2.5 0 1 1 2 4H2m7.8-7.6A2 2 0 1 1 11 8H2"/>"#
            ),
            Route::Overlays => lucide!(
                r#"<path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>"#
            ),
            Route::Platform => {
                lucide!(
                    r#"<rect width="20" height="14" x="2" y="3" rx="2"/><path d="M8 21h8M12 17v4"/>"#
                )
            }
            Route::Device => {
                lucide!(
                    r#"<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>"#
                )
            }
            Route::RemoteScreen => lucide!(
                r#"<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><path d="M3.29 7 12 12l8.71-5M12 22V12"/>"#
            ),
            Route::Diagnostics => lucide!(r#"<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>"#),
        }
    }

    /// One line on what the page shows, for Home's cards.
    pub fn summary(self) -> &'static str {
        match self {
            Route::Home => "Every page of the boilerplate.",
            Route::UiGallery => "Every control of the app's own library, with its states.",
            Route::NativeVsApp => "Each control as the app draws it and as the system does.",
            Route::Forms => "A full sign-up and a settings form on nsh-form.",
            Route::Data => "An infinite feed on nsh-query with optimistic likes.",
            Route::DataDetail(_) => "One post, read through a tracked query key.",
            Route::QueryDevtools => "The query cache: keys, states and timings.",
            Route::Lists => "Tables, a tree, a calendar, a chat and a carousel.",
            Route::Canvas => "Drawing, charts, shaders, SVG, media and the web view.",
            Route::Motion => "Springs, transitions, swipe rows and gestures.",
            Route::Overlays => "Dialogs, sheets, menus, toasts and desktop panes.",
            Route::Platform => "Safe areas, clipboard, picker, speech and locales.",
            Route::Device => "The device, its screen and the environment.",
            Route::RemoteScreen => "A signed TSX package, verified and revocable.",
            Route::Diagnostics => "Config, capabilities, tasks, events and the log.",
        }
    }

    pub fn key_str(self) -> &'static str {
        match self {
            Route::Home => "home",
            Route::UiGallery => "ui",
            Route::NativeVsApp => "native",
            Route::Forms => "forms",
            Route::Data => "data",
            Route::QueryDevtools => "query-devtools",
            Route::Lists => "lists",
            Route::Canvas => "canvas",
            Route::Motion => "motion",
            Route::Overlays => "overlays",
            Route::Platform => "platform",
            Route::Device => "device",
            Route::RemoteScreen => "remote",
            Route::Diagnostics => "diagnostics",
            Route::DataDetail(_) => "data-item",
        }
    }

    /// The Home row's test id (`page-<key>`), as a literal: the ui row
    /// wrapper carries test ids on `&'static str`.
    /// The test id of the route's tab in the themed (Android) tab bar.
    pub fn tab_id(self) -> &'static str {
        match self {
            Route::Home => "tab-home",
            Route::Forms => "tab-forms",
            Route::Data => "tab-data",
            Route::Lists => "tab-lists",
            _ => "tab-motion",
        }
    }

    pub fn page_row_id(self) -> &'static str {
        match self {
            Route::Home => "page-home",
            Route::UiGallery => "page-ui",
            Route::NativeVsApp => "page-native",
            Route::Forms => "page-forms",
            Route::Data => "page-data",
            Route::QueryDevtools => "page-query-devtools",
            Route::Lists => "page-lists",
            Route::Canvas => "page-canvas",
            Route::Motion => "page-motion",
            Route::Overlays => "page-overlays",
            Route::Platform => "page-platform",
            Route::Device => "page-device",
            Route::RemoteScreen => "page-remote",
            Route::Diagnostics => "page-diagnostics",
            Route::DataDetail(_) => "page-data-item",
        }
    }

    /// The sidebar row's test id (`nav-<key>`), as a literal.
    pub fn nav_row_id(self) -> &'static str {
        match self {
            Route::Home => "nav-home",
            Route::UiGallery => "nav-ui",
            Route::NativeVsApp => "nav-native",
            Route::Forms => "nav-forms",
            Route::Data => "nav-data",
            Route::QueryDevtools => "nav-query-devtools",
            Route::Lists => "nav-lists",
            Route::Canvas => "nav-canvas",
            Route::Motion => "nav-motion",
            Route::Overlays => "nav-overlays",
            Route::Platform => "nav-platform",
            Route::Device => "nav-device",
            Route::RemoteScreen => "nav-remote",
            Route::Diagnostics => "nav-diagnostics",
            Route::DataDetail(_) => "nav-data-item",
        }
    }

    /// The system header's title.
    pub fn title(self) -> SharedString {
        self.label().into()
    }

    /// The pages the phone's tab bar holds, in its order.
    pub const TABS: [Route; 5] = [
        Route::Home,
        Route::Forms,
        Route::Data,
        Route::Lists,
        Route::Motion,
    ];

    /// One of the phone's tabs: a root there, so it shows no back button.
    pub fn is_tab(self) -> bool {
        Route::TABS.contains(&self)
    }

    /// A page that runs a `nav_stack` of its own (Forms: sign-up, then
    /// settings), whose pages draw the phone's system header themselves.
    pub fn has_own_stack(self) -> bool {
        matches!(self, Route::Forms)
    }

    /// A page pushed over its parent rather than opened in its place.
    pub fn is_detail(self) -> bool {
        matches!(self, Route::DataDetail(_))
    }

    /// The route a detail sits on.
    pub fn parent(self) -> Option<Route> {
        match self {
            Route::DataDetail(_) => Some(Route::Data),
            _ => None,
        }
    }

    /// The page's content, made by the feature areas.
    pub fn page(self) -> AnyElement {
        match self {
            Route::Home => home::content().into_element(),
            Route::UiGallery => crate::ui::gallery::ui_gallery_page().into_element(),
            Route::NativeVsApp => crate::ui::native_vs_app::native_vs_app_page().into_element(),
            Route::Forms => crate::features::forms::forms_page().into_element(),
            Route::Data => crate::features::data::data_page().into_element(),
            Route::DataDetail(id) => crate::features::data::data_detail_page(id).into_element(),
            Route::QueryDevtools => crate::features::data::query_devtools_page().into_element(),
            Route::Lists => crate::features::lists::lists_page().into_element(),
            Route::Canvas => crate::features::canvas::canvas_page().into_element(),
            Route::Motion => crate::features::motion::motion_page().into_element(),
            Route::Overlays => crate::features::overlays::overlays_page().into_element(),
            Route::Platform => crate::platform::platform_page().into_element(),
            Route::Device => crate::platform::device_page().into_element(),
            Route::RemoteScreen => {
                crate::features::remote_screen::remote_screen_page().into_element()
            }
            Route::Diagnostics => crate::foundation::diagnostics_content().into_element(),
        }
    }
}

/// The route a deep-link path names: the segments after
/// `nsh-starter://`, in order. `None` when the path is not the app's.
pub fn route_from_deep_link(seg: &[&str]) -> Option<Route> {
    match seg {
        [] => Some(Route::Home),
        ["forms"] => Some(Route::Forms),
        ["ui"] => Some(Route::UiGallery),
        ["native"] => Some(Route::NativeVsApp),
        ["data"] => Some(Route::Data),
        ["data", "devtools"] => Some(Route::QueryDevtools),
        ["data", "item", id] => Some(Route::DataDetail(id.parse().ok()?)),
        ["lists"] => Some(Route::Lists),
        ["canvas"] => Some(Route::Canvas),
        ["motion"] => Some(Route::Motion),
        ["overlays"] => Some(Route::Overlays),
        ["platform"] => Some(Route::Platform),
        ["device"] => Some(Route::Device),
        ["remote"] => Some(Route::RemoteScreen),
        ["diagnostics"] => Some(Route::Diagnostics),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_per_route() {
        assert_eq!(Route::DataDetail(3).key(), "data-item:3");
        assert_eq!(Route::DataDetail(3), Route::DataDetail(3));
        assert_ne!(Route::DataDetail(3), Route::DataDetail(4));
    }
}
