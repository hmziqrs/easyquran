//! Read tracking (PARITY C12; web `lib/analytics/read-tracker.ts`): a
//! verse counts as read once it stayed on screen at least
//! `max(150 ms, 10 ms per Arabic letter)` (a hafiz reading fast is never
//! penalised: the floor is a skim, not a reading speed) and then left past
//! the top of the viewport, or is still on screen at a flush. A verse is
//! counted at most once per 30 s. Active time (interactions within the
//! last 5 min, while visible) is split across the verses read in each
//! window by letters and handed to the emit hook.
//!
//! Plain Rust over injected milliseconds, no engine types: the reader
//! feeds it the list's viewable rows (`intersect`) and activity.
//! Domain logic that would sit in `eq-core`; it lives here because this
//! slice may not edit `eq-core` (report: move it to `eq_core::reading`).

use std::collections::HashMap;

pub const MIN_DWELL_MS: f64 = 150.0;
pub const MS_PER_LETTER: f64 = 10.0;
pub const IDLE_MS: f64 = 5.0 * 60.0 * 1000.0;
const REPEAT_GAP_MS: f64 = 30_000.0;
const FALLBACK_LETTERS: usize = 56;
/// How often a window of reads is flushed (`WINDOW_FLUSH_MS`).
pub const WINDOW_FLUSH_MS: u64 = 10_000;

/// `dwellFloorMs`.
pub fn dwell_floor_ms(letters: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let by_letters = letters as f64 * MS_PER_LETTER;
    MIN_DWELL_MS.max(by_letters)
}

/// `arabicLetterCount`: letters in U+0621-064A and U+0671-06D3.
pub fn arabic_letter_count(text: &str) -> usize {
    text.chars()
        .filter(|c| matches!(*c, '\u{0621}'..='\u{064A}' | '\u{0671}'..='\u{06D3}'))
        .count()
}

/// Letters of a verse for the dwell floor, with the web's fallback.
pub fn letters_or_fallback(text: &str) -> usize {
    match arabic_letter_count(text) {
        0 => FALLBACK_LETTERS,
        n => n,
    }
}

#[derive(Debug, Clone)]
struct Watch {
    surah: u16,
    ayah: u16,
    letters: usize,
    in_view: bool,
    visible_since: Option<f64>,
    visible_ms: f64,
}

/// One counted read waiting for its share of the window's active time.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadEvent {
    pub surah: u16,
    pub ayah: u16,
    pub letters: usize,
    /// Active milliseconds credited to this verse.
    pub ms: f64,
}

/// The tracker state (`ReadTracker`). `on_read` runs at each counted
/// read; `flush_window` returns the window's events.
#[derive(Debug, Default)]
pub struct ReadTracker {
    watches: HashMap<(u16, u16), Watch>,
    last_counted: HashMap<(u16, u16), f64>,
    pending: Vec<(u16, u16, usize)>,
    active_ms: f64,
    last_tick: f64,
    last_interaction: f64,
    hidden: bool,
}

impl ReadTracker {
    pub fn new(now: f64) -> Self {
        Self {
            last_tick: now,
            last_interaction: now,
            ..Self::default()
        }
    }

    fn tick(&mut self, now: f64) {
        if !self.hidden {
            let active_until = now.min(self.last_interaction + IDLE_MS);
            self.active_ms += (active_until - self.last_tick).max(0.0);
        }
        self.last_tick = now;
    }

    fn dwell_now(&self, now: f64) -> f64 {
        now.min(self.last_interaction + IDLE_MS)
    }

    /// Scroll, drag, press: the reader is active.
    pub fn interaction(&mut self, now: f64) {
        self.tick(now);
        self.last_interaction = now;
    }

    /// Starts watching a verse (its row was laid out).
    pub fn track(&mut self, surah: u16, ayah: u16, letters: usize) {
        self.watches.entry((surah, ayah)).or_insert(Watch {
            surah,
            ayah,
            letters,
            in_view: false,
            visible_since: None,
            visible_ms: 0.0,
        });
    }

