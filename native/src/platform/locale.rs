//! The app's UI locale: `rust-i18n`'s global locale mirrored into a signal,
//! because a component runs once and only bindings that read a changed signal
//! run again. Interim: the i18n foundation (PARITY O1, O2) replaces
//! `rust-i18n` with `eq_core::i18n` catalogues baked from `web/messages/**`.

use nsh_core::prelude::*;

/// The locale code the pages show (`en`, `ar`), empty until first set
/// (`t!` then answers in the `i18n!` default, `en`).
pub fn app_locale() -> Signal<String> {
    global::<String>()
}

/// Switches the UI language: `rust-i18n`'s global locale and the signal the
/// string bindings read move together.
pub fn set_app_locale(code: impl Into<String>) {
    let code = code.into();
    rust_i18n::set_locale(&code);
    global::<String>().set(code);
}

/// The locales the app ships, with their names as read in that language.
pub const CHOICES: [(&str, &str); 2] = [("en", "English"), ("ar", "العربية")];
