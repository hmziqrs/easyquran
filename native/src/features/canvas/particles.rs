//! The particle field: dots with velocity bouncing inside the canvas
//! bounds, driven by the same engine-clock interval as the frame loop —
//! the port of the showcase's particle demo. The field seeds itself the
//! same way on every mount, so a reset restores the opening frame.

use std::time::Duration;

use nsh_base::theme;
use nsh_core::{Bounds, Points, prelude::*};

use super::{Palette, caption, section, tool_button};

/// The dots on the field, and the interval's period (about 60 fps).
const DOTS: usize = 48;
const PERIOD: u64 = 16;

/// One bouncing dot: position and velocity in points, its radius and the
/// series tint it paints with.
#[derive(Clone, Copy, PartialEq)]
struct Dot {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    series: usize,
}

/// A tiny LCG, so the field is identical on every seed.
fn seeded_dots() -> Vec<Dot> {
    let mut state = 0x9E37_79B9_u32;
    let mut next = |modulo: u32| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) % modulo
    };
    (0..DOTS)
        .map(|i| Dot {
            x: 20.0 + next(300) as f32,
            y: 16.0 + next(140) as f32,
            vx: (0.4 + next(30) as f32 * 0.05) * if next(2) == 0 { 1.0 } else { -1.0 },
            vy: (0.4 + next(30) as f32 * 0.05) * if next(2) == 0 { 1.0 } else { -1.0 },
            r: 3.0 + next(5) as f32,
            series: i % 3,
        })
        .collect()
}

/// One step: each dot advances by its velocity and bounces off the
/// walls. `width` is 0 before the layout, so the field starts closed.
fn tick_field(dots: &mut [Dot], width: f32, height: f32) {
    for d in dots {
        d.x += d.vx;
        d.y += d.vy;
        if d.x < d.r && d.vx < 0.0 {
            d.x = d.r;
            d.vx = -d.vx;
        }
        let right = (width - d.r).max(d.r);
        if d.x > right && d.vx > 0.0 {
            d.x = right;
            d.vx = -d.vx;
        }
        if d.y < d.r && d.vy < 0.0 {
            d.y = d.r;
            d.vy = -d.vy;
        }
        let bottom = (height - d.r).max(d.r);
        if d.y > bottom && d.vy > 0.0 {
            d.y = bottom;
            d.vy = -d.vy;
        }
    }
}

/// The field at one tick.
fn draw(dots: &[Dot]) -> Painter {
    let c = Palette::now();
    let mut p = Painter::new();
    for d in dots {
        p = p
            .fill_color(c.series[d.series % c.series.len()].opacity(0.85))
            .begin_path()
            .arc(d.x, d.y, d.r, Radians(0.0), Radians(std::f32::consts::TAU))
            .close_path()
            .fill();
    }
    p
}

/// The particle field: play holds the interval, reset re-seeds the field.
#[component]
pub(crate) fn ParticleField() -> impl IntoElement {
    let dots = signal(seeded_dots());
    let playing = signal(true);
    let tick = signal(0_u32);
    let canvas_width = signal(0.0_f32);
    let height = 180.0;
    // The interval is owned by the effect, like the frame loop's: a
    // toggle stops and starts it, and leaving the component disposes both.
    effect(move || {
        if !playing.get() {
            return;
        }
        set_interval(Duration::from_millis(PERIOD), move || {
            tick.update(|t| *t += 1);
            dots.update(|field| tick_field(field, canvas_width.get(), height));
        });
    });
    let readout = move || format!("{} dots · tick {}", dots.with(Vec::len), tick.get());
    let field = canvas(move || dots.with(|field| draw(field)))
        .h(px(height))
        .test_id("particle-field")
        .label("Particle field")
        .on_layout(move |frame: &Bounds<Points>| canvas_width.set(frame.size.width.0))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Particle field",
        "canvas redrawn from a signal · bouncing dots at 60 ticks a second",
        view()
            .gap_3()
            .child(field)
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(view().test_id("particles-toggle").child(tool_button(
                        move || {
                            if playing.get() { "Pause" } else { "Play" }
                        },
                        false,
                        move || playing.update(|p| *p = !*p),
                    )))
                    .child(view().test_id("particles-reset").child(tool_button(
                        "Reset",
                        false,
                        move || {
                            batch(|| {
                                dots.set(seeded_dots());
                                tick.set(0);
                            });
                        },
                    ))),
            )
            .child(caption("particles-readout", readout)),
    ))
}
