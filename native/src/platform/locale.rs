//! The system's language preferences, as NSH reports them. The app's UI
//! locale itself is `stores::locale`.

use eq_core::i18n::{Locale, locale_from_languages};
use nsh_core::prelude::*;

/// The user's preferred languages, most preferred first, as BCP 47 tags
/// (`ar-SA`). Reading it subscribes: what reads it follows the system
/// settings. Must run where components run.
///
/// Once the app has set its own locale (`nsh_core::i18n::set_locale`), NSH
/// moves that locale to the front of this list; `stores::locale` keeps the
/// list it read before overriding.
pub fn system_languages() -> Vec<String> {
    locale_settings()
        .languages
        .iter()
        .map(|l| l.as_str().to_owned())
        .collect()
}

/// The first supported UI locale among [`system_languages`].
pub fn system_locale() -> Option<Locale> {
    let languages = system_languages();
    locale_from_languages(languages.iter().map(String::as_str))
}
