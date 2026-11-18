//! i18n (PARITY O1, O2). Every user-facing string comes from the web's
//! inlang catalogues (`web/messages/{,reader/,auth/,account/,appeal/}{en,ar}.json`),
//! baked into a static table by `gen/i18n.rs` at compile time: never a
//! hand copy. Also: CLDR plural rules for en/ar, `{param}` substitution,
//! English fallback, RTL flag, relative time (`relative-time.ts`) and the
//! locale suggestion (`locale-suggest.svelte.ts`).
//!
//! # Keys
//!
//! Messages are addressed by the generated consts in [`keys`]: the web key
//! upper-cased (`nav_home` -> `keys::NAV_HOME`). A key the catalogue lacks
//! fails to compile. Each const's rustdoc shows its English text.
//!
//! ```
//! use eq_core::i18n::{Locale, keys, tr, tr_args};
//!
//! // Static text.
//! assert_eq!(tr(Locale::En, keys::NAV_SETTINGS), "Settings");
//! // With args and a plural (count picks the CLDR variant).
//! assert_eq!(tr_args(Locale::En, keys::READER_OFFLINE_ROUTES, &[("count", 1.into())]), "1 route");
//! assert_eq!(tr_args(Locale::Ar, keys::READER_OFFLINE_ROUTES, &[("count", 2.into())]), "مساران");
//! ```
//!
//! # Numbers
//!
//! The web passes raw numbers into messages, and paraglide interpolates them
//! with JS template literals (`${i?.count}`): plain ASCII digits, no
//! grouping, in both locales. [`Arg`] formats the same way.

mod plural;
mod prefs;
mod relative_time;
pub mod static_copy;
mod table;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

pub use plural::{PluralCategory, plural_category};
pub use prefs::{
    LOCALE_PREF_KEY, LOCALE_SUGGEST_STORAGE_KEY, SuggestResolution, SuggestResolved,
    clear_locale_pref, load_locale_pref, locale_from_languages, save_locale_pref, suggest_locale,
};
pub use relative_time::{TimeUnit, format_past, time_ago, time_ago_ms};
pub use table::keys;

use table::{AR, AR_MISSING, EN, KEY_COUNT, KEY_NAMES, Msg};

/// A UI locale (`locales.ts`: `UI_LOCALE_IDS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    /// English, the base locale (`DEFAULT_UI_LOCALE`).
    #[default]
    En,
    /// Arabic, right to left.
    Ar,
}

impl Locale {
    /// Every supported locale, base first (`SUPPORTED_UI_LOCALES`).
    pub const ALL: [Locale; 2] = [Locale::En, Locale::Ar];

    /// The BCP 47 code (`en`, `ar`).
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ar => "ar",
        }
    }

    /// The locale whose code is `code`, compared case-insensitively
    /// (paraglide's `toLocale`); `None` for any other tag, including
    /// regional ones (see [`locale_from_languages`] for those).
    pub fn from_code(code: &str) -> Option<Locale> {
        Locale::ALL
            .into_iter()
            .find(|l| l.code().eq_ignore_ascii_case(code.trim()))
    }

    /// Written right to left (`UI_LOCALES[l].direction == "rtl"`).
    pub fn is_rtl(self) -> bool {
        matches!(self, Locale::Ar)
    }

    /// The language's own name (`endonym`): what a locale switcher lists.
    pub fn endonym(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Ar => "العربية",
        }
    }

    fn index(self) -> usize {
        match self {
            Locale::En => 0,
            Locale::Ar => 1,
        }
    }
}

/// A message key: one of the [`keys`] consts, or [`Key::from_name`] for a
/// key known only at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key(pub(crate) u16);

impl Key {
    /// The key whose web name is `name` (`"nav_home"`), if the catalogue has
    /// it. Prefer the [`keys`] consts, which are checked when compiling.
    pub fn from_name(name: &str) -> Option<Key> {
        KEY_NAMES
            .binary_search(&name)
            .ok()
            .and_then(|i| u16::try_from(i).ok())
            .map(Key)
    }

    /// The web's name of the key (`"nav_home"`).
    pub fn name(self) -> &'static str {
        KEY_NAMES[usize::from(self.0)]
    }

    /// Every key, in name order.
    pub fn all() -> impl Iterator<Item = Key> {
        (0..KEY_COUNT)
            .filter_map(|i| u16::try_from(i).ok())
            .map(Key)
    }
}

/// A value for a `{param}`: text or a number. Numbers print as JS template
/// literals print them (`12`, `1.5`), and drive plural selection.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg<'a> {
    Str(Cow<'a, str>),
    Int(i64),
    Num(f64),
}

impl Arg<'_> {
    /// The number plural selection sees (`Number(x)` in JS).
    fn number(&self) -> f64 {
        match self {
            Arg::Int(n) => *n as f64,
            Arg::Num(n) => *n,
            Arg::Str(s) => js_number(s),
        }
    }

    fn render(&self, out: &mut String) {
        match self {
            Arg::Str(s) => out.push_str(s),
            Arg::Int(n) => out.push_str(&n.to_string()),
            Arg::Num(n) => out.push_str(&js_number_string(*n)),
        }
    }
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(s: &'a str) -> Self {
        Arg::Str(Cow::Borrowed(s))
    }
}

