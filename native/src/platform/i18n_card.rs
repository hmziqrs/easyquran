//! The language card: switches the UI locale so every string on these pages
//! re-renders, and the RTL sanity demo — the same row laid out both ways,
//! `direction` mirroring the row, not the text engine.

use nsh_base::{button, theme};
use nsh_core::prelude::*;

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::{
    locale::{self, CHOICES},
    widgets::{hint, localized, panel, rule},
};

/// One language choice: its own name (a language's name does not translate),
/// selected when it is the current one.
fn language_button(code: &'static str, name: &'static str, current: Signal<String>) -> AnyElement {
    button()
        .min_h(px(44.0))
        .px_4()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(move || theme().radius)
        .bg(move || theme().secondary)
        .selected_style(|s| s.bg(move || theme().primary))
        .selected(move || current.get() == code)
        .on_press(move || locale::set_app_locale(code))
        .child(text(name).text_sm().text_color(move || {
            if current.get() == code {
                theme().primary_foreground
            } else {
                theme().secondary_foreground
            }
        }))
        .into_element()
}

/// One item of the direction demo row.
fn direction_item(key: &'static str) -> AnyElement {
    text(localized(key))
        .text_sm()
        .px_3()
        .py_1()
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .into_element()
}

/// The same three items laid out in `direction`: the RTL row mirrors the LTR
/// one while the strings stay theirs.
fn direction_row(direction: LayoutDirection, label: &'static str) -> AnyElement {
    view()
        .gap_1()
        .child(
            text(localized(label))
                .text_size(px(12.0))
                .text_color(move || theme().muted_foreground),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .direction(direction)
                .child(direction_item("platform.rtl.first"))
                .child(direction_item("platform.rtl.second"))
                .child(direction_item("platform.rtl.third")),
        )
        .into_element()
}

#[component]
fn LanguageCard() -> impl IntoElement {
    let current = locale::app_locale();
    panel(
        "platform.i18n.title",
        [
            hint("platform.i18n.hint"),
            view()
                .flex_row()
                .gap_2()
                .children(CHOICES.map(|(code, name)| language_button(code, name, current)))
                .into_element(),
            text(move || {
                locale::app_locale().get();
                let code = current.get();
                let code = if code.is_empty() { "en" } else { code.as_str() };
                format!("{}: {}", t!("platform.i18n.current"), code)
            })
            .text_sm()
            .tabular_nums()
            .test_id("platform-locale-code")
            .into_element(),
            rule(),
            direction_row(LayoutDirection::Ltr, "platform.rtl.ltr"),
            direction_row(LayoutDirection::Rtl, "platform.rtl.rtl"),
        ],
    )
}

/// The language card, as the platform page places it.
pub(crate) fn card() -> AnyElement {
    view().child(language_card()).into_element()
}
