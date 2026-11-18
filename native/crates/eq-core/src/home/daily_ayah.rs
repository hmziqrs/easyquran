//! Ayah of the day (PARITY H2; web `lib/home/daily-ayah.ts`,
//! `lib/server/daily-ayah.ts`). The verse is a pure function of the UTC
//! date: `(days * K + C) mod 6236`, 1-based, the same permutation as the
//! backend's `random_global` and the web page, so every surface names the
//! same verse on the same UTC day. No network: the app reads the verse
//! text from its own Quran source.

use crate::{quran::meta::meta, store::goals::local_day};

const DAILY_K: i64 = 1103;
const DAILY_C: i64 = 4177;
const DAILY_VERSE_COUNT: i64 = 6236;
const DAY_MS: i64 = 86_400_000;

/// The UTC calendar date of `now_ms` as `YYYY-MM-DD` (`utcDateKey`).
pub fn utc_date_key(now_ms: i64) -> String {
    local_day(now_ms, 0)
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` date; `None` for anything else.
fn epoch_days(date: &str) -> Option<i64> {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = date.get(r)?;
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())
            .flatten()
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant), proleptic Gregorian.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// 1-based global ayah index for a `YYYY-MM-DD` UTC date
/// (`dailyGlobalIndex`); `None` for a malformed date.
pub fn daily_global_index(utc_date: &str) -> Option<u32> {
    let days = epoch_days(utc_date)?;
    let slot = (days * DAILY_K + DAILY_C).rem_euclid(DAILY_VERSE_COUNT);
    u32::try_from(slot + 1).ok()
}

/// The day's verse: `(global index, surah, ayah)`.
pub fn daily_verse(utc_date: &str) -> Option<(u32, u16, u16)> {
    let global = daily_global_index(utc_date)?;
    let (surah, ayah) = meta().verse_at_global(global)?;
    Some((global, surah, ayah))
}

/// Milliseconds from `now_ms` to the next 00:00 UTC, at least 1 s.
pub fn ms_until_utc_midnight(now_ms: i64) -> i64 {
    let next = (now_ms.div_euclid(DAY_MS) + 1) * DAY_MS;
    (next - now_ms).max(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_web_permutation() {
        // days = 0 → (0 * K + C) mod 6236 + 1.
        assert_eq!(daily_global_index("1970-01-01"), Some(4178));
        // 2026-11-18 is day 20775: (20775 * 1103 + 4177) mod 6236 + 1.
        let days = 20_775_i64;
        assert_eq!(epoch_days("2026-11-18"), Some(days));
        let want = ((days * 1103 + 4177) % 6236 + 1) as u32;
        assert_eq!(daily_global_index("2026-11-18"), Some(want));
        assert_eq!(daily_global_index("2026-13-01"), None);
        assert_eq!(daily_global_index("20261118"), None);
        assert_eq!(utc_date_key(0), "1970-01-01");
        let (g, s, a) = daily_verse("1970-01-01").unwrap();
        assert_eq!(meta().global_index_of(s, a), Some(g));
    }
}
