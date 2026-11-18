//! Sync rounds (PARITY L2; web `lib/sync/engine.svelte.ts`). The store's
//! dirty rows are the queue: a round collects them, POSTs them with the pull
//! position, merges the page, and clears `dirty` only for rows whose stamps
//! the server acknowledged. Domains never block each other; a failed domain
//! backs off (2 s doubling to 60 s, plus jitter); a 429 holds that domain's
//! pushes for `Retry-After` while its pulls continue; signed out every flush
//! is a no-op. Native polls while focused and on resume: [`should_run`] is
//! the pure cadence the app drives with its timer and lifecycle.
//!
//! Store work goes through a [`StoreRunner`], so the app runs it on a
//! blocking thread while the HTTP call stays on the engine thread.

use std::{cell::RefCell, collections::HashMap, future::Future, pin::Pin};

use serde_json::{Value, json};

use super::{
    RoundApplied, SyncDomain,
    round_status::{ClientInfo, RoundFailure, classify},
    transport::SyncPost,
};
use crate::store::{AppStore, StoreError, bookmarks, position::Position, preferences, sessions};

const RETRY_BASE_MS: i64 = 2_000;
const RETRY_CAP_MS: i64 = 60_000;
const RETRY_JITTER_FRACTION: f64 = 0.25;
/// A focused app pulls this often with nothing queued.
pub const PERIODIC_FLUSH_MS: i64 = 30_000;
/// A runaway domain cannot spin: this many rounds, then the next flush continues.
const MAX_ROUNDS_PER_FLUSH: usize = 50;

/// Exponential retry delay: 2 s, 4 s, 8 s, 16 s, 32 s, capped at 60 s.
pub fn retry_delay_ms(failures: u32) -> i64 {
    let bounded = failures.clamp(1, 6);
    (RETRY_BASE_MS << (bounded - 1)).min(RETRY_CAP_MS)
}

/// A boxed, non-`Send` future of a store job.
pub type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// A store job, `Send` so the app can run it on a blocking thread.
pub type StoreJob<T> = Box<dyn FnOnce(&mut AppStore) -> T + Send + 'static>;

/// Runs store jobs where the caller wants them.
pub trait StoreRunner {
    fn run<T: Send + 'static>(&self, job: StoreJob<T>) -> StoreFuture<'_, Result<T, StoreError>>;
}

/// Runs jobs inline on the calling thread (tests; a caller already off the UI thread).
impl StoreRunner for RefCell<AppStore> {
    fn run<T: Send + 'static>(&self, job: StoreJob<T>) -> StoreFuture<'_, Result<T, StoreError>> {
        let out = job(&mut self.borrow_mut());
        Box::pin(async move { Ok(out) })
    }
}

/// One round's request, built from the store.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRound {
    pub user_id: i64,
    pub body: Value,
    pub sent: Vec<Value>,
}

/// Builds the round body: dirty rows (none when `pull_only`) and the pull
/// position. `None` signed out or on a newer-schema file.
pub fn prepare_round(
    store: &AppStore,
    domain: SyncDomain,
    pull_only: bool,
    client: &ClientInfo,
) -> Result<Option<PreparedRound>, StoreError> {
    let Some(user_id) = store.account_id() else {
        return Ok(None);
    };
    if store.future_schema() {
        return Ok(None);
    }
    let sent = match (pull_only, domain) {
        (true, _) => Vec::new(),
        (false, SyncDomain::Bookmarks) => bookmarks::dirty_rows(store)?,
        (false, SyncDomain::Sessions) => sessions::dirty_rows(store)?,
        (false, SyncDomain::Preferences) => preferences::dirty_rows(store)?,
    };
    let position = Position::load(store.owner_conn(), domain)?;
    let mut body = json!({
        "expectedUserId": user_id,
        "client": client.to_json(),
        "since": position.since,
        "mutations": sent,
    });
    if domain != SyncDomain::Preferences {
        body["after"] = json!(position.after);
    }
    Ok(Some(PreparedRound {
        user_id,
        body,
        sent,
    }))
}