    /// The verse entered or left the viewport; `above`: it left past the
    /// top (read on), not the bottom (scrolled back). Returns a counted
    /// read.
    pub fn intersect(
        &mut self,
        surah: u16,
        ayah: u16,
        visible: bool,
        above: bool,
        now: f64,
    ) -> Option<(u16, u16)> {
        let dwell = self.dwell_now(now);
        let hidden = self.hidden;
        let watch = self.watches.get_mut(&(surah, ayah))?;
        watch.in_view = visible;
        if visible {
            if watch.visible_since.is_none() && !hidden {
                watch.visible_since = Some(dwell);
            }
            return None;
        }
        watch.visible_since?;
        self.leave((surah, ayah), above, now)
    }

    fn leave(&mut self, key: (u16, u16), counts: bool, now: f64) -> Option<(u16, u16)> {
        let dwell = self.dwell_now(now);
        let watch = self.watches.get_mut(&key)?;
        if let Some(since) = watch.visible_since.take() {
            watch.visible_ms += (dwell - since).max(0.0);
        }
        let counted = if counts {
            self.maybe_count(key, now)
        } else {
            None
        };
        if let Some(w) = self.watches.get_mut(&key) {
            w.visible_ms = 0.0;
        }
        counted
    }

    fn maybe_count(&mut self, key: (u16, u16), now: f64) -> Option<(u16, u16)> {
        let watch = self.watches.get(&key)?.clone();
        if watch.visible_ms < dwell_floor_ms(watch.letters) {
            return None;
        }
        if self
            .last_counted
            .get(&key)
            .is_some_and(|last| now - last < REPEAT_GAP_MS)
        {
            return None;
        }
        self.last_counted.insert(key, now);
        if self.pending.is_empty() {
            // A new window starts here: time before the first read was browsing.
            self.tick(now);
            self.active_ms = self.active_ms.min(watch.visible_ms);
        }
        self.pending.push((watch.surah, watch.ayah, watch.letters));
        Some((watch.surah, watch.ayah))
    }

    /// The window lost or regained visibility (app to background).
    pub fn set_visible(&mut self, visible: bool, now: f64) -> Vec<(u16, u16)> {
        self.tick(now);
        let mut counted = Vec::new();
        if !visible {
            counted = self.flush_visible(now);
        }
        self.hidden = !visible;
        if visible {
            self.last_interaction = now;
        }
        let dwell = self.dwell_now(now);
        for watch in self.watches.values_mut() {
            match (visible, watch.visible_since) {
                (false, Some(since)) => {
                    watch.visible_ms += (dwell - since).max(0.0);
                    watch.visible_since = None;
                }
                (true, None) if watch.in_view => watch.visible_since = Some(now),
                _ => {}
            }
        }
        counted
    }

    /// Counts every verse still on screen that already met its floor.
    pub fn flush_visible(&mut self, now: f64) -> Vec<(u16, u16)> {
        let dwell = self.dwell_now(now);
        let hidden = self.hidden;
        let keys: Vec<(u16, u16)> = self
            .watches
            .iter()
            .filter(|(_, w)| w.visible_since.is_some())
            .map(|(k, _)| *k)
            .collect();
        let mut counted = Vec::new();
        for key in keys {
            if let Some(w) = self.watches.get_mut(&key)
                && let Some(since) = w.visible_since
            {
                w.visible_ms += (dwell - since).max(0.0);
                w.visible_since = (!hidden).then_some(dwell);
            }
            if let Some(read) = self.maybe_count(key, now) {
                counted.push(read);
                if let Some(w) = self.watches.get_mut(&key) {
                    w.visible_ms = 0.0;
                }
            }
        }
        counted
    }

    /// Closes the window: its active time split by letters.
    pub fn flush_window(&mut self, now: f64) -> Vec<ReadEvent> {
        self.tick(now);
        let pending = std::mem::take(&mut self.pending);
        let budget = self.active_ms;
        self.active_ms = 0.0;
        #[allow(clippy::cast_precision_loss)]
        let total: f64 = pending.iter().map(|p| p.2.max(1) as f64).sum();
        pending
            .into_iter()
            .map(|(surah, ayah, letters)| {
                #[allow(clippy::cast_precision_loss)]
                let share = letters.max(1) as f64;
                ReadEvent {
                    surah,
                    ayah,
                    letters,
                    ms: budget * share / total,
                }
            })
            .collect()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}
