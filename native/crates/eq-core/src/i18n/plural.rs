//! CLDR cardinal plural rules for the app's locales: what the web's
//! `Intl.PluralRules(locale).select(Number(x))` answers (paraglide's
//! `registry.plural`).

use super::Locale;

/// A CLDR plural category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    /// The CLDR name, as the catalogues' match keys write it.
    pub fn as_str(self) -> &'static str {
        match self {
            PluralCategory::Zero => "zero",
            PluralCategory::One => "one",
            PluralCategory::Two => "two",
            PluralCategory::Few => "few",
            PluralCategory::Many => "many",
            PluralCategory::Other => "other",
        }
    }
}

/// The plural category of `n` in `locale`. NaN and infinities are `Other`,
/// as `Intl.PluralRules` answers.
///
/// - en: `one` for exactly 1, else `other` (a JS number cannot tell `1.0`
///   from `1`, so neither does this).
/// - ar: `zero` 0, `one` 1, `two` 2, `few` when `n % 100` is 3..=10, `many`
///   when it is 11..=99 (integers only), else `other`.
pub fn plural_category(locale: Locale, n: f64) -> PluralCategory {
    if !n.is_finite() {
        return PluralCategory::Other;
    }
    let n = n.abs();
    match locale {
        Locale::En => english(n),
        Locale::Ar => arabic(n),
    }
}

fn english(n: f64) -> PluralCategory {
    if n == 1.0 {
        return PluralCategory::One;
    }
    PluralCategory::Other
}

fn arabic(n: f64) -> PluralCategory {
    if n == 0.0 {
        return PluralCategory::Zero;
    }
    if n == 1.0 {
        return PluralCategory::One;
    }
    if n == 2.0 {
        return PluralCategory::Two;
    }
    if n.fract() != 0.0 {
        return PluralCategory::Other;
    }
    let rem = n % 100.0;
    if (3.0..=10.0).contains(&rem) {
        return PluralCategory::Few;
    }
    if (11.0..=99.0).contains(&rem) {
        return PluralCategory::Many;
    }
    PluralCategory::Other
}
