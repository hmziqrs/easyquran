//! The node graph: four named cards linked by edges; a drag moves a node
//! and its edges follow, an empty drag pulls a marquee that selects every
//! node it crosses, and dragging one of the selected moves them all —
//! the port of the showcase's node-graph demo. Multi-select state lives
//! in a signal; the edges are drawn from the same positions signal, so a
//! dragged node takes its edges with it.

use nsh_base::theme;
use nsh_core::{
    Bounds,
    canvas::{CanvasTextAlign, CanvasTextBaseline},
    prelude::*,
};

use super::{Palette, caption, section, tool_button};

/// A node card's size, in canvas points.
const NODE: (f32, f32) = (96.0, 40.0);

/// The graph: a node per name and position, and the edges between them.
const NAMES: [&str; 4] = ["Input", "Filter", "Merge", "Output"];
const EDGES: [(usize, usize); 5] = [(0, 1), (1, 2), (2, 3), (0, 2), (1, 3)];
/// Input straddles the canvas center (the view tests' press point);
/// Filter and Merge sit in the upper-right band a marquee from the
/// center crosses; Output stays outside it.
const START: [(f32, f32); 4] = [(116.0, 120.0), (240.0, 70.0), (250.0, 110.0), (60.0, 220.0)];

/// The node whose card `world` is on, topmost first.
fn node_at(world: (f32, f32), nodes: &[(f32, f32)]) -> Option<usize> {
    (0..nodes.len()).rev().find(|&i| {
        let (x, y) = nodes[i];
        world.0 >= x && world.0 <= x + NODE.0 && world.1 >= y && world.1 <= y + NODE.1
    })
}

/// Whether `node`'s center is inside the marquee `rect`.
fn inside_marquee(node: (f32, f32), rect: ((f32, f32), (f32, f32))) -> bool {
    let (cx, cy) = (node.0 + NODE.0 / 2.0, node.1 + NODE.1 / 2.0);
    let ((x0, y0), (x1, y1)) = rect;
    let (left, right) = (x0.min(x1), x0.max(x1));
    let (top, bottom) = (y0.min(y1), y0.max(y1));
    cx >= left && cx <= right && cy >= top && cy <= bottom
}

/// The graph: the edges, then the cards, then the marquee over all.
fn draw(
    nodes: &[(f32, f32)],
    selected: &[usize],
    marquee: Option<((f32, f32), (f32, f32))>,
) -> Painter {
    let c = Palette::now();
    let center = |i: usize| (nodes[i].0 + NODE.0 / 2.0, nodes[i].1 + NODE.1 / 2.0);
    let mut p = Painter::new()
        .stroke_color(c.muted_foreground.opacity(0.6))
        .line_width(1.5);
    for (a, b) in EDGES {
        let (ax, ay) = center(a);
        let (bx, by) = center(b);
        p = p.begin_path().move_to(ax, ay).line_to(bx, by).stroke();
    }
    for (i, (x, y)) in nodes.iter().enumerate() {
        let bounds = Bounds::new(point(px(*x), px(*y)), size(px(NODE.0), px(NODE.1)));
        let picked = selected.contains(&i);
        let tint = if picked {
            c.series[0]
        } else {
            c.muted_foreground.opacity(0.5)
        };
        p = p
            .fill_color(c.background)
            .stroke_color(tint)
            .line_width(if picked { 2.0 } else { 1.0 })
            .begin_path()
            .round_rect(bounds, px(8.0))
            .fill()
            .stroke()
            .font("12px system-ui")
            .text_align(CanvasTextAlign::Center)
            .text_baseline(CanvasTextBaseline::Middle)
            .fill_color(c.foreground)
            .fill_text(NAMES[i], x + NODE.0 / 2.0, y + NODE.1 / 2.0);
    }
    if let Some((from, to)) = marquee {
        let (x, y) = (from.0.min(to.0), from.1.min(to.1));
        let (w, h) = ((from.0 - to.0).abs(), (from.1 - to.1).abs());
        p = p
            .stroke_color(c.series[0])
            .line_width(1.0)
            .line_dash(&[px(4.0), px(4.0)])
            .begin_path()
            .rect(Bounds::new(
                point(px(x), px(y)),
                size(px(w.max(1.0)), px(h.max(1.0))),
            ))
            .stroke();
    }
    p
}

/// The node graph section.
#[component]
pub(crate) fn NodeGraph() -> impl IntoElement {
    let nodes = signal(START.to_vec());
    let selected = signal(Vec::<usize>::new());
    let marquee = signal(None::<((f32, f32), (f32, f32))>);
    let dragging = signal(Vec::<usize>::new());
    let last = signal((0.0_f32, 0.0_f32));
    let height = 280.0;
    let selection_text = computed(move || format!("{} selected", selected.get().len()));
    let first_text = computed(move || {
        nodes.with(|placed| {
            let (x, y) = placed[0];
            format!("first node {}, {}", x as i32, y as i32)
        })
    });
    let board = canvas(move || nodes.with(|placed| draw(placed, &selected.get(), marquee.get())))
        .h(px(height))
        .test_id("node-graph")
        .label("Node graph")
        .claim_touches(TouchClaim::on_start())
        .keep_touches(true)
        .on_responder_grant(move |e: &ResponderEvent| {
            let at = (e.location.x.0, e.location.y.0);
            let all = nodes.get_untracked();
            last.set(at);
            // A node press drags it and everything selected with it; an empty
            // press starts a marquee.
            if let Some(i) = node_at(at, &all) {
                if !selected.get_untracked().contains(&i) {
                    selected.set(vec![i]);
                }
                dragging.set(selected.get_untracked());
            } else {
                selected.set(Vec::new());
                marquee.set(Some((at, at)));
            }
        })
        .on_responder_move(move |e: &ResponderEvent| {
            let at = (e.location.x.0, e.location.y.0);
            let from = last.get();
            if from == at {
                return;
            }
            last.set(at);
            let (dx, dy) = (at.0 - from.0, at.1 - from.1);
            if !dragging.get_untracked().is_empty() {
                let moving = dragging.get_untracked();
                nodes.update(|all| {
                    for (i, node) in all.iter_mut().enumerate() {
                        if moving.contains(&i) {
                            node.0 += dx;
                            node.1 += dy;
                        }
                    }
                });
            } else if let Some((start, _)) = marquee.get_untracked() {
                let current = (start, at);
                marquee.set(Some(current));
                let all = nodes.get_untracked();
                selected.set(
                    (0..all.len())
                        .filter(|&i| inside_marquee(all[i], current))
                        .collect(),
                );
            }
        })
        .on_responder_release(move |_: &ResponderEvent| {
            dragging.set(Vec::new());
            marquee.set(None);
        })
        .on_responder_terminate(move |_: &ResponderEvent| {
            dragging.set(Vec::new());
            marquee.set(None);
        })
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Node graph",
        "drag a card to move it · drag the empty space to marquee-select",
        view()
            .gap_3()
            .child(board)
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(view().test_id("node-graph-reset").child(tool_button(
                        "Reset",
                        false,
                        move || {
                            batch(|| {
                                nodes.set(START.to_vec());
                                selected.set(Vec::new());
                                marquee.set(None);
                            });
                        },
                    ))),
            )
            .child(caption("node-graph-selection", move || {
                selection_text.get()
            }))
            .child(caption("node-graph-first", move || first_text.get())),
    ))
}
