//! The spring and tween demo: one knob driven across a track, with the
//! spring's stiffness and damping and the tween's duration picked from the
//! tab bar, and its settled position read back from the UI thread.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui;

use super::{api_line, section, status_line};

/// The track the knob travels: its travel is the width less the knob.
const TRACK: f32 = 240.0;
const KNOB: f32 = 32.0;

/// A spring of stiffness `k` and damping `c`, mass 1.
const fn spring(k: f32, c: f32) -> Spring {
    Spring {
        stiffness: k,
        damping: c,
        mass: 1.0,
        velocity: None,
        frame_rate: None,
    }
}

/// The spring presets, stiffness and damping by index.
const SPRING_SETTINGS: [(f32, f32); 3] = [(120.0, 14.0), (180.0, 8.0), (400.0, 30.0)];
const SPRING_LABELS: &[&str] = &["Gentle", "Wobbly", "Stiff"];

/// The tween durations, in milliseconds, by index.
const TWEEN_SETTINGS: [f32; 2] = [250.0, 700.0];
const TWEEN_LABELS: &[&str] = &["250 ms", "700 ms"];

/// The drivers, by index: 0 springs, 1 tweens.
const KIND_LABELS: &[&str] = &["Spring", "Tween"];

/// The knob: a spring or a tween with the picked settings carries it
/// between the track's ends; reduced motion sets it at once.
pub(crate) fn spring_tween(calm: Signal<bool>) -> impl IntoElement {
    let (preset, tween, at_start, kind) = (
        signal(0_usize),
        signal(0_usize),
        signal(true),
        signal(0_usize),
    );
    // What happened last, from the animation's own end: the settled
    // position is read on the UI thread, where the value moved.
    let status = signal(String::from("Idle at 0"));
    let x = animated_value(0.0);
    // The run going on: a report of a replaced run must not overwrite the
    // newer one's.
    let run = stored(0_u32);
    let go = move || {
        let to = if at_start.get_untracked() { TRACK } else { 0.0 };
        at_start.update(|s| *s = !*s);
        run.with(|r| *r += 1);
        let mine = run.get();
        // Springs for instant states is the whole reduced-motion contract.
        if calm.get_untracked() || reduce_motion() {
            x.set(to);
            status.set(format!("Instant {}", to.round()));
            return;
        }
        let end = if kind.get_untracked() == 1 {
            x.timing(
                to,
                TWEEN_SETTINGS[tween.get_untracked()],
                Easing::Curve(Curve::Cubic, EaseMode::InOut),
            )
        } else {
            let (k, c) = SPRING_SETTINGS[preset.get_untracked()];
            x.spring(to, spring(k, c))
        };
        let read = x;
        spawn(async move {
            let finished = end.await;
            if run.get() != mine {
                return;
            }
            let value = read.value().await;
            status.set(format!(
                "Rest {} · {}",
                value.round(),
                if finished { "finished" } else { "interrupted" }
            ));
        });
    };
    section(
        "Spring and tween",
        "One value drives the knob; the UI thread does every frame",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(
                view()
                    .w(px(TRACK))
                    .h(px(KNOB))
                    .rounded(px(KNOB / 2.0))
                    .bg(move || theme().muted)
                    .child(
                        view()
                            .absolute()
                            .left(px(0.0))
                            .top(px(0.0))
                            .size(px(KNOB))
                            .rounded(px(KNOB / 2.0))
                            .bg(move || theme().primary)
                            .bind_translate_x(&x.mul((TRACK - KNOB) / TRACK)),
                    ),
            )
            .child(ui::tabs::tab_bar(
                move || preset.get(),
                SPRING_LABELS,
                move |at: usize| preset.set(at),
            ))
            .child(ui::tabs::tab_bar(
                move || tween.get(),
                TWEEN_LABELS,
                move |at: usize| tween.set(at),
            ))
            .child(ui::tabs::tab_bar(
                move || kind.get(),
                KIND_LABELS,
                move |at: usize| kind.set(at),
            ))
            .child(ui::button::button(
                move || {
                    if at_start.get() {
                        "Play to 240"
                    } else {
                        "Play to 0"
                    }
                },
                go,
            ))
            .child(status_line("spring-status", move || status.get()))
            .child(api_line(
                "A value, its end heard",
                "AnimatedValue::spring / timing",
            )),
    )
}
