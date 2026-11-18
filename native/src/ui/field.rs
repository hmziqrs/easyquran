//! Text fields: the app's input wrapper, with the frame, focus ring,
//! caret and placeholder colors, plus the labeled field layout.
//!
//! Two consumption paths: the `*_field` wrappers own a plain
//! `Signal<String>`; a form field from `nsh-form` binds over the `*_frame`
//! constructors instead — `form_field.bind(ui::field::text_field_frame("Email"))`
//! — and the `BindField` supplies the value, the edits and the blur while
//! the frame supplies the look.

use nsh_base::{input, theme};
use nsh_core::prelude::*;

/// One field of a form: its label above, the control, then the hint or
/// validation message under it.
pub fn field(
    label: impl IntoReactive<SharedString>,
    control: impl IntoElement,
) -> impl IntoElement {
    view()
        .flex_col()
        .gap_1p5()
        .child(crate::ui::text::label(label))
        .child(control)
}

/// The input frame (`input.svelte`): a pill on the surface with a
/// hairline, min height by target type (44 touch, 36 pointer), the focus
/// ring, the caret and placeholder from the theme.
fn frame(input: Input) -> Input {
    let touch = !is_desktop();
    input
        .min_h(move || px(if touch { 44.0 } else { 36.0 }))
        .px_4()
        .py_2()
        .text_sm()
        .text_color(move || theme().foreground)
        .rounded(crate::ui::tokens::radius_pill)
        .border_1()
        .border_color(move || theme().border)
        .bg(crate::ui::tokens::surface)
        .transition_colors()
        .focus(crate::ui::button::focus_ring)
        .disabled_style(|s| s.opacity(0.5))
        .placeholder_color(move || color(theme().muted_foreground))
        .caret_color(move || color(theme().foreground))
}

/// The multi-line frame (`textarea.svelte`): a 10-point block, not a pill.
fn block(input: Input) -> Input {
    frame(input).px_3().rounded(crate::ui::tokens::radius_md)
}

/// Marks a field invalid while `invalid` holds (`aria-invalid`): the
/// danger edge. Put the message next to the field ([`field_message`]).
pub fn invalid_when(input: Input, invalid: impl Fn() -> bool + 'static) -> Input {
    input.style_when(invalid, |s| s.border_color(crate::ui::tokens::danger))
}

/// The hint or validation message under a field: muted, or danger when
/// it reports an error.
pub fn field_message(
    message: impl IntoReactive<SharedString>,
    error: impl Fn() -> bool + 'static,
) -> impl IntoElement {
    text(message.into_reactive()).text_xs().text_color(move || {
        if error() {
            crate::ui::tokens::danger()
        } else {
            theme().muted_foreground
        }
    })
}

/// A single-line field bound to `value`; typing writes the signal.
pub fn text_field(value: Signal<String>, placeholder: impl Into<String>) -> Input {
    frame(
        input()
            .value(move || value.get())
            .on_change(move |text: &str| value.set(text.to_owned()))
            .placeholder(placeholder.into()),
    )
}

/// A password field: masked, no autocapitalization or correction.
pub fn password_field(value: Signal<String>, placeholder: impl Into<String>) -> Input {
    frame(
        input()
            .value(move || value.get())
            .on_change(move |text: &str| value.set(text.to_owned()))
            .placeholder(placeholder.into())
            .masked(true)
            .auto_capitalize(AutoCapitalize::None)
            .auto_correct(false),
    )
}

/// A multi-line field: it grows with its content from a 4-line floor.
pub fn multiline_field(value: Signal<String>, placeholder: impl Into<String>) -> Input {
    block(
        input()
            .value(move || value.get())
            .on_change(move |text: &str| value.set(text.to_owned()))
            .placeholder(placeholder.into())
            // 4 lines of 14pt text: the floor a note needs to read as one.
            .min_h(px(96.0))
            .multiline(true)
            .auto_capitalize(AutoCapitalize::Sentences),
    )
}

/// A numeric field: the digits keyboard, values kept as text.
pub fn numeric_field(value: Signal<String>, placeholder: impl Into<String>) -> Input {
    frame(
        input()
            .value(move || value.get())
            .on_change(move |text: &str| value.set(text.to_owned()))
            .placeholder(placeholder.into())
            .keyboard_type(KeyboardType::Number),
    )
}

/// The text field's frame without value wiring, for `nsh-form`: bind a
/// `BindField` over it and it supplies the value, the edits and the blur.
pub fn text_field_frame(placeholder: impl Into<String>) -> Input {
    frame(input().placeholder(placeholder.into()))
}

/// The masked frame for a password `BindField`: bind over it as over
/// [`text_field_frame`]; masking is not `bind`'s to set.
pub fn password_field_frame(placeholder: impl Into<String>) -> Input {
    frame(
        input()
            .placeholder(placeholder.into())
            .masked(true)
            .auto_capitalize(AutoCapitalize::None)
            .auto_correct(false),
    )
}

/// The multi-line frame, four lines tall at the floor: bind a
/// `BindField` over it as over [`text_field_frame`].
pub fn multiline_field_frame(placeholder: impl Into<String>) -> Input {
    block(
        input()
            .placeholder(placeholder.into())
            // 4 lines of 14pt text: the floor a note needs to read as one.
            .min_h(px(96.0))
            .multiline(true)
            .auto_capitalize(AutoCapitalize::Sentences),
    )
}