/// Applies a page durably, unless the account changed while it flew.
pub fn apply_round(
    store: &mut AppStore,
    domain: SyncDomain,
    user_id: i64,
    response: &Value,
    sent: &[Value],
) -> Result<RoundApplied, StoreError> {
    if store.account_id() != Some(user_id) {
        return Err(StoreError::AccountChanged);
    }
    match domain {
        SyncDomain::Bookmarks => bookmarks::apply_round(store, response, sent),
        SyncDomain::Sessions => sessions::apply_round(store, response, sent),
        SyncDomain::Preferences => preferences::apply_round(store, response, sent),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Syncing,
    Error,
}

/// What the status line shows (web `SyncStatus` plus the store's counts).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoundStatus {
    pub phase: Phase,
    /// Rows waiting to reach the server.
    pub pending: usize,
    /// Rows the server refused (kept, not resent until edited).
    pub rejected: usize,
    /// Rows waiting for room under the daily ceiling.
    pub capped: usize,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    /// A durable write failed: never report rows as synced.
    pub degraded: bool,
    /// 426: show the update prompt; reading keeps working.
    pub update_required: bool,
    /// The server says the session is not this account: re-probe auth.
    pub session_lost: bool,
    /// Pushes held by a 429 until this time (pulls continue).
    pub held_until: Option<i64>,
    pub signed_in: bool,
}

/// The store's side of the status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StoreCounts {
    pub pending: usize,
    pub rejected: usize,
    pub capped: usize,
    pub degraded: bool,
    pub signed_in: bool,
}

/// Counts across every domain.
pub fn store_counts(store: &AppStore) -> Result<StoreCounts, StoreError> {
    Ok(StoreCounts {
        pending: bookmarks::pending(store)?
            + sessions::pending(store)?
            + preferences::pending(store)?,
        rejected: bookmarks::rejected_count(store)?
            + sessions::rejected_count(store)?
            + preferences::rejected_count(store)?,
        capped: bookmarks::capped_count(store) + sessions::capped_count(store),
        degraded: store.degraded(),
        signed_in: store.account_id().is_some(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Drained,
    Failed,
    Paused,
}

/// In-memory engine state: backoff, holds, phase, last error.
#[derive(Debug, Clone, Default)]
pub struct SyncEngine {
    client: ClientInfo,
    failures: HashMap<SyncDomain, u32>,
    hold_until: HashMap<SyncDomain, i64>,
    phase: Phase,
    last_sync_at: Option<i64>,
    last_error: Option<String>,
    retry_at: Option<i64>,
    last_flush_at: Option<i64>,
    running: bool,
    again: bool,
    update_required: bool,
    session_lost: bool,
}

impl SyncEngine {
    pub fn new(client: ClientInfo) -> Self {
        Self {
            client,
            ..Self::default()
        }
    }

    /// The status, joined with the store's counts.
    pub fn status(&self, counts: StoreCounts, now: i64) -> RoundStatus {
        RoundStatus {
            phase: self.phase,
            pending: counts.pending,
            rejected: counts.rejected,
            capped: counts.capped,
            last_sync_at: self.last_sync_at,
            last_error: self.last_error.clone(),
            degraded: counts.degraded,
            update_required: self.update_required,
            session_lost: self.session_lost,
            held_until: self.hold_until.values().copied().filter(|t| *t > now).max(),
            signed_in: counts.signed_in,
        }
    }

    /// When the next backoff retry is due.
    pub fn retry_at(&self) -> Option<i64> {
        self.retry_at
    }

    pub fn last_flush_at(&self) -> Option<i64> {
        self.last_flush_at
    }

    /// The auth seam handled a lost session (signed in again): clear the flag.
    pub fn clear_session_lost(&mut self) {
        self.session_lost = false;
    }

    /// Account switch: backoff and holds belong to the old account.
    pub fn reset_for_account(&mut self) {
        let client = self.client.clone();
        *self = Self::new(client);
    }

    /// Back online: failures reset so the first round runs at once.
    pub fn note_online(&mut self) {
        self.failures.clear();
        self.retry_at = None;
    }

    fn schedule_retry(&mut self, now: i64, jitter: f64) {
        let worst = self.failures.values().copied().max().unwrap_or(1).max(1);
        let base = retry_delay_ms(worst);
        let extra = (base as f64 * RETRY_JITTER_FRACTION * jitter.clamp(0.0, 1.0)).round() as i64;
        self.retry_at = Some(now + base + extra);
    }
}

/// What the app knows when its timer or lifecycle fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollInput {
    pub now: i64,
    /// The app is in the foreground and active.
    pub focused: bool,
    pub online: bool,
    pub signed_in: bool,
}

/// Why a flush should run now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A backoff retry is due (also in the background: it was asked for).
    Retry,
    /// The periodic pull of a focused app.
    Periodic,
}

/// The poll cadence, pure: retries when due; every 30 s while focused.
/// Resume, wake, coming online and local edits flush directly (`kick`).
pub fn should_run(engine: &SyncEngine, input: PollInput) -> Option<Trigger> {
    if !input.signed_in || !input.online || engine.running {
        return None;
    }
    if engine.retry_at.is_some_and(|at| at <= input.now) {
        return Some(Trigger::Retry);
    }
    let stale = engine
        .last_flush_at
        .is_none_or(|at| input.now - at >= PERIODIC_FLUSH_MS);
    (input.focused && stale).then_some(Trigger::Periodic)
}

/// What a flush call did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlushReport {
    /// Another flush was running: this one joined it as a later pass.
    pub coalesced: bool,
    /// `settings.*` values that arrived: the app's settings store shows them.
    pub settings: Vec<(String, Value)>,
    /// The first full preference pull landed: capture settings now.
    pub capture_settings: bool,
}

