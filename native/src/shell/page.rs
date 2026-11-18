//! Every page's frame: a `nav_page` with the route's key (pages below
//! keep their state), its title for the system header, and the shell's
//! header slot for the desktops and the web, which carry the back button
//! and the palette.

use nsh_base::{nav_page, theme};
use nsh_core::{element::Screen, prelude::*};

use crate::{
    shell::{chrome, nav, palette, routes::Route},
    ui::{icons, native_kit::native_icon_button, nav::themed_header},
};

/// The widest the page content grows, in points: past it, label and value
/// columns drift too far apart to read as one row.
const CONTENT_MAX: f32 = 1040.0;

/// The route's page as the stack shows it.
pub fn page_screen(route: Route) -> Screen {
    let mut screen = themed_header(nav_page().key(route.key()).title(route.title()));
    // A detail over the tab bar's pages keeps the bar out while it is up.
    if route.is_detail() {
        screen = screen.hides_bottom_bar_when_pushed(true);
    }
    // A tab is a root on phones: the tab bar, not a back button, leaves it
    // (Android's system back still returns Home, the shell's back order).
    if route.is_tab() && !is_desktop() {
        screen = screen.header_back_visible(false).pop_on_swipe(false);
    }
    // Under the transparent iOS header, UIKit's soft edge: content fades
    // and blurs lightly as it slides under.
    let top_fade = os() == nsh_core::platform::Os::Ios && !is_desktop();
    let detail = route.is_detail();
    // nav_page paints the platform's window color (macOS dark: #1e1e1e),
    // so the content area takes the theme's own background on every page;
    // wide windows center the content at a readable measure.
    let content = route.page();
    screen.header(page_header(route)).child(
        view()
            .flex_1()
            .items_center()
            .bg(move || theme().background)
            .child(
                view()
                    .flex_1()
                    .w_full()
                    .max_w(px(CONTENT_MAX))
                    .child(content),
            )
            .child(when(move || top_fade, crate::shell::soft_edges::soft_edges))
            // ...and behind the floating tab bar, the bottom one.
            .child(when(
                move || top_fade && !detail,
                || {
                    crate::shell::soft_edges::soft_bottom_edge(|| {
                        crate::ui::layout::tab_bar_room() + safe_area().bottom.0
                    })
                },
            )),
    )
}

/// The header slot (desktops and the web only; iOS and Android draw their
/// system header from `title`), drawn as a unified toolbar: draggable, as
/// tall as the title bar, clear of the window controls while the sidebar
/// is hidden. A page is born pushed or root, so the back button's
/// presence is decided here.
fn page_header(route: Route) -> impl IntoElement {
    let lead = move || {
        if chrome::sidebar_open() {
            px(12.0)
        } else {
            chrome::controls_inset() + px(12.0)
        }
    };
    view()
        .h(px(chrome::TITLE_BAR))
        .flex_row()
        .items_center()
        .gap_1()
        .pl(lead)
        .pr_3()
        .border_b_1()
        .border_color(move || theme().border)
        .bg(move || theme().background)
        .window_drag(true)
        .child(when(
            move || !chrome::sidebar_open(),
            || {
                native_icon_button(
                    "sidebar.left",
                    icons::SIDEBAR,
                    "Show sidebar",
                    chrome::toggle_sidebar,
                )
            },
        ))
        .when(route != Route::Home, |header| {
            header.child(native_icon_button(
                "chevron.left",
                icons::BACK,
                "Back",
                nav::pop,
            ))
        })
        .child(
            text(route.title())
                .flex_1()
                .px_1()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .number_of_lines(1),
        )
        .when(is_desktop(), |header| {
            header.child(native_icon_button(
                "magnifyingglass",
                icons::SEARCH,
                "Commands",
                palette::toggle,
            ))
        })
}
