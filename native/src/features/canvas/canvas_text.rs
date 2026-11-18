//! Text on canvas: `fill_text` at three sizes over shown baselines, the
//! same string anchored left, center and right across the width, and an
//! outlined `stroke_text` — the port of the showcase's text demo. Static:
//! it draws, it takes no input.

use nsh_base::theme;
use nsh_core::{
    Points,
    canvas::{CanvasTextAlign, CanvasTextBaseline},
    prelude::*,
};

use super::{Palette, caption, section};

/// The specimen's font sizes, in points.
const SIZES: [f32; 3] = [14.0, 20.0, 28.0];

/// The specimen. `width` is 0 before the layout.
fn draw(width: f32, height: f32) -> Painter {
    let c = Palette::now();
    let mut p = Painter::new();
    let mut baseline = 26.0;
    for size in SIZES {
        // The rule under each row makes the alphabetic baseline visible;
        // the string stays short so even the 28 pt row fits a phone width.
        p = p
            .stroke_color(c.border.opacity(0.6))
            .line_width(1.0)
            .begin_path()
            .move_to(12.0, baseline)
            .line_to(width - 12.0, baseline)
            .stroke()
            .font(&format!("{size}px system-ui"))
            .fill_color(c.foreground)
            .text_align(CanvasTextAlign::Left)
            .text_baseline(CanvasTextBaseline::Alphabetic)
            .fill_text(&format!("{size} pt on the rule"), 12.0, baseline);
        baseline += size * 1.5;
    }
    // One string, three anchors: the row's y is mid-height of what is left.
    let row = baseline + 14.0;
    let label = "aligned";
    for (align, anchor) in [
        (CanvasTextAlign::Left, 12.0),
        (CanvasTextAlign::Center, width / 2.0),
        (CanvasTextAlign::Right, width - 12.0),
    ] {
        p = p
            .font("16px system-ui")
            .fill_color(c.series[1])
            .text_align(align)
            .text_baseline(CanvasTextBaseline::Middle)
            .fill_text(label, anchor, row);
    }
    if height > row + 40.0 {
        // The outlined word shows the stroke side of text.
        p = p
            .font("26px system-ui")
            .stroke_color(c.series[0])
            .line_width(1.0)
            .text_align(CanvasTextAlign::Center)
            .text_baseline(CanvasTextBaseline::Middle)
            .stroke_text("outlined", width / 2.0, row + 38.0);
    }
    p
}

/// The text specimen section.
#[component]
pub(crate) fn TextOnCanvas() -> impl IntoElement {
    let canvas_width = signal(0.0_f32);
    let height = 170.0;
    let canvas = canvas(move || draw(canvas_width.get(), height))
        .h(px(height))
        .test_id("canvas-text")
        .label("Text specimen")
        .on_layout(move |frame: &Bounds<Points>| canvas_width.set(frame.size.width.0))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Text on canvas",
        "fill_text · sizes, alignment, baselines · stroke_text",
        view()
            .gap_3()
            .child(canvas)
            .child(caption("canvas-text-note", move || {
                "glyphs shape through the engine; the rules mark their baselines".to_owned()
            })),
    ))
}
