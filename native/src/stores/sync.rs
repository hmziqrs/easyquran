//! The sync store (PARITY L1, L2): the local app database and the sync
//! engine wired to signals. One `Copy` handle, made once at the root and
//! handed down by context ([`provide`], [`sync_store`]).
//!
//! - Every store call runs through [`blocking`] (SQLite never touches the
//!   engine thread); results come back on the engine thread to callbacks.
//! - Writes bump the per-area change version screens subscribe to, then
//!   schedule a flush of that area's domain (2 s; reading threads 4 s, so a
//!   burst of cursor moves is one round, as on the web).
//! - Polling follows [`eq_core::sync::engine::should_run`]: a backoff retry
//!   when due, every 30 s while the app is active. Becoming active again
//!   (resume, wake) flushes at once, which also refreshes the clock offset.
//!   Nothing depends on background work.
//!
//! The HTTP side comes from the API client through [`SyncStore::set_transport`].

use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use eq_core::{
    clock::SystemClock,
    store::{AppStore, SignInReport, StoreError},
    sync::{
        SyncDomain,
        engine::{
            self, FlushOptions, PollInput, RoundStatus, StoreFuture, StoreJob, StoreRunner,
            SyncEngine, store_counts,
        },
        ids::OsRandom,
        round_status::ClientInfo,
        transport::SyncPost,
    },
};
use nsh_core::{platform::Lifecycle, prelude::*};
use serde_json::Value;

/// How often the poll cadence is checked.
const POLL_TICK: Duration = Duration::from_secs(5);
/// A local edit flushes its domain after this quiet time.
const EDIT_FLUSH_DELAY: Duration = Duration::from_secs(2);
/// Cursor moves coalesce longer: scrolling marks many verses in seconds.
const SESSION_FLUSH_DELAY: Duration = Duration::from_secs(4);
/// The database folder under the app's data directory.
const DB_FOLDER: &str = "sync";

/// What a screen subscribes to: one change version per area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Bookmarks,
    Sessions,
    /// Settings registers and the reading bookmark.
    Preferences,
    Goals,
    /// Home layout and cards.
    Home,
}

impl Area {
    const ALL: [Area; 5] = [
        Area::Bookmarks,
        Area::Sessions,
        Area::Preferences,
        Area::Goals,
        Area::Home,
    ];

    fn index(self) -> usize {
        match self {
            Area::Bookmarks => 0,
            Area::Sessions => 1,
            Area::Preferences => 2,
            Area::Goals => 3,
            Area::Home => 4,
        }
    }

    /// The domain whose round carries this area (cards never sync, but the
    /// layout in the same area does).
    fn domain(self) -> SyncDomain {
        match self {
            Area::Bookmarks => SyncDomain::Bookmarks,
            Area::Sessions => SyncDomain::Sessions,
            Area::Preferences | Area::Goals | Area::Home => SyncDomain::Preferences,
        }
    }
}

type Db = Arc<Mutex<AppStore>>;

struct Shared {
    db: Option<Db>,
    engine: Rc<RefCell<SyncEngine>>,
    post: Option<Rc<dyn SyncPost>>,
    online: bool,
    /// One pending edit-flush per domain.
    scheduled: Vec<(SyncDomain, Spawned<()>)>,
    last_phase: Option<Lifecycle>,
}

/// Runs store jobs on the runtime's blocking threads.
struct BlockingRunner(Db);

fn lock_and_run<T>(db: &Db, job: StoreJob<T>) -> Result<T, StoreError> {
    let mut guard = db
        .lock()
        .map_err(|_| StoreError::Unavailable("store lock poisoned".into()))?;
    Ok(job(&mut guard))
}

impl StoreRunner for BlockingRunner {
    fn run<T: Send + 'static>(&self, job: StoreJob<T>) -> StoreFuture<'_, Result<T, StoreError>> {
        let db = self.0.clone();
        Box::pin(run_job(db, job))
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_job<T: Send + 'static>(db: Db, job: StoreJob<T>) -> Result<T, StoreError> {
    blocking(move || lock_and_run(&db, job))
        .await
        .map_err(|e| StoreError::Unavailable(e.to_string()))?
}

#[cfg(target_arch = "wasm32")]
async fn run_job<T: Send + 'static>(db: Db, job: StoreJob<T>) -> Result<T, StoreError> {
    lock_and_run(&db, job)
}

fn now_ms() -> i64 {
    nsh_core::time::unix_ms() as i64
}

/// 0..1 for retry jitter, from the clock's sub-second part.
fn jitter() -> f64 {
    (nsh_core::time::unix_ms().fract() * 997.0).fract()
}

/// The sync store handle.
#[derive(Clone, Copy)]
pub struct SyncStore {
    status: Signal<RoundStatus>,
    versions: [Signal<u64>; 5],
    settings_inbox: Signal<Vec<(String, Value)>>,
    capture_due: Signal<u64>,
    open_error: Signal<Option<String>>,
    ready: Signal<bool>,
    shared: Stored<Shared>,
}

