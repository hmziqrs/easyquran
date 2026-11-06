use std::time::Duration;

use rux_request_gate::{GateError, RateLimitStore};
use serde_json::json;
use tracing::warn;

use crate::error::{ErrorCode, ErrorResponse};
use crate::utils::telemetry;

pub use rux_request_gate::{
    AbuseLimiterConfig, BlockScope, FailureMode, GateDecision, LimiterDecision,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct TelemetryHooks;

impl rux_request_gate::LimiterHooks for TelemetryHooks {
    fn on_check(&self) {
        telemetry::limiter_metrics().checks.add(1, &[]);
    }
    fn on_allowed(&self, _short_count: u64, _long_count: u64) {
        telemetry::limiter_metrics().allowed.add(1, &[]);
    }
    fn on_blocked(
        &self,
        scope: BlockScope,
        _retry_after_secs: u64,
        _short_count: u64,
        _long_count: u64,
    ) {
        let m = telemetry::limiter_metrics();
        m.blocked.add(1, &[]);
        match scope {
            BlockScope::Temp => m.temp_blocks.add(1, &[]),
            BlockScope::Long => m.long_blocks.add(1, &[]),
        }
    }
}

pub async fn limiter(
    store: &rux_request_gate::InMemoryStore,
    key_prefix: &str,
    config: AbuseLimiterConfig,
) -> Result<(), ErrorResponse> {
    // ── DEV: auth throttling off ────────────────────────────────────────────
    // Non-production RUNTIME skips the per-account/per-IP throttle entirely so
    // auth flows can be exercised repeatedly while testing — a tripped block
    // window otherwise waits out up to a full hour ("Too many attempts. Try
    // again in N seconds."). cfg!(test) keeps the real limiter under `cargo
    // test` (bucket-separation tests drive this fn directly); production keeps
    // the fail-closed limiter untouched. Remove this gate when dev needs
    // realistic limits again.
    if !cfg!(test) && matches!(crate::config::settings::is_production(), Ok(false)) {
        return Ok(());
    }
    let res = rux_request_gate::check(store, &TelemetryHooks, key_prefix, config).await;
    map_limiter_result(res)
}

pub(crate) fn map_limiter_result(
    res: Result<rux_request_gate::LimiterDecision, rux_request_gate::GateError>,
) -> Result<(), ErrorResponse> {
    use rux_request_gate::LimiterDecision;
    match res {
        Ok(LimiterDecision::Allowed { .. }) => Ok(()),
        Ok(LimiterDecision::Blocked {
            retry_after_secs, ..
        }) => Err(too_many_attempts(retry_after_secs)),
        Err(err) => Err(map_gate_error(err)),
    }
}

fn too_many_attempts(retry_after_secs: u64) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::TooManyAttempts)
        .with_message(format!(
            "Too many attempts. Try again in {} seconds.",
            retry_after_secs
        ))
        .with_retry_after(retry_after_secs)
        .with_context(json!({ "retryAfter": retry_after_secs }))
}

fn map_gate_error(err: GateError) -> ErrorResponse {
    match err {
        GateError::StoreUnavailable(detail) => ErrorResponse::new(ErrorCode::ServiceUnavailable)
            .with_message("Limiter unavailable (store error)")
            .with_details(detail),
        GateError::UnexpectedResult => ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Limiter returned unexpected result"),
    }
}

/// Failure-only IP budget for the passkey and OAuth finish paths (A13). A rejected finish
/// counts one failure; a success never clears another client's block (Ip mode).
pub const FINISH_FAILURE_CONFIG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 10,
    temp_block_range: 600,
    temp_block_duration: 600,
    block_retry_limit: 5,
    block_range: 900,
    block_duration: 900,
};

/// Outcome of one credential or IP check, as [`settle`] sees it (A8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    /// The check passed. A credential bucket is cleared.
    Succeeded,
    /// A wrong password or code. Counts as one failure.
    Rejected,
    /// Anything else (ban, session error, store error). Counts neither way.
    Neutral,
}

