//! Local app database (PARITY F, G, J9, L2): one SQLite file for signed-out
//! and device-wide data, one per account (contracts/sync.md "The row is the
//! queue"). Blocking I/O: the app runs every call off the engine thread.
//!
//! File layout under the app's data directory (`sync/`):
//!
//! - `local.sqlite`: the signed-out owner's rows (bookmarks, folders,
//!   threads, progress, reading bookmark, goals) plus device-wide documents
//!   (home layout, home cards).
//! - `account-{userId}.sqlite`: one account's rows with stamps and `dirty`,
//!   its sync positions, clock and documents. It stays after sign-out (dirty
//!   rows wait for that account), and goes on [`AppStore::purge_account`].
//!
//! Signing in folds the signed-out rows into the account silently, as the
//! web does: bookmarks (device default folder onto the account default),
//! threads and progress with stamps from their own time, the reading
//! bookmark with its `setAt`. Goals stay per owner.

pub mod bookmarks;
pub mod export;
pub mod goals;
pub mod home;
pub(crate) mod position;
pub mod preferences;
pub(crate) mod rows;
pub mod schema;
pub mod sessions;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use rusqlite::Connection;
use serde_json::Value;

use self::{
    rows::{kv_get, kv_json, kv_set, kv_set_json},
    schema::SchemaState,
};
use crate::{
    clock::Clock,
    sync::{
        SyncDomain,
        clock::{ClockState, HybridClock},
        ids::{Randomness, UuidV7Generator, uuid_v4},
    },
};

/// A clean row of an account nobody signed into for this long is dropped.
pub const IDLE_ACCOUNT_MS: i64 = 180 * 24 * 60 * 60 * 1000;
const LOCAL_FILE: &str = "local.sqlite";
const CLOCK_KEY: &str = "sync.clock";
const LAST_SEEN_KEY: &str = "account.lastSeenAt";

/// Why a store call did not land.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// SQLite failed (disk full, I/O, corrupt file). The store reports
    /// itself degraded until a write lands again.
    #[error("storage failed: {0}")]
    Sqlite(String),
    /// The file was written by a newer build: left untouched, sync paused.
    #[error("local data is from a newer version of the app")]
    FutureSchema,
    /// The call needs an account.
    #[error("signed out")]
    SignedOut,
    /// The account changed between a round's request and its response.
    #[error("account changed")]
    AccountChanged,
    /// A response that does not decode.
    #[error("unreadable: {0}")]
    Unreadable(String),
    /// The store could not be reached (the app's worker failed).
    #[error("store unavailable: {0}")]
    Unavailable(String),
}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        StoreError::Sqlite(err.to_string())
    }
}

/// What signing in did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignInReport {
    /// Signed-out bookmarks that joined the account ("Added N bookmarks from this device").
    pub absorbed_bookmarks: usize,
    pub absorbed_threads: usize,
    /// The account's file is from a newer build: sync stays paused.
    pub future_schema: bool,
}

enum Location {
    Dir(PathBuf),
    Memory,
}

pub(crate) struct Account {
    pub(crate) user_id: i64,
    pub(crate) conn: Connection,
    pub(crate) clock: HybridClock,
    pub(crate) future: bool,
}

/// Per-owner sync state that lives in memory only (web: fields of the stores).
#[derive(Debug, Default)]
pub(crate) struct SyncMemory {
    /// Rows the server answered `capped`: they wait behind the others.
    pub(crate) capped: HashMap<SyncDomain, HashSet<String>>,
    /// Past the tombstone floor: ids a full pull named so far.
    pub(crate) full_pull_seen: HashMap<SyncDomain, HashSet<String>>,
    pub(crate) update_required: bool,
    /// The first full preference pull landed: the app should capture settings.
    pub(crate) settings_capture_due: bool,
}

/// The local app database. `Send`, so the app keeps it behind a mutex and
/// runs calls on a blocking thread.
pub struct AppStore {
    location: Location,
    memory_accounts: HashMap<i64, Connection>,
    local: Connection,
    local_future: bool,
    pub(crate) account: Option<Account>,
    clock: Box<dyn Clock + Send>,
    random: Box<dyn Randomness + Send>,
    ids: UuidV7Generator,
    degraded: bool,
    pub(crate) sync_memory: SyncMemory,
    pub(crate) tab: sessions::Tab,
}

