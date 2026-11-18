//! The shell's index: every page reachable from here, as the smoke suite
//! walks it. Rows are the ui `nav_row`, whose test ids (`page-<key>`) let
//! the suite press them apart from the sidebar's same-named rows on
//! desktop widths.

use nsh_base::{button, theme};
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    shell::{
        nav,
        routes::{Route, route_from_deep_link},
    },
    ui::{
        nav::{chevron, nav_row},
        text::muted,
    },
};

/// Home's content, under the page's header: a card per page, wrapping
/// into as many columns as the width holds (one on a phone).
pub fn content() -> AnyElement {
    scroll()
        .flex_1()
        .content_inset(crate::ui::layout::page_insets(0.0))
        .child(
            view()
                .gap_6()
                .p_4()
                .pb_safe_area_plus(px(16.0))
                // The header above already says "Home".
                .child(muted(
                    "Everything the boilerplate demonstrates, one page each.",
                ))
                .child(
                    view().gap_3().child(section_label("Pages")).child(
                        view().flex_row().flex_wrap().gap_3().children(
                            Route::ALL
                                .iter()
                                .filter(|route| **route != Route::Home)
                                .map(|route| tile(*route)),
                        ),
                    ),
                )
                .child(view().gap_1().child(section_label("Pushed")).children([
                    demo_row("page-data-item", "Open data item 3", move || {
                        nav::push(Route::DataDetail(3))
                    }),
                    demo_row("page-deep-link", "Open a deep link", move || {
                        deep_link("nsh-starter://data/item/7")
                    }),
                ])),
        )
        .into_element()
}

/// Deep links from within the app: the same parse the host seam uses.
fn deep_link(url: &str) {
    if let Some(route) = url
        .strip_prefix("nsh-starter://")
        .and_then(|path| route_from_deep_link(&path.split('/').collect::<Vec<_>>()))
    {
        nav::open(route);
    }
}

fn section_label(label: &'static str) -> impl IntoElement {
    text(label)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(move || theme().muted_foreground)
}

/// A page's card: its name over one line of what it shows, the whole card
/// the target. The test id rides the inner view, as `nav_row`'s does.
fn tile(route: Route) -> AnyElement {
    button()
        .on_press(move || nav::open(route))
        .accessibility_label(route.label_str())
        .flex_grow(1.0)
        .basis(px(240.0))
        .min_h(px(84.0))
        .rounded_lg()
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.opacity(0.8))
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .child(
            view()
                .test_id(route.page_row_id())
                .flex_1()
                .p_4()
                .gap_1()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            text(route.label_str())
                                .flex_1()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .number_of_lines(1)
                                .text_color(move || theme().card_foreground),
                        )
                        .child(chevron()),
                )
                .child(
                    text(route.summary())
                        .text_xs()
                        .number_of_lines(2)
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .into_element()
}

fn demo_row(id: &'static str, label: &'static str, open: impl Fn() + 'static) -> AnyElement {
    nav_row(label, Some(chevron()), false, id, open).into_element()
}