impl From<String> for Arg<'_> {
    fn from(s: String) -> Self {
        Arg::Str(Cow::Owned(s))
    }
}

impl<'a> From<&'a String> for Arg<'a> {
    fn from(s: &'a String) -> Self {
        Arg::Str(Cow::Borrowed(s.as_str()))
    }
}

macro_rules! int_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg<'_> {
            fn from(n: $t) -> Self {
                Arg::Int(i64::from(n))
            }
        }
    )*};
}
int_arg!(i8, i16, i32, i64, u8, u16, u32);

impl From<usize> for Arg<'_> {
    fn from(n: usize) -> Self {
        i64::try_from(n).map_or(Arg::Num(n as f64), Arg::Int)
    }
}

impl From<u64> for Arg<'_> {
    fn from(n: u64) -> Self {
        i64::try_from(n).map_or(Arg::Num(n as f64), Arg::Int)
    }
}

impl From<f64> for Arg<'_> {
    fn from(n: f64) -> Self {
        Arg::Num(n)
    }
}

impl From<f32> for Arg<'_> {
    fn from(n: f32) -> Self {
        Arg::Num(f64::from(n))
    }
}

/// `Number(s)` for the strings messages get: trimmed decimal, empty is 0,
/// anything else NaN.
fn js_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    t.parse().unwrap_or(f64::NAN)
}

/// `String(n)` for the numbers messages show: integers without a fraction.
fn js_number_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        return format!("{n:.0}");
    }
    n.to_string()
}

fn message(locale: Locale, key: Key) -> &'static Msg {
    let i = usize::from(key.0);
    match locale {
        Locale::En => &EN[i],
        Locale::Ar => AR[i].as_ref().unwrap_or(&EN[i]),
    }
}

/// The text of `key` in `locale` (English where Arabic lacks it). For a
/// message with `{param}`s or plural variants this is its raw `other`
/// pattern: use [`tr_args`] for those.
pub fn tr(locale: Locale, key: Key) -> &'static str {
    match message(locale, key) {
        Msg::Text(text) => text,
        Msg::Match(m) => m
            .variants
            .iter()
            .find(|v| v.when.iter().all(|w| *w == "other" || *w == "*"))
            .or(m.variants.last())
            .map_or("", |v| v.text),
    }
}

/// The text of `key` in `locale` with `args` in its `{param}`s; a plural
/// message picks the variant by the CLDR category of its selector's arg.
/// A placeholder without an arg stays as written (`{name}`); a match no
/// variant takes shows the key's name, as paraglide does.
pub fn tr_args(locale: Locale, key: Key, args: &[(&str, Arg<'_>)]) -> String {
    let pattern = match message(locale, key) {
        Msg::Text(text) => *text,
        Msg::Match(m) => match select(locale, m, args) {
            Some(text) => text,
            None => return key.name().to_owned(),
        },
    };
    substitute(pattern, args)
}

fn select(locale: Locale, m: &table::Match, args: &[(&str, Arg<'_>)]) -> Option<&'static str> {
    let values: Vec<Cow<'_, str>> = m
        .selectors
        .iter()
        .map(|selector| {
            let arg = args
                .iter()
                .find(|(name, _)| *name == selector.input)
                .map(|(_, a)| a);
            selector_value(locale, selector.plural, arg)
        })
        .collect();
    m.variants
        .iter()
        .find(|v| {
            v.when
                .iter()
                .zip(&values)
                .all(|(want, have)| *want == "*" || *want == have.as_ref())
        })
        .map(|v| v.text)
}

fn selector_value<'a>(locale: Locale, plural: bool, arg: Option<&'a Arg<'a>>) -> Cow<'a, str> {
    if plural {
        let n = arg.map_or(f64::NAN, Arg::number);
        return Cow::Borrowed(plural_category(locale, n).as_str());
    }
    match arg {
        Some(Arg::Str(s)) => Cow::Borrowed(s.as_ref()),
        Some(other) => {
            let mut out = String::new();
            other.render(&mut out);
            Cow::Owned(out)
        }
        None => Cow::Borrowed("undefined"),
    }
}

fn substitute(pattern: &str, args: &[(&str, Arg<'_>)]) -> String {
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &after[..close];
        match args.iter().find(|(n, _)| *n == name) {
            Some((_, arg)) => arg.render(&mut out),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The text of the key named `name` (a key known only at run time), `None`
/// when the catalogue lacks it.
pub fn tr_name(locale: Locale, name: &str) -> Option<&'static str> {
    Key::from_name(name).map(|key| tr(locale, key))
}

/// The keys Arabic lacks (they show in English).
pub fn ar_missing() -> &'static [&'static str] {
    &AR_MISSING
}