impl SyncStore {
    /// Makes the store's signals under the current owner (the root).
    pub fn new(client: ClientInfo) -> Self {
        Self {
            status: signal(RoundStatus::default()),
            versions: [signal(0), signal(0), signal(0), signal(0), signal(0)],
            settings_inbox: signal(Vec::new()),
            capture_due: signal(0),
            open_error: signal(None),
            ready: signal(false),
            shared: stored(Shared {
                db: None,
                engine: Rc::new(RefCell::new(SyncEngine::new(client))),
                post: None,
                online: true,
                scheduled: Vec::new(),
                last_phase: None,
            }),
        }
    }

    /// Opens the database under `<data_dir>/sync` (in memory where the host
    /// keeps no files), off the engine thread. `ready` turns true once open.
    pub fn open(self) {
        let dir: Option<PathBuf> = data_dir().map(|d| d.join(DB_FOLDER));
        let this = self;
        spawn(async move {
            match open_db(dir).await {
                Ok(store) => {
                    this.shared
                        .with(|s| s.db = Some(Arc::new(Mutex::new(store))));
                    this.ready.set(true);
                    this.bump_all();
                    this.refresh_status();
                }
                Err(err) => this.open_error.set(Some(err.to_string())),
            }
        });
    }

    /// The API client's sync transport. Until set, flushes do nothing.
    pub fn set_transport(self, post: Rc<dyn SyncPost>) {
        self.shared.with(|s| s.post = Some(post));
    }

    /// Wires the poll timer and the resume trigger. Call once at the root.
    pub fn start(self) {
        set_interval(POLL_TICK, move || self.poll());
        effect(move || {
            let phase = lifecycle();
            let previous = self.shared.with(|s| s.last_phase.replace(phase));
            // Resume or wake: pull now. The round refreshes the clock offset.
            let resumed =
                phase == Lifecycle::Active && previous.is_some_and(|p| p != Lifecycle::Active);
            if resumed {
                untracked(|| self.kick());
            }
        });
    }

    // --- reads ---

    /// The round status (tracked).
    pub fn status(self) -> RoundStatus {
        self.status.get()
    }

    /// An area's change version (tracked): re-read that area's data when it moves.
    pub fn version(self, area: Area) -> u64 {
        self.versions[area.index()].get()
    }

    /// True once the database is open (tracked).
    pub fn ready(self) -> bool {
        self.ready.get()
    }

    /// Why the database could not open (tracked).
    pub fn open_error(self) -> Option<String> {
        self.open_error.get()
    }

    /// `settings.*` values that arrived from another device (tracked). The
    /// settings store applies them and calls [`SyncStore::take_settings`].
    pub fn settings_inbox(self) -> Vec<(String, Value)> {
        self.settings_inbox.get()
    }

    pub fn take_settings(self) -> Vec<(String, Value)> {
        let taken = self.settings_inbox.get_untracked();
        if !taken.is_empty() {
            self.settings_inbox.set(Vec::new());
        }
        taken
    }

    /// Moves when the account's first full preference pull landed (tracked):
    /// the settings store then captures its current values
    /// (`eq_core::store::preferences::capture`).
    pub fn capture_due(self) -> u64 {
        self.capture_due.get()
    }

    // --- store calls ---

    fn db(self) -> Option<Db> {
        self.shared.with(|s| s.db.clone())
    }