impl std::fmt::Debug for AppStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppStore")
            .field("account", &self.account_id())
            .field("degraded", &self.degraded)
            .finish_non_exhaustive()
    }
}

/// The account file for `user_id` under `dir`.
pub fn account_file(dir: &Path, user_id: i64) -> PathBuf {
    dir.join(format!("account-{user_id}.sqlite"))
}

fn open_file(path: &Path) -> Result<(Connection, bool), StoreError> {
    let conn = Connection::open(path)?;
    let future = matches!(schema::migrate(&conn, true)?, SchemaState::Future(_));
    Ok((conn, future))
}

fn open_memory() -> Result<(Connection, bool), StoreError> {
    let conn = Connection::open_in_memory()?;
    let future = matches!(schema::migrate(&conn, false)?, SchemaState::Future(_));
    Ok((conn, future))
}

fn remove_file_set(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(name));
    }
}

impl AppStore {
    /// Opens (and migrates) `dir/local.sqlite`. Creates `dir` if needed.
    pub fn open(
        dir: impl Into<PathBuf>,
        clock: Box<dyn Clock + Send>,
        random: Box<dyn Randomness + Send>,
    ) -> Result<Self, StoreError> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(|e| StoreError::Sqlite(e.to_string()))?;
        let (local, local_future) = open_file(&dir.join(LOCAL_FILE))?;
        Ok(Self::with(
            Location::Dir(dir),
            local,
            local_future,
            clock,
            random,
        ))
    }

    /// An in-memory store (tests). Accounts keep their data across sign-outs
    /// for the life of the store, as files would.
    pub fn open_in_memory(
        clock: Box<dyn Clock + Send>,
        random: Box<dyn Randomness + Send>,
    ) -> Result<Self, StoreError> {
        let (local, local_future) = open_memory()?;
        Ok(Self::with(
            Location::Memory,
            local,
            local_future,
            clock,
            random,
        ))
    }

    fn with(
        location: Location,
        local: Connection,
        local_future: bool,
        clock: Box<dyn Clock + Send>,
        random: Box<dyn Randomness + Send>,
    ) -> Self {
        let mut store = Self {
            location,
            memory_accounts: HashMap::new(),
            local,
            local_future,
            account: None,
            clock,
            random,
            ids: UuidV7Generator::new(),
            degraded: false,
            sync_memory: SyncMemory::default(),
            tab: sessions::Tab::default(),
        };
        store.tab = sessions::Tab::load(&store);
        store
    }

    // --- owner ---

    /// The signed-in account, or `None` signed out.
    pub fn account_id(&self) -> Option<i64> {
        self.account.as_ref().map(|a| a.user_id)
    }

    /// This install's device id for the signed-in account.
    pub fn device_id(&self) -> Option<&str> {
        self.account.as_ref().map(|a| a.clock.device_id())
    }

    /// The account's file is from a newer build (sync paused, writes refused).
    pub fn future_schema(&self) -> bool {
        match &self.account {
            Some(account) => account.future,
            None => self.local_future,
        }
    }

    /// A durable write failed: never report those rows as synced.
    pub fn degraded(&self) -> bool {
        self.degraded
    }

    /// Sign-in or account switch. `device_id` is this install's id for the
    /// account (the device module mints and keeps it). Folds signed-out rows
    /// into the account when coming from signed out.
    pub fn sign_in(&mut self, user_id: i64, device_id: &str) -> Result<SignInReport, StoreError> {
        if self.account_id() == Some(user_id) {
            return Ok(SignInReport::default());
        }
        let from_local = self.account.is_none();
        self.sign_out();
        let (conn, future) = self.open_account(user_id)?;
        let state: ClockState = if future {
            ClockState::default()
        } else {
            kv_json(&conn, CLOCK_KEY)?
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default()
        };
        self.account = Some(Account {
            user_id,
            conn,
            clock: HybridClock::new(device_id, state),
            future,
        });
        self.sync_memory = SyncMemory::default();
        let mut report = SignInReport {
            future_schema: future,
            ..SignInReport::default()
        };
        if future {
            self.tab = sessions::Tab::default();
            return Ok(report);
        }
        let now = self.local_now();
        self.track(kv_set(self.owner_conn(), LAST_SEEN_KEY, &now.to_string()))?;
        if from_local && !self.local_future {
            report.absorbed_bookmarks = bookmarks::absorb_local(self)?;
            report.absorbed_threads = sessions::absorb_local(self)?;
            preferences::absorb_local(self)?;
        }
        self.prune_idle_accounts()?;
        self.tab = sessions::Tab::load(self);
        Ok(report)
    }

    /// Sign-out, expiry, revoke: the account's rows (dirty ones included) stay
    /// in its file for its next sign-in.
    pub fn sign_out(&mut self) {
        if let Some(account) = self.account.take()
            && matches!(self.location, Location::Memory)
        {
            self.memory_accounts.insert(account.user_id, account.conn);
        }
        self.sync_memory = SyncMemory::default();
        self.tab = sessions::Tab::load(self);
    }

    /// Account deleted or purged: its file goes, dirty rows included.
    pub fn purge_account(&mut self, user_id: i64) -> Result<(), StoreError> {
        if self.account_id() == Some(user_id) {
            self.account = None;
            self.sync_memory = SyncMemory::default();
            self.tab = sessions::Tab::load(self);
        }
        match &self.location {
            Location::Dir(dir) => remove_file_set(&account_file(dir, user_id)),
            Location::Memory => {
                self.memory_accounts.remove(&user_id);
            }
        }
        Ok(())
    }

    /// Everything this install holds: every account file and the local file's rows and documents.
    pub fn wipe_device(&mut self) -> Result<(), StoreError> {
        self.account = None;
        self.sync_memory = SyncMemory::default();
        match &self.location {
            Location::Dir(dir) => {
                for user_id in account_ids_in(dir) {
                    remove_file_set(&account_file(dir, user_id));
                }
            }
            Location::Memory => self.memory_accounts.clear(),
        }
        if !self.local_future {
            self.local
                .execute_batch("DELETE FROM rows; DELETE FROM kv;")?;
        }
        self.tab = sessions::Tab::default();
        Ok(())
    }

    fn open_account(&mut self, user_id: i64) -> Result<(Connection, bool), StoreError> {
        match &self.location {
            Location::Dir(dir) => open_file(&account_file(dir, user_id)),
            Location::Memory => match self.memory_accounts.remove(&user_id) {
                Some(conn) => {
                    let future = matches!(schema::migrate(&conn, false)?, SchemaState::Future(_));
                    Ok((conn, future))
                }
                None => open_memory(),
            },
        }
    }

    /// Other accounts' clean rows go after 180 days away; dirty rows wait for
    /// sign-in. Their pull positions reset so the dropped rows come back.
    fn prune_idle_accounts(&mut self) -> Result<(), StoreError> {
        let now = self.local_now();
        let current = self.account_id();
        let idle = |conn: &Connection| -> Result<bool, StoreError> {
            let seen = kv_get(conn, LAST_SEEN_KEY)?.and_then(|v| v.parse::<i64>().ok());
            Ok(seen.is_some_and(|seen| seen > 0 && now - seen >= IDLE_ACCOUNT_MS))
        };
        let prune = |conn: &Connection| -> Result<(), StoreError> {
            rows::in_tx(conn, |tx| {
                rows::delete_clean(tx)?;
                tx.execute(
                    "DELETE FROM kv WHERE key LIKE 'sync.%.since' OR key LIKE 'sync.%.after'",
                    [],
                )?;
                Ok(())
            })
        };
        match &self.location {
            Location::Dir(dir) => {
                for user_id in account_ids_in(dir) {
                    if Some(user_id) == current {
                        continue;
                    }
                    let Ok((conn, false)) = open_file(&account_file(dir, user_id)) else {
                        continue;
                    };
                    if idle(&conn)? {
                        prune(&conn)?;
                    }
                }
            }
            Location::Memory => {
                for conn in self.memory_accounts.values() {
                    if idle(conn)? {
                        prune(conn)?;
                    }
                }
            }
        }
        Ok(())
    }

    // --- connections ---

    /// The current owner's file: the account's, else the local one.
    pub(crate) fn owner_conn(&self) -> &Connection {
        match &self.account {
            Some(account) => &account.conn,
            None => &self.local,
        }
    }

    /// The device-wide file (home layout, cards, signed-out rows).
    pub(crate) fn local_conn(&self) -> &Connection {
        &self.local
    }

    /// The owner's file for a write; refuses a newer-schema file.
    pub(crate) fn writable(&self) -> Result<&Connection, StoreError> {
        if self.future_schema() {
            return Err(StoreError::FutureSchema);
        }
        Ok(self.owner_conn())
    }

    pub(crate) fn local_writable(&self) -> Result<&Connection, StoreError> {
        if self.local_future {
            return Err(StoreError::FutureSchema);
        }
        Ok(&self.local)
    }

    /// Records whether a write landed.
    pub(crate) fn track<T>(&mut self, result: Result<T, StoreError>) -> Result<T, StoreError> {
        match &result {
            Err(StoreError::Sqlite(_)) => self.degraded = true,
            Ok(_) => self.degraded = false,
            Err(_) => {}
        }
        result
    }

    // --- time, ids, stamps ---

    /// The raw device clock.
    pub fn local_now(&self) -> i64 {
        self.clock.unix_ms()
    }

    /// Device time corrected by the server offset (contracts/sync.md):
    /// staleness, labels and local days use this.
    pub fn now(&self) -> i64 {
        let local = self.local_now();
        self.account.as_ref().map_or(local, |a| a.clock.now(local))
    }

    /// A new UUID v7 entity id.
    pub fn new_id(&mut self) -> String {
        let now = self.local_now();
        self.ids.next(now, self.random.as_mut())
    }

    pub(crate) fn new_v4(&mut self) -> String {
        uuid_v4(self.random.as_mut())
    }

    /// The next stamp, persisted before use. `None` signed out.
    pub(crate) fn stamp(&mut self) -> Result<Option<String>, StoreError> {
        let local = self.local_now();
        let Some(account) = self.account.as_mut() else {
            return Ok(None);
        };
        let stamp = account.clock.stamp(local);
        self.persist_clock()?;
        Ok(Some(stamp))
    }

    /// A stamp at `ms` for a signed-out edit folding in.
    pub(crate) fn stamp_at(&mut self, ms: i64) -> Result<Option<String>, StoreError> {
        let Some(account) = self.account.as_mut() else {
            return Ok(None);
        };
        let stamp = account.clock.stamp_at(ms);
        self.persist_clock()?;
        Ok(Some(stamp))
    }

    /// Advance past received stamps (persisted by the caller's round write).
    pub(crate) fn observe(&mut self, stamp: Option<&str>) {
        if let Some(account) = self.account.as_mut() {
            account.clock.observe(stamp);
        }
    }

    pub(crate) fn on_server_time(&mut self, server_ms: i64) {
        let local = self.local_now();
        if let Some(account) = self.account.as_mut() {
            account.clock.on_server_time(server_ms, local);
        }
    }

    pub(crate) fn persist_clock(&mut self) -> Result<(), StoreError> {
        let Some(account) = &self.account else {
            return Ok(());
        };
        let state = serde_json::to_value(account.clock.state()).unwrap_or(Value::Null);
        let result = kv_set_json(&account.conn, CLOCK_KEY, &state);
        self.track(result)
    }

    /// Seconds-free ISO now, for display `updatedAt` fields.
    pub(crate) fn now_iso(&self) -> String {
        crate::sync::clock::iso_from_ms(self.local_now())
    }

    pub(crate) fn mark_seen(&mut self) -> Result<(), StoreError> {
        let now = self.local_now();
        let result = kv_set(self.writable()?, LAST_SEEN_KEY, &now.to_string());
        self.track(result)
    }

    /// The first full preference pull landed since the last call: the app
    /// should run [`preferences::capture`] with its current settings.
    pub fn take_settings_capture_due(&mut self) -> bool {
        std::mem::take(&mut self.sync_memory.settings_capture_due)
    }

    /// The server answered 426 for this account this session.
    pub fn update_required(&self) -> bool {
        self.sync_memory.update_required
    }

    pub(crate) fn set_update_required(&mut self) {
        self.sync_memory.update_required = true;
    }
}