/// The flush's surroundings.
pub struct FlushOptions<'a> {
    /// Device time in epoch ms.
    pub now: &'a dyn Fn() -> i64,
    pub online: bool,
    /// 0..1, for retry jitter.
    pub jitter: f64,
    /// Flush one domain only (a local edit's domain).
    pub only: Option<SyncDomain>,
}

/// Runs rounds for every target domain until each drains, pauses or fails.
/// Concurrent calls coalesce into one more pass of the running flush.
pub async fn flush<R, P>(
    engine: &RefCell<SyncEngine>,
    runner: &R,
    post: &P,
    options: FlushOptions<'_>,
) -> FlushReport
where
    R: StoreRunner + ?Sized,
    P: SyncPost + ?Sized,
{
    {
        let mut e = engine.borrow_mut();
        if e.running {
            e.again = true;
            return FlushReport {
                coalesced: true,
                ..FlushReport::default()
            };
        }
        e.running = true;
    }
    // A dropped (cancelled) flush must not leave the engine marked running.
    let _running = RunningGuard(engine);
    let mut report = FlushReport::default();
    let mut only = options.only;
    loop {
        engine.borrow_mut().again = false;
        pass(engine, runner, post, &options, only, &mut report).await;
        if !engine.borrow().again {
            break;
        }
        // A coalesced call may have named another domain: widen to all.
        only = None;
    }
    report
}

struct RunningGuard<'a>(&'a RefCell<SyncEngine>);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut e) = self.0.try_borrow_mut() {
            e.running = false;
            e.again = false;
        }
    }
}

async fn pass<R, P>(
    engine: &RefCell<SyncEngine>,
    runner: &R,
    post: &P,
    options: &FlushOptions<'_>,
    only: Option<SyncDomain>,
    report: &mut FlushReport,
) where
    R: StoreRunner + ?Sized,
    P: SyncPost + ?Sized,
{
    let now = (options.now)();
    let signed_in = runner
        .run(Box::new(|s: &mut AppStore| {
            s.account_id().is_some() && !s.future_schema()
        }))
        .await
        .unwrap_or(false);
    // Signed out (or a newer local file): nothing to push or pull.
    if !signed_in {
        return;
    }
    let targets: Vec<SyncDomain> = only.map_or_else(|| SyncDomain::ALL.to_vec(), |d| vec![d]);
    {
        let mut e = engine.borrow_mut();
        e.retry_at = None;
        e.last_flush_at = Some(now);
        if !options.online {
            e.phase = Phase::Error;
            e.last_error = Some("offline".into());
            for domain in &targets {
                *e.failures.entry(*domain).or_insert(0) += 1;
            }
            e.schedule_retry(now, options.jitter);
            return;
        }
        e.phase = Phase::Syncing;
    }
    let mut failed = false;
    let mut drained_any = false;
    for domain in targets {
        let outcome = drain_domain(engine, runner, post, options, domain, report).await;
        let mut e = engine.borrow_mut();
        match outcome {
            Outcome::Failed => {
                *e.failures.entry(domain).or_insert(0) += 1;
                failed = true;
            }
            Outcome::Drained => {
                e.failures.remove(&domain);
                drained_any = true;
            }
            Outcome::Paused => {}
        }
    }
    let mut e = engine.borrow_mut();
    if failed {
        // Keep the failing domain's error; its rows stay dirty.
        e.phase = Phase::Error;
        e.schedule_retry((options.now)(), options.jitter);
    } else if drained_any {
        e.phase = Phase::Idle;
        e.last_error = None;
    } else {
        // Every target paused: a paused round must not clear a real error.
        e.phase = match e.last_error {
            Some(_) => Phase::Error,
            None => Phase::Idle,
        };
    }
}

