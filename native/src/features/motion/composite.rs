//! The composites demo: steps grouped as sequence, stagger and a counted
//! loop, plus a keyframe run — all described once and played on the UI
//! thread, each step starting at the exact end of the one before.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui;

use super::{api_line, section, status_line};

/// The staggered dots and the knob travel this far.
const TRAVEL: f32 = 36.0;

/// A dot of the stagger, rising into place by `y`.
fn dot(y: &AnimatedValue, color: AdaptiveColor) -> impl IntoElement {
    view()
        .size(px(20.0))
        .rounded(px(10.0))
        .bg(color)
        .bind_translate_y(y)
}

pub(crate) fn composites() -> impl IntoElement {
    // The stagger: four dots rise one after another (90 ms apart).
    let ys = [0.0; 4].map(animated_value);
    // The sequence: a knob jumps, then springs home as the second step.
    let knob = animated_value(0.0);
    // The keyframes: a badge's opacity pulses down and back, twice.
    let pulse = animated_value(1.0);
    let status = signal(String::from("Idle"));
    let stagger = Animation::stagger(
        90.0,
        ys.iter().map(|y| {
            Animation::timing(
                y,
                0.0,
                Timing::new(380.0, Easing::Curve(Curve::Quad, EaseMode::Out)),
            )
        }),
    );
    let jump = Animation::sequence([
        Animation::timing(&knob, TRAVEL * 4.0, Timing::new(320.0, Easing::EaseOut)),
        Animation::spring(&knob, 0.0, Spring::default()),
    ]);
    let blink = Animation::keyframes(
        &pulse,
        [Keyframe::new(1.0), Keyframe::new(0.25), Keyframe::new(1.0)],
        KeyframeTiming::new(700.0).iterations(Some(2)),
    );
    let run_stagger = move || {
        // Each dot starts away from rest, so the stagger is visible.
        for y in &ys {
            y.set(TRAVEL);
        }
        let end = stagger.start();
        spawn(async move {
            let finished = end.await;
            status.set(format!(
                "Stagger {}",
                if finished { "finished" } else { "interrupted" }
            ));
        });
    };
    let run_jump = move || {
        let end = jump.start();
        spawn(async move {
            let finished = end.await;
            status.set(format!(
                "Sequence {}",
                if finished { "finished" } else { "interrupted" }
            ));
        });
    };
    let run_blink = move || {
        let end = blink.start();
        spawn(async move {
            let finished = end.await;
            status.set(format!(
                "Keyframes {}",
                if finished { "finished" } else { "interrupted" }
            ));
        });
    };
    section(
        "Composites",
        "Groups of steps play as one, each step starting as the last ends",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(
                view()
                    .flex_row()
                    .gap(px(14.0))
                    .items_end()
                    .h(px(64.0))
                    .children(
                        ys.iter()
                            .zip([
                                theme().chart_1,
                                theme().chart_2,
                                theme().chart_3,
                                theme().chart_4,
                            ])
                            .map(|(y, color)| dot(y, color)),
                    ),
            )
            .child(
                view()
                    .w(px(TRAVEL * 4.0 + 24.0))
                    .h(px(20.0))
                    .rounded(px(10.0))
                    .bg(move || theme().muted)
                    .child(
                        view()
                            .absolute()
                            .left(px(2.0))
                            .top(px(2.0))
                            .size(px(16.0))
                            .rounded(px(8.0))
                            .bg(move || theme().primary)
                            .bind_translate_x(&knob),
                    ),
            )
            .child(
                view()
                    .size(px(28.0))
                    .rounded(px(14.0))
                    .items_center()
                    .justify_center()
                    .bg(move || theme().primary)
                    .bind_opacity(&pulse)
                    .child(
                        text("3")
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(move || theme().primary_foreground),
                    ),
            )
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(8.0))
                    .child(ui::button::button_with(
                        "Stagger",
                        ui::button::ButtonVariant::Secondary,
                        ui::button::ButtonSize::Small,
                        false,
                        Some("Stagger"),
                        run_stagger,
                    ))
                    .child(ui::button::button_with(
                        "Sequence",
                        ui::button::ButtonVariant::Secondary,
                        ui::button::ButtonSize::Small,
                        false,
                        Some("Sequence"),
                        run_jump,
                    ))
                    .child(ui::button::button_with(
                        "Pulse ×2",
                        ui::button::ButtonVariant::Secondary,
                        ui::button::ButtonSize::Small,
                        false,
                        Some("Pulse twice"),
                        run_blink,
                    )),
            )
            .child(status_line("composite-status", move || status.get()))
            .child(api_line(
                "Steps as data, run as one",
                "Animation::sequence / stagger / keyframes",
            )),
    )
}
