//! The carousel: sliding pages of content, with previous and next and a
//! pagination row of dots, all drawn here.

use nsh_base::{
    carousel, carousel_content, carousel_item, carousel_next, carousel_pagination,
    carousel_pagination_item, carousel_previous, theme,
};
use nsh_core::prelude::*;

/// A carousel of `pages`, its shown page in `page`.
pub fn carousel_of(
    name: &'static str,
    pages: Vec<impl IntoElement>,
    page: Signal<u32>,
) -> nsh_base::CarouselElement {
    let count = pages.len() as u32;
    carousel()
        .accessibility_label(name)
        .index(move || Some(page.get()))
        .on_change(move |next: &u32| page.set(*next))
        .flex_col()
        .gap_2()
        .child(
            carousel_content()
                .flex_1()
                .overflow_hidden()
                .rounded_lg()
                .child(pages.into_iter().map(|page_content| {
                    carousel_item()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .rounded_lg()
                        .bg(move || theme().muted)
                        .child(page_content)
                })),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    carousel_previous()
                        .accessibility_label("Previous")
                        .min_h(px(36.0))
                        .min_w(px(36.0))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(move || theme().border)
                        .transition_colors()
                        .hover(|s| s.bg(move || theme().accent))
                        .active(|s| s.opacity(0.7))
                        .focus(|s| {
                            s.outline(px(2.0))
                                .outline_offset(px(2.0))
                                .outline_color(move || theme().ring)
                        })
                        .child(text("‹").text_sm().text_color(move || theme().foreground)),
                )
                .child(
                    carousel_pagination()
                        .accessibility_label("Slides")
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .indicator_style(|s| s.top(px(0.0)).bottom(px(0.0)).rounded_full())
                        .children((0..count).map(|at| {
                            carousel_pagination_item()
                                .index(at)
                                .accessibility_label(format!("Go to {}", at + 1))
                                .min_h(px(24.0))
                                .min_w(px(24.0))
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .selected_style(|s| s.opacity(1.0))
                                .child(
                                    view()
                                        .size(px(8.0))
                                        .rounded_full()
                                        .transition_colors()
                                        .style_when(computed(move || page.get() == at), |s| {
                                            s.bg(move || theme().primary)
                                        }),
                                )
                        })),
                )
                .child(
                    carousel_next()
                        .accessibility_label("Next")
                        .min_h(px(36.0))
                        .min_w(px(36.0))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(move || theme().border)
                        .transition_colors()
                        .hover(|s| s.bg(move || theme().accent))
                        .active(|s| s.opacity(0.7))
                        .focus(|s| {
                            s.outline(px(2.0))
                                .outline_offset(px(2.0))
                                .outline_color(move || theme().ring)
                        })
                        .child(text("›").text_sm().text_color(move || theme().foreground)),
                ),
        )
}