async fn drain_domain<R, P>(
    engine: &RefCell<SyncEngine>,
    runner: &R,
    post: &P,
    options: &FlushOptions<'_>,
    domain: SyncDomain,
    report: &mut FlushReport,
) -> Outcome
where
    R: StoreRunner + ?Sized,
    P: SyncPost + ?Sized,
{
    let now = (options.now)();
    let client = engine.borrow().client.clone();
    let mut pull_only = {
        let mut e = engine.borrow_mut();
        let held = e.hold_until.get(&domain).is_some_and(|until| now < *until);
        if !held {
            e.hold_until.remove(&domain);
        }
        held
    };
    for _ in 0..MAX_ROUNDS_PER_FLUSH {
        let job_client = client.clone();
        let prepared = runner
            .run(Box::new(move |s: &mut AppStore| {
                prepare_round(s, domain, pull_only, &job_client)
            }))
            .await
            .and_then(|r| r);
        let prepared = match prepared {
            Ok(Some(prepared)) => prepared,
            Ok(None) => return Outcome::Paused,
            Err(err) => {
                engine.borrow_mut().last_error = Some(err.to_string());
                return Outcome::Failed;
            }
        };
        let PreparedRound {
            user_id,
            body,
            sent,
        } = prepared;
        let response = match post.post_json(domain.path(), body).await {
            Ok(response) => response,
            Err(err) => {
                let failure = classify(domain.label(), domain.mismatch_message(), &err);
                match on_failure(engine, runner, domain, failure, pull_only, (options.now)()).await
                {
                    Some(outcome) => return outcome,
                    None => {
                        pull_only = true;
                        continue;
                    }
                }
            }
        };
        let applied = runner
            .run(Box::new(move |s: &mut AppStore| {
                let applied = apply_round(s, domain, user_id, &response, &sent);
                let capture = applied.is_ok() && s.take_settings_capture_due();
                applied.map(|a| (a, capture))
            }))
            .await
            .and_then(|r| r);
        let (applied, capture) = match applied {
            Ok(applied) => applied,
            Err(StoreError::AccountChanged | StoreError::SignedOut | StoreError::FutureSchema) => {
                return Outcome::Paused;
            }
            Err(err) => {
                engine.borrow_mut().last_error = Some(err.to_string());
                return Outcome::Failed;
            }
        };
        report.settings.extend(applied.settings);
        report.capture_settings |= capture;
        {
            let mut e = engine.borrow_mut();
            e.last_sync_at = Some((options.now)());
            e.last_error = None;
        }
        // A pull-only round never drains the queue: the held pushes still wait.
        if pull_only {
            if !applied.more {
                return Outcome::Paused;
            }
            continue;
        }
        if !applied.more {
            return Outcome::Drained;
        }
    }
    if pull_only {
        Outcome::Paused
    } else {
        Outcome::Drained
    }
}

/// The status table's verdict. `None`: the push was held by a 429, pull now.
async fn on_failure<R: StoreRunner + ?Sized>(
    engine: &RefCell<SyncEngine>,
    runner: &R,
    domain: SyncDomain,
    failure: RoundFailure,
    pull_only: bool,
    now: i64,
) -> Option<Outcome> {
    match failure {
        RoundFailure::Paused {
            account_mismatch, ..
        } => {
            if account_mismatch {
                engine.borrow_mut().session_lost = true;
            }
            Some(Outcome::Paused)
        }
        RoundFailure::UpgradeRequired => {
            engine.borrow_mut().update_required = true;
            let _ = runner
                .run(Box::new(|s: &mut AppStore| s.set_update_required()))
                .await;
            Some(Outcome::Paused)
        }
        RoundFailure::RetryAfter { seconds } => {
            let until = now
                + i64::try_from(seconds)
                    .unwrap_or(i64::MAX / 2_000)
                    .saturating_mul(1000);
            engine.borrow_mut().hold_until.insert(domain, until);
            if pull_only {
                return Some(Outcome::Paused);
            }
            None
        }
        RoundFailure::SnapshotTooLarge { message } | RoundFailure::Failed { message, .. } => {
            engine.borrow_mut().last_error = Some(message);
            Some(Outcome::Failed)
        }
    }
}

#[cfg(test)]
mod tests;
