//! The animated canvas: a frame loop whose state is a signal, so each
//! frame's content is computed in app code (the tier the animation guide
//! reserves for simulations — pure motion would use animated values). The
//! interval follows the engine's clock, so a manual clock in tests
//! advances it without real time.

use std::time::Duration;

use nsh_base::theme;
use nsh_core::{Bounds, Points, prelude::*};

use super::{Palette, caption, section, tool_button};

/// The bars of the sweep, and the interval's period (about 30 Hz).
const BARS: usize = 48;
const PERIOD: u64 = 33;

/// Bar `i` of `tick`: a height in 0..1 swept by two out-of-phase waves,
/// so the content changes on every frame and no native transition can
/// describe it ahead of time.
fn bar_height(i: usize, tick: u32) -> f32 {
    let t = tick as f32 * 0.09;
    let a = (i as f32 * 0.42 + t).sin();
    let b = (i as f32 * 0.13 - t * 0.62).sin();
    0.18 + 0.72 * (0.55 * a + 0.45 * b + 1.0) / 2.0
}

/// The sweep at `tick`: a bar per column, heights from `height`.
fn draw(width: f32, height: f32, tick: u32) -> Painter {
    let c = Palette::now();
    let step = width / BARS as f32;
    let bar_w = (step * 0.62).max(1.0);
    let mut p = Painter::new();
    for i in 0..BARS {
        let h = bar_height(i, tick) * (height - 6.0);
        p = p
            .fill_color(c.series[0].opacity(0.35 + 0.65 * bar_height(i, tick)))
            .begin_path()
            .round_rect(
                Bounds::new(
                    point(px(i as f32 * step + (step - bar_w) / 2.0), px(height - h)),
                    size(px(bar_w), px(h)),
                ),
                px(bar_w / 2.0),
            )
            .fill();
    }
    p
}

/// The frame loop: play holds the interval, reset zeroes the tick.
#[component]
pub(crate) fn FrameLoop() -> impl IntoElement {
    let tick = signal(0_u32);
    let playing = signal(true);
    let canvas_width = signal(0.0_f32);
    let height = 140.0;
    // The interval is owned by the effect: toggling play stops and starts
    // it, and leaving the component disposes both.
    effect(move || {
        if !playing.get() {
            return;
        }
        set_interval(Duration::from_millis(PERIOD), move || {
            tick.update(|t| *t += 1);
        });
    });
    let readout = move || format!("tick {}", tick.get());
    let canvas = canvas(move || draw(canvas_width.get(), height, tick.get()))
        .h(px(height))
        .test_id("sweep")
        .label("Animated sweep")
        .on_layout(move |frame: &Bounds<Points>| canvas_width.set(frame.size.width.0))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Frame loop",
        "canvas redrawn from a signal · 30 ticks a second",
        view()
            .gap_3()
            .child(canvas)
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(
                        // The id rides a view around the button; tests press
                        // a scoped selector (`id:sim-toggle >> text:Pause`),
                        // for the press walks up from the text it matches.
                        view().test_id("sim-toggle").child(tool_button(
                            move || {
                                if playing.get() { "Pause" } else { "Play" }
                            },
                            false,
                            move || playing.update(|p| *p = !*p),
                        )),
                    )
                    .child(view().test_id("frame-loop-reset").child(tool_button(
                        "Reset",
                        false,
                        move || tick.set(0),
                    ))),
            )
            .child(caption("tick-count", readout)),
    ))
}
