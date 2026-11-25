//! Rules for `Field::rule`. Each returns the message when the value fails.

use std::fmt::Display;
use std::str::FromStr;

use gpui_form_core::numeric::{validate_signed_numeric, validate_unsigned_numeric};

/// Something a person left blank: empty after trimming, or `None`.
pub trait Blank {
    fn is_blank(&self) -> bool;
}

impl Blank for String {
    fn is_blank(&self) -> bool {
        self.trim().is_empty()
    }
}

impl<T> Blank for Option<T> {
    fn is_blank(&self) -> bool {
        self.is_none()
    }
}

impl<T> Blank for Vec<T> {
    fn is_blank(&self) -> bool {
        self.is_empty()
    }
}

pub fn required<T: Blank, H>(message: impl Display) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| value.is_blank().then(|| message.clone())
}

/// Counts characters, not bytes. An empty value passes, so pair it with `required`.
pub fn min_length<H>(min: usize, message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (!value.is_empty() && value.chars().count() < min).then(|| message.clone())
}

pub fn max_length<H>(max: usize, message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (value.chars().count() > max).then(|| message.clone())
}

/// One `@` with something on both sides and a dot after it. A mail server decides
/// the rest. An empty value passes, so pair it with `required`.
pub fn email<H>(message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| {
        if value.is_empty() {
            return None;
        }
        let valid = value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && !domain.contains('@')
                && domain.split('.').count() >= 2
                && domain.split('.').all(|part| !part.is_empty())
        });
        (!valid).then(|| message.clone())
    }
}

/// The value must equal another value of the form, such as a password's confirmation.
pub fn matches<T: PartialEq, H>(
    other: fn(&H) -> &T,
    message: impl Display,
) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, values| (value != other(values)).then(|| message.clone())
}

pub fn range<T: PartialOrd, H>(
    min: T,
    max: T,
    message: impl Display,
) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (*value < min || *value > max).then(|| message.clone())
}

/// The value must be at least `min`. The lower bound of [`range`].
pub fn min<T: PartialOrd, H>(min: T, message: impl Display) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (*value < min).then(|| message.clone())
}

/// The value must be at most `max`. The upper bound of [`range`].
pub fn max<T: PartialOrd, H>(max: T, message: impl Display) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (*value > max).then(|| message.clone())
}

/// The value must be one of `allowed`.
pub fn one_of<T: PartialEq, H>(
    allowed: Vec<T>,
    message: impl Display,
) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (!allowed.contains(value)).then(|| message.clone())
}

/// Text that parses as `T`, signed or decimal, using gpui-form's numeric check.
pub fn signed_number<T: FromStr, H>(
    message: impl Display,
) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (!validate_signed_numeric::<T>(value, true)).then(|| message.clone())
}

/// Text that parses as an unsigned `T`, using gpui-form's numeric check.
pub fn unsigned_number<T: FromStr, H>(
    message: impl Display,
) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| (!validate_unsigned_numeric::<T>(value, true)).then(|| message.clone())
}

/// The text must match the regular expression `pattern`, which uses Rust's `regex` syntax.
/// An empty value passes, so pair it with `required`. Panics when `pattern` does not compile.
pub fn pattern<H>(pattern: &str, message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let regex = regex::Regex::new(pattern).expect("a valid regular expression");
    let message = message.to_string();
    move |value, _| (!value.is_empty() && !regex.is_match(value)).then(|| message.clone())
}

/// The value must start with `http://` or `https://`, then have a host with no spaces. The
/// whole address is not parsed, so a host that is wrong in a way a DNS lookup would catch still
/// passes. An empty value passes; pair it with `required`.
pub fn url<H>(message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| {
        if value.is_empty() {
            return None;
        }
        let rest = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"));
        let valid = rest.is_some_and(|rest| {
            let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
            !host.is_empty() && !value.contains(char::is_whitespace) && !host.starts_with('.')
        });
        (!valid).then(|| message.clone())
    }
}

/// Lowercase ASCII letters, digits and single hyphens, with no hyphen at either end. An empty
/// value passes; pair it with `required`.
pub fn slug<H>(message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| {
        if value.is_empty() {
            return None;
        }
        let valid = !value.starts_with('-')
            && !value.ends_with('-')
            && !value.contains("--")
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        (!valid).then(|| message.clone())
    }
}

/// The 8-4-4-4-12 hexadecimal form of a UUID, in either case. An empty value passes; pair it
/// with `required`.
pub fn uuid<H>(message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| {
        if value.is_empty() {
            return None;
        }
        let groups: Vec<&str> = value.split('-').collect();
        let shape = [8usize, 4, 4, 4, 12];
        let valid = groups.len() == shape.len()
            && groups.iter().zip(shape).all(|(group, len)| {
                group.len() == len && group.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
        (!valid).then(|| message.clone())
    }
}

/// Required only when `when` holds for the whole form, such as "an email is needed when the
/// newsletter is on". `T` is blank when [`Blank::is_blank`] says so.
pub fn required_when<T: Blank, H>(
    when: fn(&H) -> bool,
    message: impl Display,
) -> impl Fn(&T, &H) -> Option<String> {
    let message = message.to_string();
    move |value, form| (when(form) && value.is_blank()).then(|| message.clone())
}

/// A phone number that libphonenumber accepts, parsed with gpui-form's own check. An empty value
/// passes; pair it with `required`. Needs the `phone` feature.
#[cfg(feature = "phone")]
pub fn phone<H>(message: impl Display) -> impl Fn(&String, &H) -> Option<String> {
    let message = message.to_string();
    move |value, _| {
        let result = gpui_form_core::phone::validate_optional_phone_number(value, None);
        (!result.is_valid_or_empty()).then(|| message.clone())
    }
}
