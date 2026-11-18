//! The infinite board: one canvas whose camera (pan, zoom) lives in
//! signals and whose content lives in world coordinates, so pan and zoom
//! never rewrite it. One finger pans (or drags a node in world space);
//! a pinch scales the board natively (`pinch_value` + `bind_scale`) and
//! folds into the camera on `on_pinch_end` — `GestureEvent` carries no
//! centroid on 0.5.0, so the pinch anchors at the view's center. The
//! board's own hit test runs in world space, at any zoom.

use nsh_base::theme;
use nsh_core::{
    Bounds, Points,
    canvas::{CanvasTextAlign, CanvasTextBaseline, LineCap, LineJoin},
    prelude::*,
};

use super::{Palette, caption, section, tool_button};

/// A node's size, in world units.
const NODE: (f32, f32) = (120.0, 56.0);

/// The world content: a node per name and position, and the stroke that
/// links them, all stored in world units. None covers the origin, so a
/// drag from the view's center always pans.
const NODES: [(&str, f32, f32); 3] = [
    ("Aurora", -140.0, -40.0),
    ("Notes", 70.0, 0.0),
    ("Orbit", 90.0, 60.0),
];

/// The world grid's spacing, in world units.
const GRID: f32 = 24.0;

/// The zoom limits.
const ZOOM_MIN: f32 = 0.25;
const ZOOM_MAX: f32 = 4.0;

/// `x` with the `* 1` guard `round()` needs for the readout's integers.
fn at(x: f32) -> i32 {
    x.round() as i32
}

/// `world` on screen: `world * zoom + pan + center` — the camera's pan is
/// the world origin's offset from the view's center, so zooming about the
/// center scales `pan` by the zoom ratio alone.
fn to_screen(world: (f32, f32), pan: (f32, f32), zoom: f32, center: (f32, f32)) -> (f32, f32) {
    (
        world.0 * zoom + pan.0 + center.0,
        world.1 * zoom + pan.1 + center.1,
    )
}

/// The world point under a screen point (the hit test's inverse).
fn to_world(screen: (f32, f32), pan: (f32, f32), zoom: f32, center: (f32, f32)) -> (f32, f32) {
    (
        (screen.0 - pan.0 - center.0) / zoom,
        (screen.1 - pan.1 - center.1) / zoom,
    )
}

/// The node under a world point, topmost first (drawing order).
fn node_at(world: (f32, f32)) -> Option<usize> {
    (0..NODES.len()).rev().find(|&i| {
        let (_, x, y) = NODES[i];
        world.0 >= x - NODE.0 / 2.0
            && world.0 <= x + NODE.0 / 2.0
            && world.1 >= y - NODE.1 / 2.0
            && world.1 <= y + NODE.1 / 2.0
    })
}

/// The board: the grid, the linking stroke and the nodes, all through the
/// camera. `zoom` and `pan` are read here, so a camera change redraws.
fn draw(
    board: (f32, f32),
    pan: (f32, f32),
    zoom: f32,
    nodes: &[(f32, f32)],
    selected: Option<usize>,
) -> Painter {
    let c = Palette::now();
    let center = (board.0 / 2.0, board.1 / 2.0);
    let to = |world: (f32, f32)| to_screen(world, pan, zoom, center);
    // The grid: its spacing is the world grid times the zoom, anchored at
    // the world origin's screen position, so it moves and scales with the
    // camera.
    let step = GRID * zoom;
    let mut p = Painter::new()
        .stroke_color(c.border.opacity(0.5))
        .line_width(1.0);
    let mut offset = (center.0 + pan.0) % step;
    if offset < 0.0 {
        offset += step;
    }
    let mut x = offset;
    while x < board.0 {
        p = p.begin_path().move_to(x, 0.0).line_to(x, board.1).stroke();
        x += step;
    }
    let mut offset = (center.1 + pan.1) % step;
    if offset < 0.0 {
        offset += step;
    }
    let mut y = offset;
    while y < board.1 {
        p = p.begin_path().move_to(0.0, y).line_to(board.0, y).stroke();
        y += step;
    }
    // The world axes, heavier so the origin stays findable at any zoom.
    let (ox, oy) = to((0.0, 0.0));
    p = p
        .stroke_color(c.muted_foreground.opacity(0.6))
        .begin_path()
        .move_to(0.0, oy)
        .line_to(board.0, oy)
        .stroke()
        .begin_path()
        .move_to(ox, 0.0)
        .line_to(ox, board.1)
        .stroke();
    // The stroke that links the nodes, in world coordinates: positions
    // come from the signal, so a dragged node takes its stroke with it.
    p = p
        .stroke_color(c.series[2].opacity(0.7))
        .line_width(2.0 * zoom)
        .line_cap(LineCap::Round)
        .line_join(LineJoin::Round)
        .begin_path();
    for (i, (x, y)) in nodes.iter().enumerate() {
        let at = to((*x, *y));
        p = if i == 0 {
            p.move_to(at.0, at.1)
        } else {
            p.line_to(at.0, at.1)
        };
    }
    p = p.stroke();
    // The nodes: a card per entry, its label set in screen points scaled
    // by the zoom.
    for (i, (name, _, _)) in NODES.iter().enumerate() {
        let (sx, sy) = to(nodes[i]);
        let (w, h) = (NODE.0 * zoom, NODE.1 * zoom);
        let tint = if selected == Some(i) {
            c.series[0]
        } else {
            c.muted_foreground.opacity(0.5)
        };
        p = p
            .fill_color(c.background)
            .stroke_color(tint)
            .line_width(if selected == Some(i) { 2.0 } else { 1.0 })
            .begin_path()
            .round_rect(
                Bounds::new(
                    point(px(sx - w / 2.0), px(sy - h / 2.0)),
                    size(px(w), px(h)),
                ),
                px(8.0 * zoom),
            )
            .fill()
            .stroke()
            .font(&format!("{}px system-ui", (12.0 * zoom) as i32))
            .text_align(CanvasTextAlign::Center)
            .text_baseline(CanvasTextBaseline::Middle)
            .fill_color(c.foreground)
            .fill_text(name, sx, sy);
    }
    p
}