/// Maps a verification result to an [`Attempt`]. Only credential rejections
/// count; a ban or an internal error never moves the limiter.
pub fn attempt_of<T>(outcome: &Result<T, ErrorResponse>) -> Attempt {
    match outcome {
        Ok(_) => Attempt::Succeeded,
        Err(err)
            if matches!(
                err.code,
                ErrorCode::InvalidCredentials | ErrorCode::InvalidToken
            ) =>
        {
            Attempt::Rejected
        }
        Err(_) => Attempt::Neutral,
    }
}

/// Like [`attempt_of`], for verify-code lookups. A wrong or expired code comes
/// back from the DB layer as InvalidInput or RecordNotFound, not as a token error.
pub fn attempt_of_code<T>(outcome: &Result<T, ErrorResponse>) -> Attempt {
    match outcome {
        Ok(_) => Attempt::Succeeded,
        Err(err)
            if matches!(
                err.code,
                ErrorCode::InvalidInput | ErrorCode::RecordNotFound | ErrorCode::InvalidToken
            ) =>
        {
            Attempt::Rejected
        }
        Err(_) => Attempt::Neutral,
    }
}

// Non-production runs skip the auth throttle (see `limiter`). Same gate here so
// the failure-only path behaves identically in dev.
fn dev_bypass() -> bool {
    !cfg!(test) && matches!(crate::config::settings::is_production(), Ok(false))
}

/// Pre-verification step for failure-only limiters (A8). Reads the bucket and
/// never appends. Credential mode sleeps the progressive delay and admits the
/// attempt; it never returns 429. IP mode returns 429 while blocked.
pub async fn gate(
    store: &rux_request_gate::InMemoryStore,
    key: &str,
    config: AbuseLimiterConfig,
    mode: FailureMode,
) -> Result<(), ErrorResponse> {
    if dev_bypass() {
        return Ok(());
    }
    match store.abuse_peek(key, config, mode).await {
        Ok(GateDecision::Pass) => Ok(()),
        Ok(GateDecision::Delay { secs }) => {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            Ok(())
        }
        Ok(GateDecision::Blocked { retry_after_secs }) => Err(too_many_attempts(retry_after_secs)),
        Err(err) => Err(map_gate_error(err)),
    }
}

/// Post-verification step for failure-only limiters (A8). A rejection appends
/// one failure. A success clears a credential bucket. IP buckets are never
/// cleared by success, so one valid login cannot lift another client's block.
pub async fn settle(
    store: &rux_request_gate::InMemoryStore,
    key: &str,
    config: AbuseLimiterConfig,
    mode: FailureMode,
    attempt: Attempt,
) {
    if dev_bypass() {
        return;
    }
    match attempt {
        Attempt::Succeeded if mode == FailureMode::Credential => {
            if let Err(err) = store.clear_limit(key).await {
                warn!(error = %err, "limiter success clear failed");
            }
        }
        Attempt::Rejected => {
            if let Err(err) = store.abuse_record(key, config, mode).await {
                warn!(error = %err, "limiter failure record failed");
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use rux_request_gate::{BlockScope, GateError, LimiterDecision};

    #[test]
    fn allowed_maps_ok() {
        assert!(map_limiter_result(Ok(LimiterDecision::Allowed {
            short_count: 1,
            long_count: 0,
        }))
        .is_ok());
    }

    #[test]
    fn blocked_maps_to_429() {
        let resp = map_limiter_result(Ok(LimiterDecision::Blocked {
            scope: BlockScope::Temp,
            retry_after_secs: 42,
            short_count: 5,
            long_count: 9,
        }))
        .unwrap_err()
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn store_unavailable_maps_to_503() {
        let resp = map_limiter_result(Err(GateError::StoreUnavailable("boom".into())))
            .unwrap_err()
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn unexpected_result_maps_to_500() {
        let resp = map_limiter_result(Err(GateError::UnexpectedResult))
            .unwrap_err()
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    }
}
