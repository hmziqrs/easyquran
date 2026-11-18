//! Argon2 work lanes (A11).
//!
//! Three lanes, each with its own slots and its own bounded queue:
//!
//! - `Public`: anonymous password work (login, registration, password reset,
//!   the login-time second factor). Sized by `PASSWORD_WORK_CONCURRENCY`
//!   (default 2, range 1..=32).
//! - `Dummy`: the timing-equalizing verify that runs for unknown or OAuth-only
//!   emails. Its own single slot, so it never takes a real login's slot.
//! - `Authenticated`: work for a session that already proved its password or
//!   second factor (reauth, fresh-auth, 2FA setup and current-factor checks).
//!   One reserved slot, so a login flood cannot starve it.
//!
//! A caller that finds its lane full waits up to [`QUEUE_WAIT`] for a slot. At
//! most [`MAX_WAITERS`] callers wait per lane; past that the call is refused at
//! once. Both refusals are a 429 and bump the saturation counter. Waiting is
//! bounded in time and in count, so the queue cannot grow without limit.
//!
//! The public lane's size is the Argon2 CPU budget for anonymous traffic. The
//! dummy and authenticated lanes add one slot each on top of it.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use opentelemetry::KeyValue;
use rux_auth::{AuthError, AuthErrorCode};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::{ErrorCode, ErrorResponse};
use crate::utils::telemetry;

/// How long a caller waits for a lane slot before it is refused.
pub const QUEUE_WAIT: Duration = Duration::from_millis(1_500);

/// Callers allowed to wait on one lane at a time. Sized so that a burst of
/// twenty concurrent logins on the default two-slot public lane queues in full.
pub const MAX_WAITERS: usize = 32;

/// Which lane a unit of password work runs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Public,
    Dummy,
    Authenticated,
}

impl Lane {
    fn label(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Dummy => "dummy",
            Self::Authenticated => "authenticated",
        }
    }
}

/// One lane: its slots and the number of callers queued for them.
pub struct Budget {
    slots: Arc<Semaphore>,
    waiting: AtomicUsize,
}

impl Budget {
    pub fn new(slots: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(slots)),
            waiting: AtomicUsize::new(0),
        }
    }
}

fn configured_public_slots() -> usize {
    std::env::var("PASSWORD_WORK_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|slots| (1..=32).contains(slots))
        .unwrap_or(2)
}

static PUBLIC: LazyLock<Arc<Budget>> =
    LazyLock::new(|| Arc::new(Budget::new(configured_public_slots())));
static DUMMY: LazyLock<Arc<Budget>> = LazyLock::new(|| Arc::new(Budget::new(1)));
static AUTHENTICATED: LazyLock<Arc<Budget>> = LazyLock::new(|| Arc::new(Budget::new(1)));

/// Refusals since process start, across all lanes. Read by tests and by the
/// telemetry counter `auth.password_work.saturated`.
static SATURATED: AtomicU64 = AtomicU64::new(0);

pub fn saturation_count() -> u64 {
    SATURATED.load(Ordering::Relaxed)
}

fn lane_budget(lane: Lane) -> Arc<Budget> {
    match lane {
        Lane::Public => Arc::clone(&PUBLIC),
        Lane::Dummy => Arc::clone(&DUMMY),
        Lane::Authenticated => Arc::clone(&AUTHENTICATED),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PasswordWorkError {
    #[error("Password service busy; retry later")]
    Busy,
    #[error("Password processing failed")]
    Failed,
}

impl From<PasswordWorkError> for ErrorResponse {
    fn from(error: PasswordWorkError) -> Self {
        match error {
            PasswordWorkError::Busy => Self::new(ErrorCode::RateLimited),
            PasswordWorkError::Failed => Self::new(ErrorCode::InternalServerError),
        }
    }
}

impl From<PasswordWorkError> for AuthError {
    fn from(error: PasswordWorkError) -> Self {
        match error {
            PasswordWorkError::Busy => Self::new(AuthErrorCode::RateLimited),
            PasswordWorkError::Failed => Self::new(AuthErrorCode::InternalError),
        }
    }
}

/// Runs anonymous password work on the public lane.
pub async fn run<T, F>(work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_in(Lane::Public, work).await
}

/// Runs the timing-equalizing dummy verify on its own lane.
pub async fn run_dummy<T, F>(work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_in(Lane::Dummy, work).await
}

/// Runs password work for an already-authenticated session on its reserved lane.
pub async fn run_authenticated<T, F>(work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_in(Lane::Authenticated, work).await
}

pub async fn run_in<T, F>(lane: Lane, work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_with(lane, lane_budget(lane), work).await
}

/// Counts one waiter on a lane's queue for as long as the guard lives. Dropping
/// it, including when the waiting future is cancelled, releases the count.
struct WaiterGuard<'a>(&'a AtomicUsize);

impl<'a> WaiterGuard<'a> {
    fn enter(waiting: &'a AtomicUsize) -> Option<Self> {
        waiting
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                (count < MAX_WAITERS).then_some(count + 1)
            })
            .ok()
            .map(|_| Self(waiting))
    }
}

