//! The reader's reading-session glue (PARITY G1, reader side; web
//! `reader-thread.svelte.ts`, `reader-session.svelte.ts`, `ResumePill`,
//! the readers' `markRead` paths and `installReadTracker`). Every call is
//! an `eq_core::store::sessions` job on the sync store's blocking runner,
//! under `Area::Sessions` (the write bumps the area and schedules the
//! sessions flush).

use std::time::Duration;

use eq_core::store::{
    AppStore, StoreError,
    sessions::{self, EnterResult, ReaderTarget},
};
use nsh_core::prelude::*;

use super::tracker::{ReadEvent, ReadTracker, WINDOW_FLUSH_MS};
use crate::stores::sync::{Area, SyncStore};

/// `onScroll`'s gate between cursor writes (`now - lastMarkAt < 250`).
pub const CURSOR_THROTTLE: Duration = Duration::from_millis(250);
/// `ResumePill`'s `AUTO_HIDE_MS`.
pub const OFFER_HIDE: Duration = Duration::from_secs(9);

/// Set by the reader before it replaces its own route (next surah, page
/// links, resume): the next reader keeps the tab's thread
/// (`came_from_reader`).
#[derive(Clone, Copy, Default, PartialEq)]
pub struct FromReader(pub bool);

/// Reads and clears the flag.
pub fn take_from_reader() -> bool {
    let flag = global::<FromReader>();
    let was = flag.get_untracked().0;
    if was {
        flag.set(FromReader(false));
    }
    was
}

pub fn mark_from_reader() {
    global::<FromReader>().set(FromReader(true));
}

/// `sessions.enter` for the route, then the offer the store holds.
pub fn enter(
    sync: SyncStore,
    target: ReaderTarget,
    came_from_reader: bool,
    then: impl FnOnce(Option<(EnterResult, Option<String>)>) + 'static,
) {
    sync.write(
        Area::Sessions,
        move |store: &mut AppStore| {
            let result = sessions::enter(store, &target, None, came_from_reader)?;
            Ok((result, sessions::offer(store)))
        },
        move |out: Result<_, StoreError>| match out {
            Ok(pair) => then(Some(pair)),
            Err(e) => {
                log::warn!("reader: session enter failed: {e}");
                then(None);
            }
        },
    );
}

/// The cursor moved to `surah:ayah` (topmost verse on screen); a surah
/// reader also stores the fine anchor (`setLastReadAnchor`).
pub fn cursor(
    sync: SyncStore,
    surah: u16,
    ayah: u16,
    source_id: Option<String>,
    anchor: Option<(i64, f64)>,
) {
    sync.write(
        Area::Sessions,
        move |store: &mut AppStore| {
            sessions::cursor(store, i64::from(surah), i64::from(ayah), source_id)?;
            if let Some((page, ratio)) = anchor {
                sessions::anchor(store, page, ratio)?;
            }
            Ok(())
        },
        |out: Result<(), StoreError>| {
            if let Err(e) = out {
                log::warn!("reader: cursor not saved: {e}");
            }
        },
    );
}

/// A verse was read (the tracker counted it).
pub fn note_read(sync: SyncStore, surah: u16, ayah: u16) {
    sync.write(
        Area::Sessions,
        move |store: &mut AppStore| sessions::note_read(store, i64::from(surah), i64::from(ayah)),
        |out: Result<(), StoreError>| {
            if let Err(e) = out {
                log::warn!("reader: read not saved: {e}");
            }
        },
    );
}

/// "Start over": the fresh draft's place.
pub fn start_over(sync: SyncStore, then: impl FnOnce(Option<(u16, u16)>) + 'static) {
    sync.write(
        Area::Sessions,
        |store: &mut AppStore| sessions::start_over(store),
        move |out: Result<Option<sessions::ReadingThread>, StoreError>| {
            let place = out
                .ok()
                .flatten()
                .and_then(|t| Some((u16::try_from(t.surah).ok()?, u16::try_from(t.ayah).ok()?)));
            then(place);
        },
    );
}

