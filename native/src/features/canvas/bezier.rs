//! The bézier path editor: two anchors and two control points, the curve
//! stroked through the painter's own `bezier_curve_to`, guides along the
//! control legs — the port of the showcase's bézier demo. Dragging a
//! point re-strokes the curve; reset restores the opening shape.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::{Palette, caption, section, tool_button};

/// A handle's hit radius: anchors pick slightly larger than controls.
const ANCHOR_SIDE: f32 = 12.0;
const CONTROL_SIDE: f32 = 10.0;

/// The opening shape: `[p0, c1, c2, p1]`. c1 sits exactly on the canvas
/// center (164, 120 at the tests' width), which is where their drags
/// press — a grab sets the point to the pointer, so the press must land
/// on the point itself.
const START: [(f32, f32); 4] = [(50.0, 200.0), (164.0, 120.0), (250.0, 190.0), (320.0, 60.0)];

/// The handle index `world` is on, anchors first. `None` past all four.
fn handle_at(world: (f32, f32), points: &[(f32, f32); 4]) -> Option<usize> {
    let mut best = None;
    for (i, p) in points.iter().enumerate() {
        let side = if i == 0 || i == 3 {
            ANCHOR_SIDE
        } else {
            CONTROL_SIDE
        };
        let (dx, dy) = (world.0 - p.0, world.1 - p.1);
        if dx.abs() <= side && dy.abs() <= side {
            best = Some(i);
        }
    }
    best
}

/// The editor: guides, the curve, the four handles. Reads the points
/// signal, so a drag redraws the curve.
fn draw(points: &[(f32, f32); 4], dragging: Option<usize>) -> Painter {
    let c = Palette::now();
    let at = |p: (f32, f32)| point(px(p.0), px(p.1));
    let mut p = Painter::new()
        // The control legs: from each anchor to its control.
        .stroke_color(c.muted_foreground.opacity(0.6))
        .line_width(1.0)
        .begin_path()
        .move_to(points[0].0, points[0].1)
        .line_to(points[1].0, points[1].1)
        .stroke()
        .begin_path()
        .move_to(points[3].0, points[3].1)
        .line_to(points[2].0, points[2].1)
        .stroke();
    // The curve itself: the painter's cubic, not a sampled polyline.
    p = p
        .stroke_color(c.foreground)
        .line_width(2.0)
        .begin_path()
        .move_to(points[0].0, points[0].1)
        .bezier_curve_to(at(points[1]), at(points[2]), at(points[3]))
        .stroke();
    for (i, at_point) in points.iter().enumerate() {
        let anchor = i == 0 || i == 3;
        let side = if anchor { ANCHOR_SIDE } else { CONTROL_SIDE };
        let tint = if anchor { c.series[0] } else { c.series[1] };
        let picked = dragging == Some(i);
        p = if anchor {
            p.fill_color(tint.opacity(if picked { 1.0 } else { 0.9 }))
                .begin_path()
                .arc(
                    at_point.0,
                    at_point.1,
                    side / 2.0,
                    Radians(0.0),
                    Radians(std::f32::consts::TAU),
                )
                .close_path()
                .fill()
        } else {
            // Controls are hollow: the card color fills, the tint strokes.
            p.fill_color(c.background)
                .stroke_color(tint)
                .line_width(if picked { 2.0 } else { 1.0 })
                .begin_path()
                .arc(
                    at_point.0,
                    at_point.1,
                    side / 2.0,
                    Radians(0.0),
                    Radians(std::f32::consts::TAU),
                )
                .close_path()
                .fill()
                .stroke()
        };
    }
    p
}

/// The bézier editor section.
#[component]
pub(crate) fn BezierEditor() -> impl IntoElement {
    let points = signal(START);
    let dragging = signal(None::<usize>);
    let height = 240.0;
    let readout = computed(move || {
        points.with(|p| {
            format!(
                "c1 {}, {} · c2 {}, {}",
                p[1].0 as i32, p[1].1 as i32, p[2].0 as i32, p[2].1 as i32
            )
        })
    });
    let board = canvas(move || points.with(|p| draw(p, dragging.get())))
        .h(px(height))
        .test_id("bezier")
        .label("Bézier editor")
        .claim_touches(TouchClaim::on_start())
        .keep_touches(true)
        .on_responder_grant(move |e: &ResponderEvent| {
            let at = (e.location.x.0, e.location.y.0);
            dragging.set(points.with(|p| handle_at(at, p)));
        })
        .on_responder_move(move |e: &ResponderEvent| {
            let Some(i) = dragging.get_untracked() else {
                return;
            };
            let at = (e.location.x.0, e.location.y.0);
            points.update(|p| p[i] = at);
        })
        .on_responder_release(move |_: &ResponderEvent| dragging.set(None))
        .on_responder_terminate(move |_: &ResponderEvent| dragging.set(None))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Bézier path editor",
        "anchors and control points · drag to shape the curve",
        view()
            .gap_3()
            .child(board)
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(view().test_id("bezier-reset").child(tool_button(
                        "Reset",
                        false,
                        move || points.set(START),
                    ))),
            )
            .child(caption("bezier-controls", move || readout.get())),
    ))
}