impl Drop for WaiterGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn saturated(lane: Lane) -> PasswordWorkError {
    SATURATED.fetch_add(1, Ordering::Relaxed);
    telemetry::auth_metrics()
        .password_work_saturated
        .add(1, &[KeyValue::new("lane", lane.label())]);
    PasswordWorkError::Busy
}

async fn acquire(lane: Lane, budget: &Budget) -> Result<OwnedSemaphorePermit, PasswordWorkError> {
    if let Ok(permit) = Arc::clone(&budget.slots).try_acquire_owned() {
        return Ok(permit);
    }
    let Some(_waiter) = WaiterGuard::enter(&budget.waiting) else {
        return Err(saturated(lane));
    };
    match tokio::time::timeout(QUEUE_WAIT, Arc::clone(&budget.slots).acquire_owned()).await {
        Ok(Ok(permit)) => Ok(permit),
        _ => Err(saturated(lane)),
    }
}

async fn run_with<T, F>(lane: Lane, budget: Arc<Budget>, work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = acquire(lane, &budget).await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    })
    .await
    .map_err(|_| PasswordWorkError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn budget(slots: usize) -> Arc<Budget> {
        Arc::new(Budget::new(slots))
    }

    #[tokio::test]
    async fn saturation_rejects_after_the_queue_wait_without_running_work() {
        let budget = budget(1);
        let _permit = Arc::clone(&budget.slots).try_acquire_owned().unwrap();
        let started = Instant::now();
        let result = run_with(Lane::Public, Arc::clone(&budget), || 17).await;
        assert!(matches!(result, Err(PasswordWorkError::Busy)));
        assert!(started.elapsed() >= QUEUE_WAIT);
    }

    #[tokio::test]
    async fn waiter_cap_refuses_at_once_instead_of_queueing() {
        let budget = budget(1);
        let _permit = Arc::clone(&budget.slots).try_acquire_owned().unwrap();
        let mut waiters = Vec::new();
        for _ in 0..MAX_WAITERS {
            let budget = Arc::clone(&budget);
            waiters.push(tokio::spawn(async move {
                run_with(Lane::Public, budget, || ()).await
            }));
        }
        // Let every waiter reach the queue before the extra call.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = Instant::now();
        let refused = run_with(Lane::Public, Arc::clone(&budget), || ()).await;
        assert!(matches!(refused, Err(PasswordWorkError::Busy)));
        assert!(
            started.elapsed() < QUEUE_WAIT / 2,
            "waiter cap must not wait"
        );
        for waiter in waiters {
            let _ = waiter.await;
        }
    }

    #[tokio::test]
    async fn queued_caller_runs_once_a_slot_frees_within_the_wait() {
        let budget = budget(1);
        let holder = Arc::clone(&budget.slots).try_acquire_owned().unwrap();
        let waiter_budget = Arc::clone(&budget);
        let waiter =
            tokio::spawn(async move { run_with(Lane::Public, waiter_budget, || 23).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(holder);
        assert_eq!(waiter.await.unwrap().unwrap(), 23);
    }

    #[tokio::test]
    async fn cancellation_keeps_capacity_until_blocking_work_finishes() {
        let budget = budget(1);
        let worker_budget = Arc::clone(&budget);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            run_with(Lane::Public, worker_budget, move || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
            .await
        });
        entered_rx.await.unwrap();
        task.abort();
        let _ = task.await;
        let competing = run_with(Lane::Public, Arc::clone(&budget), || ()).await;
        release_tx.send(()).unwrap();
        assert!(matches!(competing, Err(PasswordWorkError::Busy)));
    }

    #[tokio::test]
    async fn panic_releases_capacity_and_returns_sanitized_error() {
        let budget = budget(1);
        let failed = run_with(Lane::Public, Arc::clone(&budget), || panic!("test panic")).await;
        assert!(matches!(failed, Err(PasswordWorkError::Failed)));
        assert_eq!(run_with(Lane::Public, budget, || 19).await.unwrap(), 19);
    }

    #[tokio::test]
    async fn refusals_bump_the_saturation_counter() {
        let budget = budget(1);
        let _permit = Arc::clone(&budget.slots).try_acquire_owned().unwrap();
        let before = saturation_count();
        let refused = run_with(Lane::Dummy, budget, || ()).await;
        assert!(matches!(refused, Err(PasswordWorkError::Busy)));
        assert!(saturation_count() > before);
    }

    #[test]
    fn saturation_maps_to_rate_limit_for_both_handler_error_types() {
        let auth: AuthError = PasswordWorkError::Busy.into();
        let api: ErrorResponse = PasswordWorkError::Busy.into();
        assert_eq!(auth.status.as_u16(), 429);
        assert_eq!(api.code.status_code(), 429);
    }
}