    /// Runs `job` on a blocking thread, then `then` with its result on the
    /// engine thread. Reads use this directly.
    pub fn run<T: Send + 'static>(
        self,
        job: impl FnOnce(&mut AppStore) -> Result<T, StoreError> + Send + 'static,
        then: impl FnOnce(Result<T, StoreError>) + 'static,
    ) {
        let Some(db) = self.db() else {
            then(Err(StoreError::Unavailable("store not open".into())));
            return;
        };
        spawn(async move {
            let out = run_job(db, Box::new(job)).await.and_then(|r| r);
            then(out);
        });
    }

    /// A write: runs `job`, then bumps `area`'s version and schedules its
    /// domain's flush (the dirty rows are already on disk).
    pub fn write<T: Send + 'static>(
        self,
        area: Area,
        job: impl FnOnce(&mut AppStore) -> Result<T, StoreError> + Send + 'static,
        then: impl FnOnce(Result<T, StoreError>) + 'static,
    ) {
        self.run(job, move |out| {
            self.bump(area);
            self.refresh_status();
            if out.is_ok() {
                self.schedule_flush(area.domain());
            }
            then(out);
        });
    }

    /// Sign-in or account switch: folds signed-out rows in, then syncs.
    pub fn sign_in(
        self,
        user_id: i64,
        device_id: String,
        then: impl FnOnce(Result<SignInReport, StoreError>) + 'static,
    ) {
        self.shared
            .with(|s| s.engine.borrow_mut().reset_for_account());
        self.run(
            move |store| store.sign_in(user_id, &device_id),
            move |out| {
                self.bump_all();
                self.kick();
                then(out);
            },
        );
    }

    /// Sign-out, expiry, revoke: the account's dirty rows wait in its file.
    pub fn sign_out(self) {
        self.cancel_scheduled();
        self.shared
            .with(|s| s.engine.borrow_mut().reset_for_account());
        self.run(
            |store| {
                store.sign_out();
                Ok(())
            },
            move |_| {
                self.bump_all();
                self.refresh_status();
            },
        );
    }

    /// Account deleted: its file goes, dirty rows included.
    pub fn purge(self, user_id: i64) {
        self.cancel_scheduled();
        self.run(
            move |store| store.purge_account(user_id),
            move |_| {
                self.bump_all();
                self.refresh_status();
            },
        );
    }

    // --- sync triggers ---

    /// Flush every domain now.
    pub fn kick(self) {
        self.flush(None);
    }

    /// Flush one domain now.
    pub fn kick_domain(self, domain: SyncDomain) {
        self.flush(Some(domain));
    }

    /// Connectivity changed. Coming online resets backoff and flushes.
    pub fn set_online(self, online: bool) {
        let was = self
            .shared
            .with(|s| std::mem::replace(&mut s.online, online));
        if online && !was {
            self.shared.with(|s| s.engine.borrow_mut().note_online());
            self.kick();
        }
    }

    /// The session's account turned out not to be this one: the auth store
    /// re-probes, then calls this.
    pub fn clear_session_lost(self) {
        self.shared
            .with(|s| s.engine.borrow_mut().clear_session_lost());
        self.refresh_status();
    }

    fn poll(self) {
        let (engine, online) = self.shared.with(|s| (s.engine.clone(), s.online));
        let input = PollInput {
            now: now_ms(),
            focused: lifecycle() == Lifecycle::Active,
            online,
            signed_in: self.status.get_untracked().signed_in,
        };
        if engine::should_run(&engine.borrow(), input).is_some() {
            self.kick();
        }
    }

    fn schedule_flush(self, domain: SyncDomain) {
        let delay = match domain {
            SyncDomain::Sessions => SESSION_FLUSH_DELAY,
            _ => EDIT_FLUSH_DELAY,
        };
        let task = set_timeout(delay, move || self.kick_domain(domain));
        let previous = self.shared.with(|s| {
            let at = s.scheduled.iter().position(|(d, _)| *d == domain);
            let old = at.map(|i| s.scheduled.remove(i).1);
            s.scheduled.push((domain, task));
            old
        });
        if let Some(old) = previous {
            old.cancel();
        }
    }

    fn cancel_scheduled(self) {
        let tasks = self.shared.with(|s| std::mem::take(&mut s.scheduled));
        for (_, task) in tasks {
            task.cancel();
        }
    }

    fn flush(self, only: Option<SyncDomain>) {
        let (db, engine, post, online) = self
            .shared
            .with(|s| (s.db.clone(), s.engine.clone(), s.post.clone(), s.online));
        let (Some(db), Some(post)) = (db, post) else {
            return;
        };
        spawn(async move {
            let runner = BlockingRunner(db);
            let now = now_ms;
            let report = engine::flush(
                &engine,
                &runner,
                &*post,
                FlushOptions {
                    now: &now,
                    online,
                    jitter: jitter(),
                    only,
                },
            )
            .await;
            if report.coalesced {
                return;
            }
            if !report.settings.is_empty() {
                self.settings_inbox
                    .update(|inbox| inbox.extend(report.settings.iter().cloned()));
            }
            if report.capture_settings {
                self.capture_due.update(|n| *n += 1);
            }
            // A pull may have changed any area's rows.
            self.bump_all();
            self.refresh_status();
        });
    }

    fn bump(self, area: Area) {
        self.versions[area.index()].update(|v| *v += 1);
    }

    fn bump_all(self) {
        for area in Area::ALL {
            self.bump(area);
        }
    }

    fn refresh_status(self) {
        let engine = self.shared.with(|s| s.engine.clone());
        self.run(
            |store| store_counts(store),
            move |counts| {
                let counts = counts.unwrap_or_default();
                self.status.set(engine.borrow().status(counts, now_ms()));
            },
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn open_db(dir: Option<PathBuf>) -> Result<AppStore, StoreError> {
    blocking(move || open_now(dir))
        .await
        .map_err(|e| StoreError::Unavailable(e.to_string()))?
}

#[cfg(target_arch = "wasm32")]
async fn open_db(dir: Option<PathBuf>) -> Result<AppStore, StoreError> {
    open_now(dir)
}

fn open_now(dir: Option<PathBuf>) -> Result<AppStore, StoreError> {
    match dir {
        Some(dir) => AppStore::open(dir, Box::new(SystemClock), Box::new(OsRandom::default())),
        None => AppStore::open_in_memory(Box::new(SystemClock), Box::new(OsRandom::default())),
    }
}

/// Makes the sync store available to every screen below the caller.
pub fn provide(store: SyncStore) {
    provide_context(store);
}

/// The sync store [`provide`] made available.
pub fn sync_store() -> Option<SyncStore> {
    use_context::<SyncStore>()
}