/// The infinite board section.
#[component]
pub(crate) fn WorldBoard() -> impl IntoElement {
    let pan = signal((0.0_f32, 0.0_f32));
    let zoom = signal(1.0_f32);
    let nodes = signal(NODES.map(|(_, x, y)| (x, y)).to_vec());
    let selected = signal(None::<usize>);
    // What a node drag or a pan is tracking, the pointer's last spot, and
    // whether a pinch is down (its end may arrive alone).
    let drag_node = signal(None::<usize>);
    let last = signal((0.0_f32, 0.0_f32));
    let pinching = signal(false);
    let size = signal((0.0_f32, 0.0_f32));
    let height = 240.0;
    // The pinch: the platform scales the board natively while the fingers
    // are down; the end folds the scale into the camera so the world point
    // at the view's center stays put (pan scales with the zoom).
    let wave = animated_value(1.0);
    let camera_text = computed(move || {
        let (zx, px, py) = (zoom.get(), pan.get().0, pan.get().1);
        format!("world {}% · pan {}, {}", at(zx * 100.0), at(px), at(py))
    });
    let under_text = computed(move || {
        let (p, z, s) = (pan.get(), zoom.get(), size.get());
        let center = to_world((s.0 / 2.0, s.1 / 2.0), p, z, (s.0 / 2.0, s.1 / 2.0));
        format!(
            "center {}, {} · under: {}",
            at(center.0),
            at(center.1),
            node_at(center).map_or("none", |i| NODES[i].0)
        )
    });
    let selection_text = computed(move || match selected.get() {
        Some(i) => {
            let (x, y) = nodes.get()[i];
            format!("selected: {} · {}, {}", NODES[i].0, at(x), at(y))
        }
        None => "selected: none".to_owned(),
    });
    let zoom_by = move |factor: f32| {
        zoom.update(|z| *z = (*z * factor).clamp(ZOOM_MIN, ZOOM_MAX));
        // The view's center keeps its world point: pan scales with zoom.
        pan.update(|p| *p = (p.0 * factor, p.1 * factor));
    };
    let board = canvas(move || {
        nodes.with(|placed| draw(size.get(), pan.get(), zoom.get(), placed, selected.get()))
    })
    .h(px(height))
    .test_id("world-board")
    .label("Infinite board")
    .on_layout(move |frame: &Bounds<Points>| size.set((frame.size.width.0, frame.size.height.0)))
    // The page around it must not scroll while a gesture is down.
    .claim_touches(TouchClaim::on_start())
    .keep_touches(true)
    .on_responder_grant(move |e: &ResponderEvent| {
        let at = (e.location.x.0, e.location.y.0);
        let (p, z, s) = (
            pan.get_untracked(),
            zoom.get_untracked(),
            size.get_untracked(),
        );
        last.set(at);
        drag_node.set(node_at(to_world(at, p, z, (s.0 / 2.0, s.1 / 2.0))));
        if let Some(i) = drag_node.get_untracked() {
            selected.set(Some(i));
        }
    })
    .on_responder_move(move |e: &ResponderEvent| {
        let at = (e.location.x.0, e.location.y.0);
        let from = last.get();
        if from == at {
            return;
        }
        last.set(at);
        match drag_node.get_untracked() {
            // The node moves in world units: the screen delta divides by
            // the zoom.
            Some(i) => {
                let z = zoom.get();
                nodes.update(|all| {
                    all[i].0 += (at.0 - from.0) / z;
                    all[i].1 += (at.1 - from.1) / z;
                });
            }
            None => pan.update(|p| *p = (p.0 + at.0 - from.0, p.1 + at.1 - from.1)),
        }
    })
    .on_responder_release(move |_: &ResponderEvent| drag_node.set(None))
    .on_responder_terminate(move |_: &ResponderEvent| drag_node.set(None))
    .pinch_value(&wave)
    .bind_scale(&wave)
    .on_pinch_start(move |_: &GestureEvent| pinching.set(true))
    // The fold runs only for a pinch that started: gesture ends also
    // arrive without one, and folding then would corrupt the camera.
    .on_pinch_end(move |_: &GestureEvent| {
        if !pinching.get_untracked() {
            return;
        }
        pinching.set(false);
        spawn(async move {
            let w = wave.value().await.max(0.05);
            zoom.update(|z| *z = (*z * w).clamp(ZOOM_MIN, ZOOM_MAX));
            pan.update(|p| *p = (p.0 * w, p.1 * w));
            wave.set(1.0);
        });
    })
    .rounded(move || theme().radius)
    .border_1()
    .border_color(move || theme().border)
    .bg(move || theme().muted);
    view().child(section(
        "Infinite board",
        "camera pan and zoom · content in world coordinates",
        view()
            .gap_3()
            .child(board)
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(tool_button("Zoom in", false, move || zoom_by(1.25)))
                    .child(tool_button("Zoom out", false, move || zoom_by(0.8)))
                    // "Recenter", not "Reset": the page's other sections
                    // have their own resets, and recentering keeps the
                    // selection.
                    .child(tool_button("Recenter", false, move || {
                        batch(|| {
                            zoom.set(1.0);
                            pan.set((0.0, 0.0));
                        });
                    })),
            )
            .child(caption("world-readout", move || camera_text.get()))
            .child(caption("world-center", move || under_text.get()))
            .child(caption("world-selection", move || selection_text.get())),
    ))
}
