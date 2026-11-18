//! The transform demo: pan, pinch and rotation recognizers on one card,
//! all writing their values on the UI thread, with a reset that springs
//! them home. Pinch and rotation want two fingers, so they are for a
//! device; the drag and its readout work headless too.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui;

use super::{api_line, section, status_line};

/// The card, square.
const SIZE: f32 = 160.0;

pub(crate) fn pinch_rotate() -> impl IntoElement {
    let values = [0.0, 0.0, 1.0, 0.0].map(animated_value);
    let [x, y, scale, angle] = values;
    // The last gesture's amounts, for the readout.
    let (drag, pinched, turned) = (signal(0.0_f32), signal(1.0_f32), signal(0.0_f32));
    let reset = move || {
        let _ = x.spring(0.0, Spring::default());
        let _ = y.spring(0.0, Spring::default());
        let _ = scale.spring(1.0, Spring::default());
        let _ = angle.spring(0.0, Spring::default());
        batch(|| {
            drag.set(0.0);
            pinched.set(1.0);
            turned.set(0.0);
        });
    };
    section(
        "Pan, pinch and rotation",
        "Three recognizers on one card, driven natively",
        view()
            .flex_col()
            .gap(px(12.0))
            .child(
                view()
                    .h(px(SIZE + 24.0))
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded(move || theme().radius)
                    .bg(move || theme().muted)
                    .child(
                        view()
                            .test_id("transform-card")
                            .size(px(SIZE))
                            .rounded(px(18.0))
                            .border_1()
                            .border_color(move || theme().border)
                            .overflow_hidden()
                            .bg(move || theme().primary)
                            .pan_values(Some(&x), Some(&y))
                            .pinch_value(&scale)
                            .rotation_value(&angle)
                            .on_pan_end(move |e: &PanEvent| {
                                drag.set(e.translation.x.0.hypot(e.translation.y.0).round());
                            })
                            .on_pinch_end(move |e: &GestureEvent| pinched.set(e.value))
                            .on_rotation_end(move |e: &GestureEvent<Radians>| {
                                turned.set(e.value.0.to_degrees().round())
                            })
                            .bind_translate_x(&x)
                            .bind_translate_y(&y)
                            .bind_scale(&scale)
                            .bind_rotate(&angle)
                            .items_center()
                            .justify_center()
                            .child(
                                text("Drag me")
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(move || theme().primary_foreground),
                            ),
                    ),
            )
            .child(status_line("transform-status", move || {
                format!(
                    "Drag {} pt · scale {:.2}× · angle {}°",
                    drag.get(),
                    pinched.get(),
                    turned.get()
                )
            }))
            .child(ui::button::button_with(
                "Reset",
                ui::button::ButtonVariant::Secondary,
                ui::button::ButtonSize::Medium,
                false,
                Some("Reset"),
                reset,
            ))
            .child(api_line(
                "Pinch and rotate run beside the pan",
                "pan_values + pinch_value + rotation_value",
            )),
    )
}
