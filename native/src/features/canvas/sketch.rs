//! The drawing app: strokes drawn with the responder, an undo stack,
//! stroke color and width through controls. Strokes live in a signal of
//! the component, so a re-layout (width change, appearance change) keeps
//! the drawing.

use nsh_base::{button, theme};
use nsh_core::{
    Bounds, Points,
    canvas::{CanvasTextAlign, CanvasTextBaseline, LineCap, LineJoin},
    element::IntoProp,
    prelude::*,
};

use super::{Palette, caption, section, series_tint, tool_button};

/// A stroke drawn since the pointer went down: its color, its width and
/// the points in the canvas's own coordinates (points from its top left).
#[derive(Clone, PartialEq)]
struct Stroke {
    series: usize,
    width: f32,
    points: Vec<(f32, f32)>,
}

/// Stroke widths the swatches pick, in points.
const WIDTHS: [f32; 3] = [2.0, 4.0, 8.0];

/// The sketch, over its dot grid, `width` wide (0 before its layout).
fn draw(width: f32, height: f32, strokes: &[Stroke]) -> Painter {
    let c = Palette::now();
    let mut p = Painter::new().fill_color(c.border);
    let mut x = 8.0;
    while x < width {
        let mut y = 8.0;
        while y < height {
            p = p.fill_rect(x - 0.75, y - 0.75, 1.5, 1.5);
            y += 16.0;
        }
        x += 16.0;
    }
    if strokes.is_empty() && width > 0.0 {
        p = p
            .font("14px system-ui")
            .fill_color(c.muted_foreground)
            .text_align(CanvasTextAlign::Center)
            .text_baseline(CanvasTextBaseline::Middle)
            .fill_text("Draw here", width / 2.0, height / 2.0);
    }
    for stroke in strokes {
        p = p
            .stroke_color(c.series[stroke.series % c.series.len()])
            .line_width(stroke.width)
            .line_cap(LineCap::Round)
            .line_join(LineJoin::Round)
            .begin_path();
        for (i, &(x, y)) in stroke.points.iter().enumerate() {
            p = if i == 0 {
                p.move_to(x, y)
            } else {
                p.line_to(x, y)
            };
        }
        p = p.stroke();
    }
    p
}

/// A round swatch picking a stroke color or width; the selected ring is
/// the selected state, not a separate border choice.
fn swatch<M>(
    selected: impl IntoProp<bool, M>,
    on_press: impl Fn() + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    button()
        .selected(selected)
        .on_press(move |_: &()| on_press())
        // 44 pt: the touch target minimum; the drawing stays 20 pt inside.
        .size(px(44.0))
        .rounded_full()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(move || theme().border)
        .selected_style(|s| s.border_2().border_color(move || theme().foreground))
        .active(|s| s.opacity(0.7))
        .child(content)
}

/// A solid color swatch for a series tint.
fn color_swatch(series: usize, current: Signal<usize>) -> impl IntoElement {
    swatch(
        Prop::bind(move || current.get() == series),
        move || current.set(series),
        view()
            .size(px(20.0))
            .rounded_full()
            .bg(move || series_tint(series)),
    )
}

/// A swatch showing a stroke width as a line of that thickness.
fn width_swatch(w: f32, current: Signal<f32>) -> impl IntoElement {
    swatch(
        Prop::bind(move || current.get() == w),
        move || current.set(w),
        view()
            .w(px(20.0))
            .h(px(w))
            .rounded(px(1.0))
            .bg(move || theme().foreground),
    )
}

/// The drawing app: the canvas claims touches and redraws on every move.
#[component]
pub(crate) fn Sketch() -> impl IntoElement {
    let strokes = signal(Vec::<Stroke>::new());
    let series = signal(0_usize);
    let width = signal(WIDTHS[1]);
    let canvas_width = signal(0.0_f32);
    let height = 220.0;
    let count_text = computed(move || {
        strokes.with(|drawn| {
            let points: usize = drawn.iter().map(|s| s.points.len()).sum();
            format!("{} strokes · {points} points", drawn.len())
        })
    });
    let has_strokes = computed(move || strokes.with(|s| !s.is_empty()));
    let sketch = canvas(move || strokes.with(|drawn| draw(canvas_width.get(), height, drawn)))
        .h(px(height))
        .test_id("sketch")
        .label("Drawing canvas")
        .on_layout(move |frame: &Bounds<Points>| canvas_width.set(frame.size.width.0))
        // The page around it must not scroll while a stroke is down.
        .claim_touches(TouchClaim::on_start())
        .keep_touches(true)
        .on_responder_grant(move |e: &ResponderEvent| {
            let at = (e.location.x.0, e.location.y.0);
            let (series, width) = (series.get_untracked(), width.get_untracked());
            strokes.update(|s| {
                s.push(Stroke {
                    series,
                    width,
                    points: vec![at],
                })
            });
        })
        .on_responder_move(move |e: &ResponderEvent| {
            let at = (e.location.x.0, e.location.y.0);
            strokes.update(|s| {
                if let Some(stroke) = s.last_mut()
                    && stroke.points.last().copied() != Some(at)
                {
                    stroke.points.push(at);
                }
            });
        });
    view().child(section(
        "Sketch",
        "canvas · responder input · undo",
        view()
            .gap_3()
            .child(sketch)
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .children(WIDTHS.map(|w| width_swatch(w, width)))
                    .children((0..3).map(|s| color_swatch(s, series)))
                    .child(view().flex_1())
                    .child(tool_button(
                        "Undo",
                        move || !has_strokes.get(),
                        move || {
                            strokes.update(|s| {
                                s.pop();
                            })
                        },
                    ))
                    .child(tool_button(
                        "Clear",
                        move || !has_strokes.get(),
                        move || strokes.set(Vec::new()),
                    )),
            )
            .child(caption("stroke-count", move || count_text.get())),
    ))
}
