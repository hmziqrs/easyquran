//! Featured covers: a `Carousel` of five slides the app controls — the
//! index comes back through `onChange` and drives the caption and the
//! pagination dots.

use crate::ui::carousel::carousel_of;
use nsh_base::theme;
use nsh_core::prelude::*;

use super::{heading, note};

/// The slides: what each cover is.
const SLIDES: [&str; 5] = [
    "Field notes",
    "Sprint 24",
    "Customer stories",
    "The archive",
    "Roadmap",
];

/// A slide's cover, tinted from the chart palette (the color is the
/// slide's data).
fn cover(at: usize) -> impl IntoElement {
    let tint = move || match at {
        0 => theme().chart_1,
        1 => theme().chart_2,
        2 => theme().chart_3,
        3 => theme().chart_4,
        _ => theme().chart_5,
    };
    view()
        .w_full()
        .h(px(170.0))
        .rounded_lg()
        .bg(tint)
        .items_center()
        .justify_center()
        .child(
            text(SLIDES[at])
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().background),
        )
}

/// The Carousel demo: swipe the covers, or use the arrows and dots.
#[component]
pub fn CarouselDemo() -> impl IntoElement {
    let index = signal(0_u32);
    let caption = move || format!("Slide {} of {}", index.get() + 1, SLIDES.len());
    let pages: Vec<_> = (0..SLIDES.len()).map(cover).collect();
    view()
        .flex_1()
        .gap_3()
        .pb_4()
        .child(heading("Carousel").child(note("Swipe the covers, or use the arrows")))
        .child(carousel_of("Featured", pages, index))
        .child(
            text(caption)
                .text_sm()
                .tabular_nums()
                .text_color(move || theme().muted_foreground),
        )
}
