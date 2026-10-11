use std::sync::{Arc, LazyLock};

use rux_auth::{AuthError, AuthErrorCode};
use tokio::sync::Semaphore;

use crate::error::{ErrorCode, ErrorResponse};

static PASSWORD_WORK: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    let slots = std::env::var("PASSWORD_WORK_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|slots| (1..=32).contains(slots))
        .unwrap_or(2);
    Arc::new(Semaphore::new(slots))
});

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

pub async fn run<T, F>(work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_with(Arc::clone(&PASSWORD_WORK), work).await
}

async fn run_with<T, F>(budget: Arc<Semaphore>, work: F) -> Result<T, PasswordWorkError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = budget
        .try_acquire_owned()
        .map_err(|_| PasswordWorkError::Busy)?;
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

    #[tokio::test]
    async fn saturation_rejects_before_executing_work() {
        let budget = Arc::new(Semaphore::new(1));
        let _permit = budget.try_acquire().unwrap();
        let result = run_with(Arc::clone(&budget), || 17).await;
        assert!(matches!(result, Err(PasswordWorkError::Busy)));
    }

    #[tokio::test]
    async fn cancellation_keeps_capacity_until_blocking_work_finishes() {
        let budget = Arc::new(Semaphore::new(1));
        let worker_budget = Arc::clone(&budget);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            run_with(worker_budget, move || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
            .await
        });
        entered_rx.await.unwrap();
        task.abort();
        let _ = task.await;
        let competing = run_with(Arc::clone(&budget), || ()).await;
        release_tx.send(()).unwrap();
        assert!(matches!(competing, Err(PasswordWorkError::Busy)));
    }

    #[tokio::test]
    async fn panic_releases_capacity_and_returns_sanitized_error() {
        let budget = Arc::new(Semaphore::new(1));
        let failed = run_with(Arc::clone(&budget), || panic!("test panic")).await;
        assert!(matches!(failed, Err(PasswordWorkError::Failed)));
        assert_eq!(run_with(budget, || 19).await.unwrap(), 19);
    }

    #[test]
    fn saturation_maps_to_rate_limit_for_both_handler_error_types() {
        let auth: AuthError = PasswordWorkError::Busy.into();
        let api: ErrorResponse = PasswordWorkError::Busy.into();
        assert_eq!(auth.status.as_u16(), 429);
        assert_eq!(api.code.status_code(), 429);
    }
}
