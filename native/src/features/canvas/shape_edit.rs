//! The shape editor: a rect, an ellipse and a triangle; a tap selects and
//! a drag moves, while the selected shape shows eight handles that drag
//! to resize — the port of the showcase's shape-editing demo. Shapes are
//! data in a signal, so a re-layout keeps them.

use nsh_base::theme;
use nsh_core::{Bounds, prelude::*};

use super::{Palette, caption, section, tool_button};

/// A shape: its kind and its bounds, in the canvas's own coordinates.
#[derive(Clone, Copy, PartialEq)]
struct Shape {
    kind: Kind,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Rect,
    Ellipse,
    Triangle,
}

/// A resize handle index, clockwise from the top-left corner: 0..8, edges
/// at the odd indexes.
const HANDLES: usize = 8;

/// Half a handle's side; the pick radius and the painted square.
const HANDLE_HALF: f32 = 6.0;

/// A resize never collapses a shape past this.
const MIN_SIDE: f32 = 20.0;

/// The opening shapes. The rect straddles the canvas center with its east
/// handle 18 points off it, so the tests can press the interior first and
/// the handle later — the view tests drag from the element's center.
const START: [Shape; 3] = [
    Shape {
        kind: Kind::Rect,
        x: 14.0,
        y: 90.0,
        w: 150.0,
        h: 84.0,
    },
    Shape {
        kind: Kind::Ellipse,
        x: 210.0,
        y: 30.0,
        w: 110.0,
        h: 70.0,
    },
    Shape {
        kind: Kind::Triangle,
        x: 60.0,
        y: 220.0,
        w: 120.0,
        h: 70.0,
    },
];

/// The corner the `handle`-th handle sits on: corners first, then edges.
fn handle_position(s: &Shape, handle: usize) -> (f32, f32) {
    let (right, bottom) = (s.x + s.w, s.y + s.h);
    let mid_x = s.x + s.w / 2.0;
    let mid_y = s.y + s.h / 2.0;
    [
        (s.x, s.y),
        (mid_x, s.y),
        (right, s.y),
        (right, mid_y),
        (right, bottom),
        (mid_x, bottom),
        (s.x, bottom),
        (s.x, mid_y),
    ][handle % HANDLES]
}

/// The handle under `world`, checking the selected shape's handles.
fn handle_at(world: (f32, f32), s: &Shape) -> Option<usize> {
    (0..HANDLES).find(|&i| {
        let (hx, hy) = handle_position(s, i);
        (world.0 - hx).abs() <= HANDLE_HALF + 4.0 && (world.1 - hy).abs() <= HANDLE_HALF + 4.0
    })
}

/// The topmost shape under `world`; the ellipse counts inside its oval.
fn shape_at(world: (f32, f32), shapes: &[Shape]) -> Option<usize> {
    (0..shapes.len()).rev().find(|&i| {
        let s = &shapes[i];
        let (cx, cy) = (s.x + s.w / 2.0, s.y + s.h / 2.0);
        match s.kind {
            Kind::Rect | Kind::Triangle => {
                world.0 >= s.x && world.0 <= s.x + s.w && world.1 >= s.y && world.1 <= s.y + s.h
            }
            Kind::Ellipse => {
                let (rx, ry) = (s.w / 2.0, s.h / 2.0);
                let (dx, dy) = ((world.0 - cx) / rx, (world.1 - cy) / ry);
                dx * dx + dy * dy <= 1.0
            }
        }
    })
}

/// `start` resized by dragging its `handle` to `to`: the opposite edge or
/// corner stays anchored, and neither side collapses past `MIN_SIDE`.
fn resize(start: &Shape, handle: usize, to: (f32, f32)) -> Shape {
    let mut out = *start;
    let east = handle == 2 || handle == 3 || handle == 4;
    let west = handle == 6 || handle == 7 || handle == 0;
    let south = handle == 4 || handle == 5 || handle == 6;
    let north = handle == 0 || handle == 1 || handle == 2;
    if west {
        let right = (start.x + start.w).max(to.0 + MIN_SIDE);
        out.x = to.0.min(right - MIN_SIDE);
        out.w = right - out.x;
    }
    if east {
        out.w = (to.0 - start.x).max(MIN_SIDE);
    }
    if north {
        let bottom = (start.y + start.h).max(to.1 + MIN_SIDE);
        out.y = to.1.min(bottom - MIN_SIDE);
        out.h = bottom - out.y;
    }
    if south {
        out.h = (to.1 - start.y).max(MIN_SIDE);
    }
    out
}

/// The bounds readout of the selection: "x, y · w × h".
fn bounds_text(s: &Shape) -> String {
    format!(
        "{}, {} · {} × {}",
        s.x as i32, s.y as i32, s.w as i32, s.h as i32
    )
}

