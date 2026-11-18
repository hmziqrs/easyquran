//! Numbered and coded entry: the number input with its steppers, and the
//! one-time code with its cells and caret.

use nsh_base::{number_input, otp_input, theme};
use nsh_core::prelude::*;

/// A number field with increment and decrement buttons beside it; the
/// value is `None` while empty.
pub fn number_field(
    name: &'static str,
    value: Signal<Option<f64>>,
    placeholder: impl Into<String>,
) -> impl IntoElement {
    let placeholder = SharedString::from(placeholder.into());
    number_input()
        .accessibility_label(name)
        .value(move || value.get())
        .on_change(move |next: &Option<f64>| value.set(*next))
        .placeholder(move || placeholder.clone())
        .min(0.0)
        .max(1_000_000.0)
        .step(1.0)
        .input_style(|s| {
            s.h(px(36.0))
                .px_3()
                .text_sm()
                .text_color(move || theme().foreground)
        })
        .controls_style(|s| s.flex_row().gap_1())
        .decrement_button_class(
            "size-8 items-center justify-center rounded-md text-sm text-foreground \
             hover:bg-accent active:bg-accent disabled:opacity-50",
        )
        .decrement_button(text("−").text_sm().text_color(move || theme().foreground))
        .increment_button_class(
            "size-8 items-center justify-center rounded-md text-sm text-foreground \
             hover:bg-accent active:bg-accent disabled:opacity-50",
        )
        .increment_button(text("+").text_sm().text_color(move || theme().foreground))
}

/// The one-time code: `length` cells grouped by `groups`, its caret
/// styled; the code is reported as text.
pub fn otp_field(
    name: &'static str,
    value: Signal<SharedString>,
    length: u32,
    groups: u32,
) -> impl IntoElement {
    otp_input()
        .accessibility_label(name)
        .length(length)
        .groups(groups)
        .value(move || Some(value.get()))
        .on_change(move |next: &SharedString| value.set(next.clone()))
        .group_class("flex-row gap-2")
        .cell_class(
            "size-10 items-center justify-center rounded-md border border-input bg-background \
             text-base text-foreground text-center focus:border-ring focus:outline-2 \
             focus:outline-ring disabled:opacity-50",
        )
        .caret_class("w-0.5 h-5 rounded-full bg-foreground")
}
