//! Numeric policy (web `lib/audio/policy.ts`): playback speed, retries,
//! offline storage and repeat limits.

/// `PLAYBACK_RATES`.
pub const PLAYBACK_RATES: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];
/// `DEFAULT_PLAYBACK_RATE`.
pub const DEFAULT_PLAYBACK_RATE: f32 = 1.0;

/// `nearestRate`: snaps any number to the nearest allowed rate (the first
/// on a tie, as the web's strict `<`).
pub fn nearest_rate(value: f64) -> f32 {
    if !value.is_finite() {
        return DEFAULT_PLAYBACK_RATE;
    }
    let mut best = DEFAULT_PLAYBACK_RATE;
    let mut best_gap = f64::INFINITY;
    for rate in PLAYBACK_RATES {
        let gap = (f64::from(rate) - value).abs();
        if gap < best_gap {
            best = rate;
            best_gap = gap;
        }
    }
    best
}

/// `MAX_LOAD_RETRIES`.
pub const MAX_LOAD_RETRIES: u32 = 3;

/// `retryDelayMs`: 1 s, 2 s, 4 s for attempts 1..=3; `None` after.
pub fn retry_delay_ms(attempt: u32) -> Option<u64> {
    if !(1..=MAX_LOAD_RETRIES).contains(&attempt) {
        return None;
    }
    Some(1000 * 2_u64.pow(attempt - 1))
}

/// `OFFLINE_AUDIO_MAX_BYTES`: 300 MB per device.
pub const OFFLINE_AUDIO_MAX_BYTES: u64 = 300 * 1024 * 1024;
/// `OFFLINE_AUDIO_MAX_FILE_BYTES`: 20 MB per file.
pub const OFFLINE_AUDIO_MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// `admitOfflineBytes`.
pub fn admit_offline_bytes(used: u64, file: u64) -> bool {
    if file == 0 || file > OFFLINE_AUDIO_MAX_FILE_BYTES {
        return false;
    }
    used.saturating_add(file) <= OFFLINE_AUDIO_MAX_BYTES
}

/// An inclusive integer range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub min: i64,
    pub max: i64,
}

/// `REPEAT_LIMITS.ayahRepeat`.
pub const AYAH_REPEAT: Limit = Limit { min: 1, max: 20 };
/// `REPEAT_LIMITS.setRepeat`.
pub const SET_REPEAT: Limit = Limit { min: 1, max: 100 };
/// `REPEAT_LIMITS.delayMs`.
pub const DELAY_MS: Limit = Limit {
    min: 0,
    max: 10_000,
};

/// `MAX_REPEAT_STEPS`.
pub const MAX_REPEAT_STEPS: u64 = 5000;

/// `clampInt`: rounds, then clamps; non-finite is `min`.
pub fn clamp_int(value: f64, limit: Limit) -> i64 {
    if !value.is_finite() {
        return limit.min;
    }
    (value.round() as i64).clamp(limit.min, limit.max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_snap() {
        assert_eq!(nearest_rate(1.1), 1.0);
        assert_eq!(nearest_rate(1.9), 2.0);
        assert_eq!(nearest_rate(0.1), 0.75);
        assert_eq!(nearest_rate(f64::NAN), 1.0);
    }

    #[test]
    fn retries_back_off() {
        assert_eq!(retry_delay_ms(0), None);
        assert_eq!(retry_delay_ms(1), Some(1000));
        assert_eq!(retry_delay_ms(3), Some(4000));
        assert_eq!(retry_delay_ms(4), None);
    }

    #[test]
    fn offline_budget() {
        assert!(admit_offline_bytes(0, 1));
        assert!(!admit_offline_bytes(0, 0));
        assert!(!admit_offline_bytes(0, OFFLINE_AUDIO_MAX_FILE_BYTES + 1));
        assert!(!admit_offline_bytes(OFFLINE_AUDIO_MAX_BYTES, 1));
    }

    #[test]
    fn clamp_rounds() {
        assert_eq!(clamp_int(2.6, AYAH_REPEAT), 3);
        assert_eq!(clamp_int(99.0, AYAH_REPEAT), 20);
        assert_eq!(clamp_int(f64::INFINITY, DELAY_MS), 0);
    }
}