/// The editor's frame: the shapes, the selection ring with its handles,
/// and the shape the drag is on in its picked tint.
fn draw(shapes: &[Shape], selected: Option<usize>, drag: Option<(bool, usize)>) -> Painter {
    let c = Palette::now();
    let mut p = Painter::new();
    for (i, s) in shapes.iter().enumerate() {
        let bounds = Bounds::new(point(px(s.x), px(s.y)), size(px(s.w), px(s.h)));
        let picked = drag.map(|(moving, ix)| moving && ix == i).unwrap_or(false);
        let tint = c.series[i % c.series.len()].opacity(if picked { 0.55 } else { 0.35 });
        p = p.fill_color(tint).begin_path();
        p = match s.kind {
            Kind::Rect => p.round_rect(bounds, px(6.0)),
            Kind::Ellipse => p.ellipse(
                point(px(s.x + s.w / 2.0), px(s.y + s.h / 2.0)),
                size(px(s.w / 2.0), px(s.h / 2.0)),
                Radians(0.0),
                Radians(0.0),
                Radians(std::f32::consts::TAU),
                false,
            ),
            Kind::Triangle => p
                .move_to(s.x + s.w / 2.0, s.y)
                .line_to(s.x + s.w, s.y + s.h)
                .line_to(s.x, s.y + s.h)
                .close_path(),
        };
        p = p.fill();
        if selected == Some(i) {
            // The ring, then the eight handles over it.
            p = p
                .stroke_color(c.series[0])
                .line_width(1.5)
                .begin_path()
                .round_rect(bounds, px(6.0))
                .stroke();
            for handle in 0..HANDLES {
                let (hx, hy) = handle_position(s, handle);
                p = p
                    .fill_color(c.foreground)
                    .begin_path()
                    .rect(Bounds::new(
                        point(px(hx - HANDLE_HALF), px(hy - HANDLE_HALF)),
                        size(px(HANDLE_HALF * 2.0), px(HANDLE_HALF * 2.0)),
                    ))
                    .fill();
            }
        } else {
            p = p
                .stroke_color(c.foreground.opacity(0.5))
                .line_width(1.0)
                .begin_path()
                .round_rect(bounds, px(6.0))
                .stroke();
        }
    }
    p
}

/// The shape editor section.
#[component]
pub(crate) fn ShapeEditing() -> impl IntoElement {
    let shapes = signal(START.to_vec());
    // What a drag is: `None`, or `(moving, index)` — moving is a move
    // drag, false is a resize of the selected shape's handle.
    let drag = signal(None::<(bool, usize)>);
    let last = signal((0.0_f32, 0.0_f32));
    let resize_start = signal(Shape {
        kind: Kind::Rect,
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    });
    let selected = signal(None::<usize>);
    let height = 300.0;
    let selection_text = computed(move || match selected.get() {
        Some(i) => shapes.with(|all| {
            all.get(i)
                .map(bounds_text)
                .unwrap_or_else(|| "none".to_owned())
        }),
        None => "none".to_owned(),
    });
    let reset = move || {
        batch(|| {
            shapes.set(START.to_vec());
            selected.set(None);
            drag.set(None);
        });
    };
    let board = canvas(move || shapes.with(|all| draw(all, selected.get(), drag.get())))
        .h(px(height))
        .test_id("shape-edit")
        .label("Shape editor")
        .claim_touches(TouchClaim::on_start())
        .keep_touches(true)
        .on_responder_grant(move |e: &ResponderEvent| {
            let at = (e.location.x.0, e.location.y.0);
            let all = shapes.get_untracked();
            // Handles first: they sit over their shape, so they win the pick.
            if let Some(i) = selected.get_untracked()
                && let Some(s) = all.get(i)
                && let Some(handle) = handle_at(at, s)
            {
                batch(|| {
                    resize_start.set(*s);
                    drag.set(Some((false, handle)));
                    last.set(at);
                });
                return;
            }
            if let Some(i) = shape_at(at, &all) {
                batch(|| {
                    selected.set(Some(i));
                    drag.set(Some((true, i)));
                    last.set(at);
                });
            } else {
                selected.set(None);
            }
        })
        .on_responder_move(move |e: &ResponderEvent| {
            let Some((moving, i)) = drag.get_untracked() else {
                return;
            };
            let at = (e.location.x.0, e.location.y.0);
            let from = last.get();
            if from == at {
                return;
            }
            last.set(at);
            if moving {
                shapes.update(|all| {
                    if let Some(s) = all.get_mut(i) {
                        s.x += at.0 - from.0;
                        s.y += at.1 - from.1;
                    }
                });
            } else {
                // Here `i` is the handle index; the shape is whichever one is
                // selected.
                let Some(target) = selected.get_untracked() else {
                    return;
                };
                let start = resize_start.get_untracked();
                shapes.update(|all| {
                    if let Some(s) = all.get_mut(target) {
                        *s = resize(&start, i, at);
                    }
                });
            }
        })
        .on_responder_release(move |_: &ResponderEvent| drag.set(None))
        .on_responder_terminate(move |_: &ResponderEvent| drag.set(None))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Shape editing",
        "tap selects · drag moves · handles resize",
        view()
            .gap_3()
            .child(board)
            .child(
                view().flex_row().gap_2().child(
                    view()
                        .test_id("shape-edit-reset")
                        .child(tool_button("Reset", false, reset)),
                ),
            )
            .child(caption("shape-edit-selection", move || {
                format!("selected bounds: {}", selection_text.get())
            })),
    ))
}
