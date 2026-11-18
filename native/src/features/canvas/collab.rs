//! The shared board: cards that any peer may move, with demo peers
//! standing in for a session — the port of the multiuser canvas page. A
//! real session needs a server; the gpui starter's page itself ships an
//! offline demo-peers mode, and this mirrors that: two simulated cursors
//! orbit the board on the engine clock, and one of them nudges its card
//! every few ticks, while the local user drags cards the same way.

use std::time::Duration;

use nsh_base::theme;
use nsh_core::{
    Bounds, Points,
    canvas::{CanvasTextAlign, CanvasTextBaseline},
    prelude::*,
};

use super::{Palette, caption, section};

/// A card's size, in canvas points.
const CARD: (f32, f32) = (88.0, 36.0);

/// The shared cards: a label and a home position. Plan straddles the
/// canvas center, which is where the view tests' drags press.
const CARDS: [(&str, f32, f32); 3] = [
    ("Plan", 116.0, 120.0),
    ("Draft", 30.0, 40.0),
    ("Ship", 200.0, 200.0),
];

/// The demo peers: the orbit's radius and period, in ticks.
const PEERS: usize = 2;
const ORBIT_PERIOD: f32 = 240.0;

/// Peer `peer`'s cursor at `tick`, orbiting the canvas inside its bounds.
fn peer_at(peer: usize, tick: u32, width: f32, height: f32) -> (f32, f32) {
    let t = tick as f32 / ORBIT_PERIOD * std::f32::consts::TAU + peer as f32 * 2.1;
    let (rx, ry) = (width.max(120.0) / 2.0 - 24.0, height.max(80.0) / 2.0 - 24.0);
    (
        width / 2.0 + rx * (t + peer as f32).cos(),
        height / 2.0 + ry * (t * 1.3).sin(),
    )
}

/// Peer 0 nudges its card along a slow sine, so the board visibly moves
/// without anyone touching it.
fn peer_drift(tick: u32) -> f32 {
    (tick as f32 / 60.0).sin() * 10.0
}

/// The board: the cards (Draft carries the peer's nudge), then the peers'
/// cursors as tinted rings with their names.
fn draw(
    cards: &[(f32, f32)],
    selected: Option<usize>,
    tick: u32,
    width: f32,
    height: f32,
) -> Painter {
    let c = Palette::now();
    let mut p = Painter::new();
    for (i, (name, _, _)) in CARDS.iter().enumerate() {
        let (mut x, y) = cards[i];
        if i == 1 {
            x += peer_drift(tick);
        }
        let bounds = Bounds::new(point(px(x), px(y)), size(px(CARD.0), px(CARD.1)));
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
            .round_rect(bounds, px(8.0))
            .fill()
            .stroke()
            .font("12px system-ui")
            .text_align(CanvasTextAlign::Center)
            .text_baseline(CanvasTextBaseline::Middle)
            .fill_color(c.foreground)
            .fill_text(name, x + CARD.0 / 2.0, y + CARD.1 / 2.0);
    }
    for peer in 0..PEERS {
        let (x, y) = peer_at(peer, tick, width, height);
        p = p
            .stroke_color(c.series[peer % c.series.len()])
            .line_width(2.0)
            .begin_path()
            .arc(x, y, 7.0, Radians(0.0), Radians(std::f32::consts::TAU))
            .close_path()
            .stroke()
            .font("10px system-ui")
            .fill_color(c.series[peer % c.series.len()])
            .text_align(CanvasTextAlign::Left)
            .text_baseline(CanvasTextBaseline::Top)
            .fill_text(&format!("peer {}", peer + 1), x + 10.0, y - 4.0);
    }
    p
}

/// The shared board section: simulated peers, no server.
#[component]
pub(crate) fn SharedBoard() -> impl IntoElement {
    let cards = signal(CARDS.map(|(_, x, y)| (x, y)).to_vec());
    let selected = signal(None::<usize>);
    let dragging = signal(false);
    let last = signal((0.0_f32, 0.0_f32));
    let tick = signal(0_u32);
    let size = signal((0.0_f32, 0.0_f32));
    let height = 280.0;
    // The peers ride the same interval as the other animated sections.
    effect(move || {
        set_interval(Duration::from_millis(33), move || {
            tick.update(|t| *t += 1);
        });
    });
    let card_at = move |world: (f32, f32)| -> Option<usize> {
        cards.with(|placed| {
            (0..placed.len()).rev().find(|&i| {
                let (mut x, y) = placed[i];
                if i == 1 {
                    x += peer_drift(tick.get_untracked());
                }
                world.0 >= x && world.0 <= x + CARD.0 && world.1 >= y && world.1 <= y + CARD.1
            })
        })
    };
    let board = canvas(move || {
        let (w, h) = size.get();
        cards.with(|placed| draw(placed, selected.get(), tick.get(), w, h))
    })
    .h(px(height))
    .test_id("collab-board")
    .label("Shared board")
    .claim_touches(TouchClaim::on_start())
    .keep_touches(true)
    .on_responder_grant(move |e: &ResponderEvent| {
        let at = (e.location.x.0, e.location.y.0);
        last.set(at);
        if let Some(i) = card_at(at) {
            batch(|| {
                selected.set(Some(i));
                dragging.set(true);
            });
        } else {
            selected.set(None);
        }
    })
    .on_responder_move(move |e: &ResponderEvent| {
        if !dragging.get_untracked() {
            return;
        }
        let at = (e.location.x.0, e.location.y.0);
        let from = last.get();
        if from == at {
            return;
        }
        last.set(at);
        if let Some(i) = selected.get_untracked() {
            let (dx, dy) = (at.0 - from.0, at.1 - from.1);
            cards.update(|all| {
                if let Some(card) = all.get_mut(i) {
                    card.0 += dx;
                    card.1 += dy;
                }
            });
        }
    })
    .on_responder_release(move |_: &ResponderEvent| dragging.set(false))
    .on_responder_terminate(move |_: &ResponderEvent| dragging.set(false))
    .on_layout(move |frame: &Bounds<Points>| size.set((frame.size.width.0, frame.size.height.0)))
    .rounded(move || theme().radius)
    .border_1()
    .border_color(move || theme().border)
    .bg(move || theme().muted);
    view().child(section(
        "Shared board",
        "demo peers · simulated session, no server",
        view()
            .gap_3()
            .child(board)
            .child(caption("collab-note", move || {
                format!(
                    "{} demo peers orbit the board; peer 1 nudges its card · tick {}",
                    PEERS,
                    tick.get()
                )
            }))
            .child(caption("collab-selection", move || {
                format!(
                    "selected: {}",
                    selected
                        .get()
                        .map_or("none".to_owned(), |i| CARDS[i].0.to_owned())
                )
            })),
    ))
}
