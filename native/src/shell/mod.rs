//! The shell: lifecycle, the typed route stack, deep links, the sidebar on
//! desktop widths and tabs on phones, the command palette, shortcuts,
//! undo and redo, the desktop chrome, the status bar and the Android back
//! order. The rest of the crate hangs off the route enum here.

pub mod back;
pub mod chrome;
pub mod command_bar;
pub mod deep_link;
pub mod home;
pub mod lifecycle;
pub mod nav;
pub mod page;
pub mod palette;
pub mod routes;
pub mod soft_edges;
pub mod status;
pub mod undo;

use nsh_base::{nav_stack, theme};
use nsh_core::{element::ImageSrc, prelude::*};

use crate::{
    shell::routes::Route,
    ui::{
        button::icon_svg_button,
        icons,
        tab_bar::{ThemedTab, mobile_tab_bar, tab_item, themed_tab_bar},
        toast,
    },
};

/// The width the desktop shell needs: the sidebar and the pane beside it.
/// Desktops take it from 700 pt, other platforms from 900 pt (a wide
/// tablet window); below it is the phone shell. Read in `when`, so the
/// shell remakes once when the window crosses the threshold.
pub fn desktop() -> bool {
    window().width >= if is_desktop() { px(700.0) } else { px(900.0) }
}

/// Flips the system's appearance between light and dark.
pub fn toggle_appearance() {
    if dark() {
        set_color_scheme(crate::Appearance::Light);
    } else {
        set_color_scheme(crate::Appearance::Dark);
    }
}

/// The shell-wide wiring: shortcuts, the back order, the lifecycle watch
/// and the launch deep link. Once, from the root's body.
pub fn install() {
    on_key("mod+k", move |_| palette::toggle());
    on_key("mod+z", move |_| undo::undo());
    on_key("mod+shift+z", move |_| undo::redo());
    on_key("ctrl+mod+s", move |_| chrome::toggle_sidebar());
    back::install();
    lifecycle::watch();
    deep_link::open_pending();
}

/// The shell: the layout of the window's width, with the palette, the
/// exit ask and the toast host over it all.
pub fn shell() -> impl IntoElement {
    view()
        .flex_1()
        .bg(move || theme().background)
        .child(when(desktop, desktop_shell).otherwise(phone_shell))
        .child(palette::palette_dialog())
        .child(command_bar::bridge())
        .child(back::exit_prompt_dialog())
        // App level, so a toast from any page or task reaches the host.
        .child(toast::toast_host())
}

/// Phones: the stack of Home and the pushed pages, and the system tab bar
/// for the primary pages (the status bar is the desktop shell's).
fn phone_shell() -> AnyElement {
    let tabs = Route::TABS;
    // One past the tabs selects none (a route the bar does not show).
    let selected = signal(tabs.len() as u32);
    // The stack drives the tabs...
    effect(move || {
        let at = tabs
            .iter()
            .position(|tab| *tab == nav::current())
            .map_or(tabs.len() as u32, |at| at as u32);
        selected.set(at);
    });
    // A feed post opened on a phone is pushed as the detail route: the
    // system header brings its own back button and the tab bar steps aside.
    // Only while this shell is up: a desktop shell after it overlays.
    crate::features::data::pushes_detail().set(true);
    cleanup(|| crate::features::data::pushes_detail().set(false));
    effect(move || {
        if let Some(id) = crate::features::data::detail_request().get() {
            crate::features::data::close_post();
            nav::push(Route::DataDetail(id));
        }
    });
    // ...and the tabs drive the stack (a no-op when this effect itself
    // moved the selection, so the two never loop).
    effect(move || {
        if let Some(route) = tabs.get(selected.get() as usize)
            && nav::current() != *route
        {
            nav::open(*route);
        }
    });
    view()
        .flex_1()
        .flex_col()
        .child(
            nav_stack()
                .flex_1()
                .child(page::page_screen(Route::Home))
                .child(pushed()),
        )
        .child(
            // Off the detail pages; the bar itself shows nothing off iOS
            // and Android. iOS keeps its system bar (Liquid Glass over the
            // page's own background); Android's Material bar paints its own
            // surface color whatever the theme, so Android draws the bar
            // in the theme's colors instead.
            // It floats over the stack's bottom edge: pages scroll under it
            // and keep room at their end (`ui::layout::page_insets`).
            view()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .style_when(move || nav::current().is_detail(), |s| s.hidden())
                .child(
                    when(
                        || os() == nsh_core::platform::Os::Android,
                        move || {
                            themed_tab_bar(
                                tabs.iter()
                                    .map(|route| ThemedTab {
                                        label: route.label_str(),
                                        svg: route.tab_svg(),
                                        test_id: route.tab_id(),
                                    })
                                    .collect(),
                                selected,
                            )
                            .into_element()
                        },
                    )
                    .otherwise(move || {
                        mobile_tab_bar(
                            tabs.iter()
                                .map(|route| tab_item(route.label_str(), route.tab_icon()))
                                .collect::<Vec<_>>(),
                            selected,
                        )
                        .into_element()
                    }),
                ),
        )
        .into_element()
}