fn account_ids_in(dir: &Path) -> Vec<i64> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_prefix("account-")?
                .strip_suffix(".sqlite")?
                .parse()
                .ok()
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    use super::AppStore;
    use crate::{clock::Clock, sync::ids::SeededRandom};

    /// A clock tests share with the store they hand it to.
    #[derive(Debug, Clone)]
    pub struct TestClock(pub Arc<AtomicI64>);

    impl TestClock {
        pub fn new(ms: i64) -> Self {
            Self(Arc::new(AtomicI64::new(ms)))
        }
        pub fn advance(&self, ms: i64) {
            self.0.fetch_add(ms, Ordering::SeqCst);
        }
        pub fn set(&self, ms: i64) {
            self.0.store(ms, Ordering::SeqCst);
        }
    }

    impl Clock for TestClock {
        fn unix_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    pub const T0: i64 = 1_800_000_000_000;

    pub fn memory_store(seed: u64) -> (AppStore, TestClock) {
        let clock = TestClock::new(T0);
        let store =
            AppStore::open_in_memory(Box::new(clock.clone()), Box::new(SeededRandom::new(seed)))
                .unwrap();
        (store, clock)
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support::*, *};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eq-core-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn files_per_account_survive_reopen_and_purge_removes_one() {
        let dir = temp_dir("files");
        let clock = TestClock::new(T0);
        {
            let mut store = AppStore::open(
                &dir,
                Box::new(clock.clone()),
                Box::new(crate::sync::ids::SeededRandom::new(1)),
            )
            .unwrap();
            store.sign_in(7, "dev-a").unwrap();
            bookmarks::toggle(&mut store, 2, 255, None).unwrap();
            store.sign_out();
            store.sign_in(8, "dev-b").unwrap();
            assert!(bookmarks::bookmarks(&store).unwrap().is_empty());
        }
        assert!(dir.join("local.sqlite").exists());
        assert!(account_file(&dir, 7).exists());
        let mut store = AppStore::open(
            &dir,
            Box::new(clock),
            Box::new(crate::sync::ids::SeededRandom::new(2)),
        )
        .unwrap();
        store.sign_in(7, "dev-a").unwrap();
        assert_eq!(bookmarks::bookmarks(&store).unwrap().len(), 1);
        store.purge_account(7).unwrap();
        assert!(!account_file(&dir, 7).exists());
        assert_eq!(store.account_id(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_account_file_is_left_untouched_and_refuses_writes() {
        let dir = temp_dir("future");
        std::fs::create_dir_all(&dir).unwrap();
        {
            let conn = Connection::open(account_file(&dir, 9)).unwrap();
            conn.execute_batch("PRAGMA user_version = 42;").unwrap();
        }
        let mut store = AppStore::open(
            &dir,
            Box::new(TestClock::new(T0)),
            Box::new(crate::sync::ids::SeededRandom::new(3)),
        )
        .unwrap();
        let report = store.sign_in(9, "dev").unwrap();
        assert!(report.future_schema);
        assert_eq!(
            bookmarks::toggle(&mut store, 1, 1, None),
            Err(StoreError::FutureSchema)
        );
        let conn = Connection::open(account_file(&dir, 9)).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 42);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn idle_accounts_lose_clean_rows_but_keep_dirty_ones() {
        let (mut store, clock) = memory_store(4);
        store.sign_in(1, "d1").unwrap();
        bookmarks::toggle(&mut store, 1, 1, None).unwrap(); // dirty
        store.sign_out();
        clock.advance(IDLE_ACCOUNT_MS + 1);
        store.sign_in(2, "d2").unwrap();
        store.sign_out();
        store.sign_in(1, "d1").unwrap();
        assert_eq!(bookmarks::bookmarks(&store).unwrap().len(), 1);
    }
}
