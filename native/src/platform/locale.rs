//! The app's UI locale: `rust-i18n`'s global locale mirrored into a signal,
//! because a component runs once and only bindings that read a changed signal
//! run again. Strings render through `widgets::localized`, which reads this
//! signal and looks `t!` up on each run.

use nsh_core::prelude::*;

/// The locale code the pages show (`en`, `zh-CN`), empty until first set
/// (`t!` then answers in the `i18n!` default, `en`).
pub(crate) fn app_locale() -> Signal<String> {
    global::<String>()
}

/// Switches the UI language: `rust-i18n`'s global locale and the signal the
/// string bindings read move together.
pub fn set_app_locale(code: impl Into<String>) {
    let code = code.into();
    rust_i18n::set_locale(&code);
    global::<String>().set(code);
}

/// The locales the language card offers, with their names as read in that
/// language (a language's own name does not translate).
pub(crate) const CHOICES: [(&str, &str); 2] = [("en", "English"), ("zh-CN", "中文")];