pub fn dismiss_offer(sync: SyncStore) {
    sync.run(
        |store: &mut AppStore| {
            sessions::dismiss_offer(store);
            Ok(())
        },
        |_: Result<(), StoreError>| {},
    );
}

/// Throttles cursor writes: the first change writes at once, later ones
/// at most every [`CURSOR_THROTTLE`], the last one always lands.
#[derive(Clone, Copy)]
pub struct CursorThrottle {
    last_at: Stored<Option<nsh_core::time::Instant>>,
    pending: Stored<Option<Spawned<()>>>,
}

impl Default for CursorThrottle {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorThrottle {
    pub fn new() -> Self {
        Self {
            last_at: stored(None),
            pending: stored(None),
        }
    }

    pub fn push(self, write: impl FnOnce() + 'static) {
        if let Some(task) = self.pending.replace(None) {
            task.cancel();
        }
        let now = nsh_core::time::now();
        let wait = self
            .last_at
            .get()
            .map(|at| CURSOR_THROTTLE.saturating_sub(now.saturating_duration_since(at)))
            .unwrap_or_default();
        if wait.is_zero() {
            self.last_at.replace(Some(now));
            write();
            return;
        }
        let task = spawn(async move {
            timer(wait).await;
            self.last_at.replace(Some(nsh_core::time::now()));
            write();
        });
        self.pending.replace(Some(task));
    }
}

/// The read tracker wired to the reader: counted reads feed
/// `sessions::note_read`; each window's events go to [`emit_reads`].
#[derive(Clone, Copy)]
pub struct Tracking {
    tracker: Stored<ReadTracker>,
    sync: SyncStore,
}

fn now_ms() -> f64 {
    nsh_core::time::unix_ms()
}

impl Tracking {
    /// Made under the reader's owner: flushes its window every 10 s and
    /// on the way out.
    pub fn install(sync: SyncStore) -> Self {
        let this = Self {
            tracker: stored(ReadTracker::new(now_ms())),
            sync,
        };
        set_interval(Duration::from_millis(WINDOW_FLUSH_MS), move || {
            if this.tracker.with(|t| t.pending_count()) > 0 {
                this.flush_window();
            }
        });
        cleanup(move || {
            let now = now_ms();
            let flushed = this.tracker.try_with(|t| {
                let reads = t.flush_visible(now);
                (reads, t.flush_window(now))
            });
            if let Some((reads, events)) = flushed {
                this.noted(reads);
                emit_reads(&events);
            }
        });
        this
    }

    fn noted(self, reads: Vec<(u16, u16)>) {
        for (surah, ayah) in reads {
            note_read(self.sync, surah, ayah);
        }
    }

    fn flush_window(self) {
        let events = self.tracker.with(|t| t.flush_window(now_ms()));
        emit_reads(&events);
    }

    pub fn interaction(self) {
        self.tracker.with(|t| t.interaction(now_ms()));
    }

    /// Verses entering (`visible`) or leaving the viewport; `above`: left
    /// past the top. `letters` is read only for a verse seen first.
    pub fn intersect(self, verses: &[(u16, u16, usize)], visible: bool, above: bool) {
        let now = now_ms();
        let reads: Vec<(u16, u16)> = self.tracker.with(|t| {
            verses
                .iter()
                .filter_map(|&(surah, ayah, letters)| {
                    t.track(surah, ayah, letters);
                    t.intersect(surah, ayah, visible, above, now)
                })
                .collect()
        });
        self.noted(reads);
    }
}

/// Where a window's reads go: the reading-insights queue
/// (`readingAnalytics.emit`), which another slice owns; until it lands,
/// the events are logged at debug level.
pub fn emit_reads(events: &[ReadEvent]) {
    // Thread and source ids are not threaded through the tracker yet; the
    // queue accepts their absence (web sends both when it has them).
    for e in events {
        crate::stores::insights::emit(crate::stores::insights::ReadInput {
            session_id: None,
            surah: e.surah,
            ayah: e.ayah,
            source_id: None,
            ms: e.ms,
        });
    }
}
