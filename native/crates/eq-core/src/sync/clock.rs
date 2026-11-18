//! Hybrid logical clock for one account on one install (contracts/sync.md
//! "Clock", web `lib/sync/clock.ts`). Merging a received stamp advances the
//! clock, so an edit made after seeing a row always beats that row whatever
//! the device clock says. Device clocks only ever set a floor. The caller
//! persists [`ClockState`] before it uses a stamp (the store does).
//!
//! Also the ISO-8601 helpers the stores use for display times (`updatedAt`),
//! so eq-core needs no date crate.

use serde::{Deserialize, Serialize};

use crate::merge::{compare_stamps, format_stamp, parse_stamp};

/// A clock once set ahead cannot poison later stamps past this.
pub const MAX_AHEAD_MS: i64 = 60_000;
const COUNTER_MAX: u32 = 0xffff;

/// What persists per account: the server offset and the last stamp issued or seen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockState {
    pub offset: i64,
    pub last_ms: i64,
    pub last_counter: u32,
}

/// One account's clock on this install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HybridClock {
    device_id: String,
    state: ClockState,
}

impl HybridClock {
    pub fn new(device_id: impl Into<String>, state: ClockState) -> Self {
        Self {
            device_id: device_id.into(),
            state,
        }
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn state(&self) -> ClockState {
        self.state
    }

    /// Next stamp: above every stamp this account issued or saw here.
    /// `local_now` is the raw device clock.
    pub fn stamp(&mut self, local_now: i64) -> String {
        let floor = local_now + self.state.offset;
        let mut ms = floor.max(self.state.last_ms);
        let mut counter = if ms == self.state.last_ms {
            self.state.last_counter + 1
        } else {
            0
        };
        if counter > COUNTER_MAX {
            ms += 1;
            counter = 0;
        }
        self.state.last_ms = ms;
        self.state.last_counter = counter;
        format_stamp(ms.max(0) as u64, counter, &self.device_id)
    }

    /// A stamp for an edit made signed out, folding in at sign-in with its own
    /// time: older than the clock's floor on purpose.
    pub fn stamp_at(&mut self, ms: i64) -> String {
        let ms = ms.max(0);
        let stamp = format_stamp(ms as u64, 0, &self.device_id);
        let seen = format_stamp(
            self.state.last_ms.max(0) as u64,
            self.state.last_counter,
            &self.device_id,
        );
        if compare_stamps(Some(&stamp), Some(&seen)).is_gt() {
            self.state.last_ms = ms;
            self.state.last_counter = 0;
        }
        stamp
    }

    /// Advance past a stamp received from the server. Returns whether it moved.
    pub fn observe(&mut self, stamp: Option<&str>) -> bool {
        let Some(parsed) = stamp.and_then(parse_stamp) else {
            return false;
        };
        let ms = parsed.ms as i64;
        let newer = ms > self.state.last_ms
            || (ms == self.state.last_ms && parsed.counter > self.state.last_counter);
        if newer {
            self.state.last_ms = ms;
            self.state.last_counter = parsed.counter;
        }
        newer
    }

    /// Learn the server's time from a sync response. A `last` more than 60 s
    /// ahead of the server is pulled back to `server + 60 s`. Wake and resume
    /// need nothing more: the next round refreshes the offset.
    pub fn on_server_time(&mut self, server_ms: i64, local_now: i64) {
        self.state.offset = server_ms - local_now;
        let ceiling = server_ms + MAX_AHEAD_MS;
        if self.state.last_ms > ceiling {
            self.state.last_ms = ceiling;
            self.state.last_counter = 0;
        }
    }

    /// Local time corrected by the last known offset: for staleness, labels
    /// and local days, in place of the raw device clock.
    pub fn now(&self, local_now: i64) -> i64 {
        local_now + self.state.offset
    }
}

const MS_PER_DAY: i64 = 86_400_000;

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's civil_from_days.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → days since 1970-01-01.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-11-18T09:30:00.000Z`, as JavaScript's `toISOString`.
pub fn iso_from_ms(ms: i64) -> String {
    let days = ms.div_euclid(MS_PER_DAY);
    let rem = ms.rem_euclid(MS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        (rem / 60_000) % 60,
        (rem / 1000) % 60,
        rem % 1000
    )
}

fn digits(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Epoch ms of an RFC 3339 time (`Z` or `±HH:MM`, optional fraction), as
/// `Date.parse` reads the times the stores and the server write.
pub fn ms_from_iso(iso: &str) -> Option<i64> {
    let (date, time) = iso.split_once('T')?;
    let mut parts = date.split('-');
    let year = digits(parts.next()?)?;
    let month = digits(parts.next()?)?;
    let day = digits(parts.next()?)?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let (clock, offset_min) = split_zone(time)?;
    let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
    let mut t = hms.split(':');
    let h = digits(t.next()?)?;
    let mi = digits(t.next()?)?;
    let s = t.next().map_or(Some(0), digits)?;
    let frac_ms = match frac.len() {
        0 => 0,
        n => digits(&frac[..n.min(3)])? * 10_i64.pow(3 - n.min(3) as u32),
    };
    let days = days_from_civil(year, month as u32, day as u32);
    Some(days * MS_PER_DAY + h * 3_600_000 + mi * 60_000 + s * 1000 + frac_ms - offset_min * 60_000)
}

fn split_zone(time: &str) -> Option<(&str, i64)> {
    if let Some(clock) = time.strip_suffix('Z').or_else(|| time.strip_suffix('z')) {
        return Some((clock, 0));
    }
    let at = time.rfind(['+', '-'])?;
    let (clock, zone) = time.split_at(at);
    let sign = if zone.starts_with('-') { -1 } else { 1 };
    let (hh, mm) = zone[1..].split_once(':').unwrap_or((&zone[1..], "0"));
    Some((clock, sign * (digits(hh)? * 60 + digits(mm)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::{Rule, merge_register};
    use serde_json::json;

    const HOUR: i64 = 3_600_000;
    const SERVER: i64 = 1_800_000_000_000;

    fn ms_of(stamp: &str) -> i64 {
        parse_stamp(stamp).unwrap().ms as i64
    }

    #[test]
    fn a_slow_device_editing_after_a_pull_beats_what_it_saw() {
        let mut fast = HybridClock::new("fast", ClockState::default());
        let mut slow = HybridClock::new("slow", ClockState::default());
        let seen = json!({ "stamp": fast.stamp(SERVER), "value": "elsewhere" });
        slow.observe(seen["stamp"].as_str());
        let edit = json!({ "stamp": slow.stamp(SERVER - HOUR), "value": "after the pull" });
        assert_eq!(merge_register(&seen, &edit, Rule::Highest), edit);
        assert_eq!(merge_register(&edit, &seen, Rule::Highest), edit);
    }

    #[test]
    fn a_fast_device_cannot_dominate_after_its_first_offset() {
        let mut fast = HybridClock::new("fast", ClockState::default());
        assert_eq!(ms_of(&fast.stamp(SERVER + HOUR)), SERVER + HOUR);
        fast.on_server_time(SERVER, SERVER + HOUR);
        assert!(ms_of(&fast.stamp(SERVER + HOUR)) <= SERVER + MAX_AHEAD_MS);
    }

    #[test]
    fn a_clock_a_year_ahead_is_pulled_back_after_any_sync() {
        let year = 365 * 24 * HOUR;
        let mut clock = HybridClock::new("d", ClockState::default());
        clock.stamp(SERVER + year);
        clock.on_server_time(SERVER, SERVER + year);
        assert_eq!(ms_of(&clock.stamp(SERVER + year)), SERVER + MAX_AHEAD_MS);
    }

    #[test]
    fn stamps_keep_rising_when_the_clock_moves_backwards() {
        let mut clock = HybridClock::new("d", ClockState::default());
        let a = clock.stamp(SERVER);
        let b = clock.stamp(SERVER - HOUR);
        let c = clock.stamp(SERVER - 2 * HOUR);
        assert!(b > a && c > b);
    }

    #[test]
    fn same_ms_counts_up_and_overflow_bumps_the_ms() {
        let mut clock = HybridClock::new(
            "d",
            ClockState {
                offset: 0,
                last_ms: SERVER,
                last_counter: 0xffff,
            },
        );
        let parsed = parse_stamp(&clock.stamp(SERVER)).unwrap();
        assert_eq!((parsed.ms as i64, parsed.counter), (SERVER + 1, 0));
    }

    #[test]
    fn offset_refreshes_on_the_next_sync() {
        let mut clock = HybridClock::new("d", ClockState::default());
        assert_eq!(clock.now(SERVER - HOUR), SERVER - HOUR);
        clock.on_server_time(SERVER, SERVER - HOUR);
        assert_eq!(clock.now(SERVER - HOUR), SERVER);
    }

    #[test]
    fn a_signed_out_edit_folds_in_with_its_own_time() {
        let mut clock = HybridClock::new("d", ClockState::default());
        assert_eq!(ms_of(&clock.stamp_at(SERVER - HOUR)), SERVER - HOUR);
        // And a later live stamp still rises above it.
        assert!(clock.stamp(SERVER - 2 * HOUR) > format_stamp((SERVER - HOUR) as u64, 0, "d"));
    }

    #[test]
    fn iso_round_trips() {
        for ms in [0, 1_700_000_000_123, 1_800_000_000_000, 951_782_400_000] {
            assert_eq!(ms_from_iso(&iso_from_ms(ms)), Some(ms));
        }
        assert_eq!(iso_from_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            ms_from_iso("2026-11-01T02:00:00+02:00"),
            ms_from_iso("2026-11-01T00:00:00Z")
        );
        assert_eq!(ms_from_iso("garbage"), None);
    }
}