/// Desktop widths: the sidebar running full height under the window
/// controls, as a macOS source list does, and the page stack beside it.
fn desktop_shell() -> AnyElement {
    chrome::install();
    view()
        .flex_1()
        .flex_row()
        .child(when(chrome::sidebar_open, sidebar))
        .child(
            view()
                .flex_1()
                .flex_col()
                .child(
                    nav_stack()
                        .flex_1()
                        .child(page::page_screen(Route::Home))
                        .child(pushed()),
                )
                .child(status::status_bar()),
        )
        .into_element()
}

/// The pushed pages, bottom to top, each dropping its route when the user
/// popped it natively.
fn pushed() -> AnyElement {
    let routes = nav::stack();
    each(
        move || routes.with(|routes| routes.0.clone()),
        |route: &Route| route.key(),
        |route, depth| {
            let route = route.get_untracked();
            page::page_screen(route).on_dismiss(move || nav::truncate(depth.get_untracked()))
        },
    )
    .into_element()
}

/// The desktop layout's sidebar: a draggable top lane for the window
/// controls and the collapse toggle, then every page, the one on top
/// selected.
fn sidebar() -> impl IntoElement {
    view()
        .w(px(232.0))
        .flex_col()
        .bg(move || theme().card)
        .border_r_1()
        .border_color(move || theme().border)
        .child(
            view()
                .h(px(chrome::TITLE_BAR))
                .flex_row()
                .items_center()
                .justify_end()
                .px_3()
                .window_drag(true)
                .child(icon_svg_button(
                    icons::SIDEBAR,
                    "Hide sidebar",
                    chrome::toggle_sidebar,
                )),
        )
        .child(
            scroll().flex_1().child(
                view()
                    .px_2()
                    .pb_4()
                    .gap(px(1.0))
                    .child(
                        text("Pages")
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(move || theme().muted_foreground)
                            .px_2()
                            .pt_1()
                            .pb_1(),
                    )
                    .children(Route::ALL.iter().copied().map(sidebar_row)),
            ),
        )
}

/// A sidebar row: the route's icon and label, the selected fill while its
/// page is on top. Hover is half the accent so exactly one row ever
/// carries the full pill (the default theme's `accent` and `secondary`
/// are the same gray).
fn sidebar_row(route: Route) -> AnyElement {
    let selected = move || nav::current() == route;
    let tint = move || {
        if selected() {
            theme().foreground
        } else {
            theme().muted_foreground
        }
    };
    nsh_base::button()
        .on_press(move || nav::open(route))
        .accessibility_label(route.label_str())
        .selected(selected)
        .w_full()
        .h(px(30.0))
        .px_2()
        .flex_row()
        .items_center()
        .gap_2()
        .rounded(px(6.0))
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent.opacity(0.5)))
        .selected_style(|s| s.bg(move || theme().accent))
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(-2.0))
                .outline_color(move || theme().ring)
        })
        .child(
            view()
                .test_id(route.nav_row_id())
                .flex_1()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    svg(ImageSrc::svg_markup(route.tab_svg()))
                        .size(px(16.0))
                        .text_color(tint),
                )
                .child(
                    text(route.label_str())
                        .flex_1()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .number_of_lines(1)
                        .text_color(move || theme().foreground),
                ),
        )
        .into_element()
}
