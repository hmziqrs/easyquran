//! Time seam: wall-clock milliseconds, injected so sync stamps, sessions
//! and schedules test on a [`ManualClock`].

use std::{
    cell::Cell,
    time::{SystemTime, UNIX_EPOCH},
};

/// Wall-clock time source.
pub trait Clock {
    /// Milliseconds since the Unix epoch.
    fn unix_ms(&self) -> i64;
}

/// The system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    }
}

/// A clock tests set and advance by hand.
#[derive(Debug, Default)]
pub struct ManualClock {
    now: Cell<i64>,
}

impl ManualClock {
    pub fn new(unix_ms: i64) -> Self {
        Self {
            now: Cell::new(unix_ms),
        }
    }

    pub fn set(&self, unix_ms: i64) {
        self.now.set(unix_ms);
    }

    pub fn advance(&self, ms: i64) {
        self.now.set(self.now.get() + ms);
    }
}

impl Clock for ManualClock {
    fn unix_ms(&self) -> i64 {
        self.now.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_advances() {
        let clock = ManualClock::new(1_000);
        clock.advance(500);
        assert_eq!(clock.unix_ms(), 1_500);
        assert!(SystemClock.unix_ms() > 0);
    }
}
