//! The scroll-linked header: a `diff_clamp` of the scroll offset hides it
//! as the list scrolls up and returns it as the list scrolls down, all on
//! the UI thread; the rest offset is a readout the `ScrollEnd` event writes.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{api_line, section, status_line};

/// The header's height, and the clamp's range.
const HEADER: f32 = 52.0;

/// A list row, so the demo has something to scroll.
fn row(n: u32) -> impl IntoElement {
    view()
        .h(px(44.0))
        .flex_row()
        .items_center()
        .px(px(12.0))
        .rounded(move || theme().radius)
        .bg(move || theme().card)
        .border_1()
        .border_color(move || theme().border)
        .child(
            text(format!("Row {n}"))
                .flex_1()
                .text_size(px(14.0))
                .text_color(move || theme().card_foreground),
        )
}

pub(crate) fn collapsing_header() -> impl IntoElement {
    // The scroll offset, and the header's height following it in 0..HEADER.
    let offset = animated_value(0.0);
    let shown = offset.diff_clamp(0.0, HEADER);
    // Where the scroll view last came to rest, from its own event.
    let rest = signal(String::from("At rest at 0"));
    let rows = (1..=24u32).map(row).collect::<Vec<_>>();
    section(
        "Scroll-linked header",
        "diff_clamp of the scroll offset hides and returns the header",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(
                view()
                    .h(px(300.0))
                    .overflow_hidden()
                    .rounded(move || theme().radius)
                    .border_1()
                    .border_color(move || theme().border)
                    .child(
                        scroll()
                            .absolute()
                            .inset_0()
                            .test_id("header-scroll")
                            .scroll_value(&offset)
                            .on_scroll_end(move |at: &Point<Points>| {
                                rest.set(format!("At rest at {}", at.y.0.round()));
                            })
                            .child(
                                view()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .pt(px(HEADER + 8.0))
                                    .px(px(8.0))
                                    .pb(px(8.0))
                                    .children(rows),
                            ),
                    )
                    .child(
                        view()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top_0()
                            .h(px(HEADER))
                            .flex_row()
                            .items_center()
                            .px(px(14.0))
                            .bg(move || theme().secondary)
                            .bind_translate_y(&shown.mul(-1.0))
                            .bind_opacity(&shown.interpolate(&[0.0, HEADER], &[0.0, 1.0]))
                            .child(
                                text("Now playing")
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(move || theme().secondary_foreground),
                            ),
                    ),
            )
            .child(status_line("header-rest", move || rest.get()))
            .child(api_line(
                "Follows the scroll both ways",
                "scroll_value + diff_clamp(0, 52)",
            )),
    )
}
