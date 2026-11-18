//! Presentational helpers private to the platform pages. They style `nsh/base`
//! and core elements with theme tokens only; when `src/ui` grows the matching
//! wrappers (`Button`, `Card`, `Field`), these hand the job over.

use nsh_base::{button, input, theme};
use nsh_core::prelude::*;

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::locale;

/// `t!` looked up on every run, so a locale switch re-renders it: reading the
/// locale signal subscribes the binding, and `t!` answers in that language.
pub(crate) fn localized(key: &'static str) -> impl Fn() -> String {
    move || {
        locale::app_locale().get();
        t!(key).to_string()
    }
}

/// A hairline between rows of a card.
pub(crate) fn rule() -> AnyElement {
    view().h(px(1.0)).bg(move || theme().border).into_element()
}

/// A section card: a titled surface the platform pages compose. The title is
/// a header role so screen readers can jump between cards. Named `panel` so
/// each card file can export its own `card`.
pub(crate) fn panel(
    title: &'static str,
    content: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    view()
        .gap_2()
        .p_4()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .text_color(move || theme().card_foreground)
        .child(
            text(localized(title))
                .text_size(px(15.0))
                .font_weight(FontWeight::MEDIUM)
                .role(AccessibilityRole::Header),
        )
        .children(content)
        .into_element()
}

/// A label / value row of the device page.
pub(crate) fn info_row(label: &'static str, value: impl Fn() -> String + 'static) -> AnyElement {
    view()
        .flex_row()
        .justify_between()
        .gap_3()
        .min_w(px(0.0))
        .child(
            text(localized(label))
                .flex_1()
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
        .child(text(value).text_sm().tabular_nums().text_right())
        .into_element()
}

/// A small line of help under a control.
pub(crate) fn hint(key: &'static str) -> AnyElement {
    text(localized(key))
        .text_size(px(12.0))
        .text_color(move || theme().muted_foreground)
        .into_element()
}

/// A filled action button; its label is read live, so it translates. `id`
/// steadies tests and screen readers against the translating label.
pub(crate) fn action_button(
    id: &'static str,
    label: impl Fn() -> String + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button()
        .min_h(px(44.0))
        .px_4()
        .flex_row()
        .items_center()
        .justify_center()
        .self_start()
        .rounded(move || theme().radius)
        .bg(move || theme().primary)
        .hover(|s| s.opacity(0.9))
        .active(|s| s.opacity(0.85))
        .on_press(on_press)
        .child(
            // The id rides the label: a press at the text hits the button.
            text(label)
                .test_id(id)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().primary_foreground),
        )
}

/// A quiet action button for secondary commands.
pub(crate) fn quiet_button(
    id: &'static str,
    label: impl Fn() -> String + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button()
        .min_h(px(44.0))
        .px_4()
        .flex_row()
        .items_center()
        .justify_center()
        .self_start()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().input)
        .bg(move || theme().background)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.opacity(0.85))
        .on_press(on_press)
        .child(
            text(label)
                .test_id(id)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().foreground),
        )
}

/// The one-line text field the platform pages use. `src/ui`'s `Field` wrapper
/// replaces this; the styling here is the placeholder.
pub(crate) fn field(
    id: &'static str,
    placeholder: &'static str,
    value: Signal<String>,
) -> impl IntoElement {
    input()
        .test_id(id)
        .label(localized(placeholder))
        .value(move || value.get())
        .on_change(move |text: &str| value.set(text.to_owned()))
        .h(px(40.0))
        .px_3()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().input)
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .placeholder(localized(placeholder))
        .placeholder_color(move || color(theme().muted_foreground))
}
