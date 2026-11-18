//! "5 min. ago" / "yesterday": the web's `timeAgo` (`lib/i18n/relative-time.ts`),
//! which formats with `Intl.RelativeTimeFormat(locale, { numeric: "auto",
//! style: "short" })`. The phrases below are that formatter's output for en
//! and ar (ICU 78 / CLDR 48, Node 24): Latin digits with `,` grouping in both
//! locales, `numeric: "auto"` words for 0 and the past day offsets CLDR
//! names.

use super::{Locale, PluralCategory, plural_category};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// A unit `time_ago` counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Minute,
    Hour,
    Day,
}

/// `iso` relative to `now_ms` (Unix ms) in `locale`; `""` when `iso` does
/// not parse. Future stamps read as now.
///
/// `iso` is RFC 3339 / ISO 8601 as the backend sends it
/// (`2026-11-18T10:00:00Z`, fractional seconds, `±hh:mm` offsets) or a bare
/// date (`2026-11-18`, UTC midnight, as `Date.parse` reads it). A date-time
/// without an offset is read as UTC (JS would read local time; the backend
/// never sends one).
pub fn time_ago(iso: &str, locale: Locale, now_ms: i64) -> String {
    match parse_iso_ms(iso) {
        Some(at) => time_ago_ms(at, locale, now_ms),
        None => String::new(),
    }
}

/// [`time_ago`] for a stamp already in Unix ms.
pub fn time_ago_ms(at_ms: i64, locale: Locale, now_ms: i64) -> String {
    let elapsed = now_ms.saturating_sub(at_ms).max(0);
    if elapsed < HOUR {
        return format_past(locale, elapsed / MINUTE, TimeUnit::Minute);
    }
    if elapsed < DAY {
        return format_past(locale, elapsed / HOUR, TimeUnit::Hour);
    }
    format_past(locale, elapsed / DAY, TimeUnit::Day)
}

/// `Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" })
/// .format(-n, unit)` for `n >= 0`.
pub fn format_past(locale: Locale, n: i64, unit: TimeUnit) -> String {
    match locale {
        Locale::En => english(n, unit),
        Locale::Ar => arabic(n, unit),
    }
}

fn english(n: i64, unit: TimeUnit) -> String {
    let count = group(n);
    match (unit, n) {
        (TimeUnit::Minute, 0) => "this minute".into(),
        (TimeUnit::Minute, _) => format!("{count} min. ago"),
        (TimeUnit::Hour, 0) => "this hour".into(),
        (TimeUnit::Hour, _) => format!("{count} hr. ago"),
        (TimeUnit::Day, 0) => "today".into(),
        (TimeUnit::Day, 1) => "yesterday".into(),
        (TimeUnit::Day, _) => format!("{count} days ago"),
    }
}

/// The ar words per unit: (now, one, two, few, many/other-with-tanween,
/// other).
struct ArUnit {
    now: &'static str,
    one: &'static str,
    two: &'static str,
    few: &'static str,
    many: &'static str,
    other: &'static str,
}

const AR_MINUTE: ArUnit = ArUnit {
    now: "هذه الدقيقة",
    one: "قبل دقيقة واحدة",
    two: "قبل دقيقتين",
    few: "دقائق",
    many: "دقيقة",
    other: "دقيقة",
};

const AR_HOUR: ArUnit = ArUnit {
    now: "الساعة الحالية",
    one: "قبل ساعة واحدة",
    two: "قبل ساعتين",
    few: "ساعات",
    many: "ساعة",
    other: "ساعة",
};

const AR_DAY: ArUnit = ArUnit {
    now: "اليوم",
    one: "أمس",
    two: "أول أمس",
    few: "أيام",
    many: "يومًا",
    other: "يوم",
};

fn arabic(n: i64, unit: TimeUnit) -> String {
    let words = match unit {
        TimeUnit::Minute => &AR_MINUTE,
        TimeUnit::Hour => &AR_HOUR,
        TimeUnit::Day => &AR_DAY,
    };
    // `n` is never negative here; `as f64` is exact for any count of days.
    let noun = match plural_category(Locale::Ar, n as f64) {
        PluralCategory::Zero => return words.now.into(),
        PluralCategory::One => return words.one.into(),
        PluralCategory::Two => return words.two.into(),
        PluralCategory::Few => words.few,
        PluralCategory::Many => words.many,
        PluralCategory::Other => words.other,
    };
    format!("قبل {} {noun}", group(n))
}

/// `n` with `,` between groups of three digits (`1,234`).
fn group(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Unix ms of an RFC 3339 date-time or a bare `YYYY-MM-DD`.
pub(crate) fn parse_iso_ms(iso: &str) -> Option<i64> {
    let s = iso.trim();
    let date = s.get(..10)?;
    let year: i64 = date.get(0..4)?.parse().ok()?;
    let month: u32 = date.get(5..7)?.parse().ok()?;
    let day: u32 = date.get(8..10)?.parse().ok()?;
    if date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let rest = &s[10..];
    if rest.is_empty() {
        return Some(days * DAY);
    }
    let rest = rest.strip_prefix(['T', 't', ' '])?;
    let hour: i64 = rest.get(0..2)?.parse().ok()?;
    let minute: i64 = rest.get(3..5)?.parse().ok()?;
    if rest.as_bytes().get(2) != Some(&b':') {
        return None;
    }
    let mut tail = &rest[5..];
    let mut second = 0_i64;
    let mut millis = 0_i64;
    if let Some(after) = tail.strip_prefix(':') {
        second = after.get(0..2)?.parse().ok()?;
        tail = &after[2..];
        if let Some(frac) = tail.strip_prefix('.') {
            let len = frac.bytes().take_while(u8::is_ascii_digit).count();
            if len == 0 {
                return None;
            }
            let digits = &frac[..len];
            let padded = format!("{:0<3}", &digits[..len.min(3)]);
            millis = padded.parse().ok()?;
            tail = &frac[len..];
        }
    }
    if hour > 24 || minute > 59 || second > 59 {
        return None;
    }
    let offset_ms = parse_offset(tail)?;
    let local = days * DAY + hour * HOUR + minute * MINUTE + second * 1000 + millis;
    Some(local - offset_ms)
}

/// `Z`, `±hh:mm`, `±hhmm`, or nothing (UTC).
fn parse_offset(tail: &str) -> Option<i64> {
    if tail.is_empty() || tail == "Z" || tail == "z" {
        return Some(0);
    }
    let sign = match tail.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let body = &tail[1..];
    let (h, m) = match body.len() {
        5 if body.as_bytes()[2] == b':' => (body.get(0..2)?, body.get(3..5)?),
        4 => (body.get(0..2)?, body.get(2..4)?),
        2 => (body, "00"),
        _ => return None,
    };
    let h: i64 = h.parse().ok()?;
    let m: i64 = m.parse().ok()?;
    Some(sign * (h * HOUR + m * MINUTE))
}

fn is_leap(year: i64) -> bool {
    (year.rem_euclid(4) == 0 && year.rem_euclid(100) != 0) || year.rem_euclid(400) == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
