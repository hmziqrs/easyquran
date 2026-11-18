//! The app's time: thin readers over the engine's clock, so everything
//! here follows a test's manual clock instead of the wall.

use nsh_core::time;

/// Milliseconds since the Unix epoch on the engine's clock (a test's
/// manual one moves only when the test moves it).
pub fn now_ms() -> f64 {
    time::unix_ms()
}

/// A monotonic point on the engine's clock, for measuring durations. The
/// engine's own `Instant`, not `std`'s: on wasm the engine type is a
/// `Duration`-backed struct, so `std::time::Instant` does not exist there.
pub fn now() -> time::Instant {
    time::now()
}

/// Milliseconds from `start` to now (`elapsed` is a `std::time::Duration`
/// on every target, native and wasm alike).
pub fn elapsed_ms(start: time::Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// `ms` as a UTC `"HH:MM:SS"` wall-time string (pure, so diagnostics and
/// log lines format identically on every platform).
pub fn format_time(ms: f64) -> String {
    let secs = (ms.max(0.0) / 1000.0).floor() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `format_time` returned an empty string — failed on `assert_eq!` with "00:00:00".
    #[test]
    fn formats_utc_wall_time() {
        assert_eq!(format_time(0.0), "00:00:00");
        assert_eq!(format_time(3_661_000.0), "01:01:01");
        // Before the epoch clamps to midnight rather than wrapping.
        assert_eq!(format_time(-5.0), "00:00:00");
    }
}
