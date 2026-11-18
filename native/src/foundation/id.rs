//! Id generation with no external crates: a wall-time prefix from the
//! engine's clock plus a process counter.

use std::sync::atomic::{AtomicU64, Ordering};

use nsh_core::element::SharedString;

/// The next process-unique number, increasing across the app's run.
pub fn next_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// An id like `"post-1730000000000-7"`: the wall time the engine's clock
/// reports (a test's manual clock, so a test's ids are reproducible) and a
/// counter that keeps same-millisecond ids apart.
pub fn new_id(prefix: &str) -> SharedString {
    format!(
        "{prefix}-{}-{}",
        crate::foundation::time::now_ms() as u64,
        next_id()
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `next_id` returned the constant 1 — failed on `second > first`.
    #[test]
    fn counter_ids_increase() {
        let first = next_id();
        let second = next_id();
        assert!(second > first);
    }
}
