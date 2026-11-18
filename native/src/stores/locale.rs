//! The UI locale (PARITY O1, O2): which catalogue the app's text reads,
//! and its layout direction.
//!
//! - The current locale is the user's explicit choice, stored under
//!   `easyquran.locale` (`eq_core::i18n::LOCALE_PREF_KEY`) in NSH
//!   `Storage`; without one, the system's first supported language
//!   (`ar-SA` -> Arabic), else English. The web keeps the locale in the URL
//!   (`/ar/...`, prefix-less = English); native has no URL, so the choice
//!   is a stored preference, and "no choice" follows the device.
//! - [`t`] / [`t_args`] give closures for `text(..)` and every other
//!   `IntoReactive<SharedString>` prop: they read the locale signal, so a
//!   switch re-renders exactly the strings on screen.
//! - [`layout_direction`] is what the app root passes to `.direction(..)`.
//! - The locale suggestion keeps the web's storage key and answers
//!   (`easyquran.localeSuggest`); see [`suggestion`].
//!
//! ```ignore
//! use eq_core::i18n::keys;
//! use crate::stores::locale::{t, t_args};
//!
//! text(t(keys::NAV_SETTINGS));                                     // static
//! text(t_args(keys::READER_OFFLINE_ROUTES, move || vec![("count", routes.get().into())])); // args + plural
//! ```

use eq_core::{
    i18n::{
        self, Arg, Key, Locale, SuggestResolution, SuggestResolved, clear_locale_pref,
        load_locale_pref, save_locale_pref,
    },
    kv::{KeyValue, KvError},
};
use nsh_core::{prelude::*, storage::Storage};

use crate::platform::locale::{system_languages, system_locale};

/// The explicit choice (`None`: follow the system).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Pref(Option<Locale>);

/// The system languages as read before the app overrode NSH's locale
/// (NSH then reports the override first).
#[derive(Debug, Clone, Default, PartialEq)]
struct SystemLanguages(Vec<String>);

/// [`KeyValue`] over NSH `Storage` (the same entries TSX `nsh/storage`
/// sees).
struct NshKv;

impl KeyValue for NshKv {
    fn get(&self, key: &str) -> Option<String> {
        Storage::get_item(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        Storage::set_item(key, value).map_err(|e| KvError::Write(e.to_string()))
    }

    fn remove(&self, key: &str) {
        Storage::remove_item(key);
    }

    fn keys(&self) -> Vec<String> {
        Storage::get_all_keys()
    }
}

/// Loads the stored choice and applies it. Call once, from the app root's
/// body, before anything reads [`locale`].
pub fn install() {
    global::<SystemLanguages>().set(SystemLanguages(untracked(system_languages)));
    let pref = load_locale_pref(&NshKv);
    global::<Pref>().set(Pref(pref));
    apply(pref);
}

/// The locale the app's text shows in. Reading it subscribes.
pub fn locale() -> Locale {
    match global::<Pref>().get().0 {
        Some(locale) => locale,
        None => system_locale().unwrap_or_default(),
    }
}

/// The user's explicit choice, `None` while following the system. Reading
/// it subscribes.
pub fn explicit_locale() -> Option<Locale> {
    global::<Pref>().get().0
}

/// Switches the UI to `locale` and keeps the choice. A failed write (storage
/// full) still switches for this run, and is logged.
pub fn set_locale(locale: Locale) {
    if let Err(e) = save_locale_pref(&NshKv, locale) {
        log::warn!("locale preference not saved: {e}");
    }
    global::<Pref>().set(Pref(Some(locale)));
    apply(Some(locale));
}

/// Forgets the explicit choice: the UI follows the system language again.
pub fn follow_system() {
    clear_locale_pref(&NshKv);
    global::<Pref>().set(Pref(None));
    apply(None);
}

/// Tells NSH (its `locale()` / `locale_settings()`, which `nsh_base`'s
/// date and time controls read) about the choice.
fn apply(pref: Option<Locale>) {
    nsh_core::i18n::set_locale(pref.and_then(|l| nsh_core::platform::Locale::parse(l.code())));
}

/// Whether the UI lays out right to left. Reading it subscribes.
pub fn is_rtl() -> bool {
    locale().is_rtl()
}

/// The direction the app root lays out in: pass it as
/// `.direction(layout_direction)`.
pub fn layout_direction() -> LayoutDirection {
    if is_rtl() {
        return LayoutDirection::Rtl;
    }
    LayoutDirection::Ltr
}

/// The text of `key` in the current locale, now. Reading it subscribes:
/// call it inside a binding closure, not in a component body.
pub fn tr(key: Key) -> &'static str {
    i18n::tr(locale(), key)
}

/// A binding for `key`'s text: pass it to `text(..)`, `accessibility_label(..)`
/// or any `IntoReactive<SharedString>` prop; it follows locale switches.
pub fn t(key: Key) -> impl Fn() -> SharedString + Copy + 'static {
    move || SharedString::from(i18n::tr(locale(), key))
}

/// A binding for `key` with `{param}`s / plural variants: `args` runs inside
/// the binding, so signals it reads keep the text current too.
pub fn t_args<F>(key: Key, args: F) -> impl Fn() -> SharedString + 'static
where
    F: Fn() -> Vec<(&'static str, Arg<'static>)> + 'static,
{
    move || SharedString::from(i18n::tr_args(locale(), key, &args()))
}

/// "5 min. ago" for an ISO stamp, in the current locale (`timeAgo`).
pub fn time_ago(iso: &str, now_ms: i64) -> String {
    i18n::time_ago(iso, locale(), now_ms)
}

/// The locale to offer in the suggestion dialog, if any: the system's
/// preferred supported locale when it is not the current one and the user
/// has not answered for it (`easyquran.localeSuggest`). The caller adds the
/// web's surface conditions (once per run, not over the auth modal or the
/// palette). With no explicit choice the app already shows the system
/// locale, so this is `None` until the user picks another one.
pub fn suggestion() -> Option<Locale> {
    let live;
    let snapshot;
    let languages: &[String] = match explicit_locale() {
        None => {
            live = system_languages();
            &live
        }
        Some(_) => {
            snapshot = global::<SystemLanguages>().get();
            &snapshot.0
        }
    };
    i18n::suggest_locale(
        languages.iter().map(String::as_str),
        locale(),
        &SuggestResolved::load(&NshKv),
    )
}

/// Records the answer to the suggestion of `target`; accepting switches to
/// it (the web's accept link reloads into the target locale).
pub fn resolve_suggestion(target: Locale, answer: SuggestResolution) {
    if let Err(e) = SuggestResolved::resolve(&NshKv, target, answer) {
        log::warn!("locale suggestion answer not saved: {e}");
    }
    if answer == SuggestResolution::Accepted {
        set_locale(target);
    }
}
