//! Daily cross-domain write ceiling (A7, contracts/sync.md).
//!
//! One fixed-window counter per user, `writes:user:{id}`, lives in the shared
//! gate store. The window ends at the next midnight UTC, so the count resets
//! once a day, and the L2 flush persists it, so a restart keeps the day's count.
//! Every accepted mutation across reading-session sync, bookmark sync and
//! analytics events charges one unit. Pull-only rounds charge nothing.
//!
//! A charge past the ceiling is refused. Sync rows that partly fit come back
//! `capped` (kept dirty by the client). A round with no room at all answers 429
//! with `Retry-After` to midnight UTC. The first refusal of a user-day writes one
//! `rate_limit_events` row (event `daily_ceiling`), after the round's transaction
//! ends: the DB pool holds one connection, so the row is never written inside a
//! transaction.

use std::time::Duration;

use chrono::{DateTime, Utc};
use rux_request_gate::{GateError, InMemoryStore, RateLimitStore};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, Value};
use tracing::warn;

use crate::error::{ErrorCode, ErrorResponse};
use crate::services::rate_limit_store::{key_class, LEDGER_RETENTION_CLASS, LEDGER_RETENTION_SECS};

/// Accepted mutations per user per UTC day, across the three write route classes.
pub const DAILY_WRITE_CEILING: u64 = 5_000;
/// `rate_limit_events.event` for the day's first refusal.
pub const CEILING_EVENT: &str = "daily_ceiling";
/// `rate_limit_events.scope` for this ledger row (a day, not a block scope).
const CEILING_SCOPE: &str = "day";
const DAY_FALLBACK_SECS: u64 = 86_400;

/// Gate-store key for one user's day counter.
pub fn writes_key(user_id: i32) -> String {
    format!("writes:user:{user_id}")
}

/// Seconds from `now` to the next UTC midnight, never zero.
pub fn secs_until_midnight_utc(now: DateTime<Utc>) -> u64 {
    let Some(next) = now
        .date_naive()
        .succ_opt()
        .and_then(|day| day.and_hms_opt(0, 0, 0))
    else {
        return DAY_FALLBACK_SECS;
    };
    (next.and_utc() - now).num_seconds().max(1) as u64
}

/// Outcome of one charge against the day's counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charge {
    /// The mutation fits under the ceiling and may apply.
    pub accepted: bool,
    /// Seconds until the counter resets (midnight UTC). Always at least 1.
    pub retry_after: u64,
    /// This refusal is the day's first. The caller writes the ledger row once.
    pub transition: bool,
}

/// Charge one accepted mutation. The counter moves on every attempt, so the
/// refusal that crosses the ceiling is the single transition of the day.
pub async fn charge(
    store: &InMemoryStore,
    user_id: i32,
    now: DateTime<Utc>,
) -> Result<Charge, GateError> {
    let window = Duration::from_secs(secs_until_midnight_utc(now));
    let (count, ttl) = store.incr_expire(&writes_key(user_id), window).await?;
    let accepted = count <= DAILY_WRITE_CEILING;
    Ok(Charge {
        accepted,
        retry_after: ttl.max(1),
        transition: !accepted && count == DAILY_WRITE_CEILING + 1,
    })
}

/// Per-request ceiling state. Collects the transition flag across charges, so a
/// request writes at most one ledger row, after its transaction ends.
#[derive(Debug, Default)]
pub struct CeilingLedger {
    transitioned: bool,
}

impl CeilingLedger {
    /// Charge one mutation. A store error fails closed as 503.
    pub async fn charge(
        &mut self,
        store: &InMemoryStore,
        user_id: i32,
        now: DateTime<Utc>,
    ) -> Result<Charge, ErrorResponse> {
        let charge = charge(store, user_id, now)
            .await
            .map_err(|_| ErrorResponse::new(ErrorCode::ServiceUnavailable))?;
        self.transitioned |= charge.transition;
        Ok(charge)
    }

    /// Write the day's ledger row if this request crossed the ceiling. Call only
    /// after the request's transaction has ended. A failed write is logged, not
    /// surfaced: the ledger is measurement, and the refusal already happened.
    pub async fn settle(self, db: &DatabaseConnection, user_id: i32, now: DateTime<Utc>) {
        if !self.transitioned {
            return;
        }
        if let Err(err) = record_transition(db, user_id, now).await {
            warn!(error = %err, "daily write ceiling ledger write failed (non-fatal)");
        }
    }
}

/// The 429 for a round with no room left. `Retry-After` runs to midnight UTC.
pub fn ceiling_error(retry_after: u64) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::RateLimited)
        .with_message("Daily sync limit reached. Changes resume after midnight UTC.")
        .with_retry_after(retry_after)
}

/// Insert the day's `daily_ceiling` ledger row unless one already exists for this
/// user and UTC day (a restart inside the 10 s flush gap could cross twice).
pub async fn record_transition(
    db: &DatabaseConnection,
    user_id: i32,
    now: DateTime<Utc>,
) -> Result<(), DbErr> {
    let key = writes_key(user_id);
    let day_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map_or(0, |midnight| midnight.and_utc().timestamp());
    let issued_at = now.timestamp();
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO rate_limit_events \
         (key, key_class, scope, event, issued_at, expires_at, retention_class) \
         SELECT ?, ?, ?, ?, ?, ?, ? \
         WHERE NOT EXISTS (SELECT 1 FROM rate_limit_events \
                           WHERE key = ? AND event = ? AND issued_at >= ?)",
        vec![
            Value::from(key.clone()),
            Value::from(key_class(&key)),
            Value::from(CEILING_SCOPE),
            Value::from(CEILING_EVENT),
            Value::from(issued_at),
            Value::from(issued_at + LEDGER_RETENTION_SECS),
            Value::from(LEDGER_RETENTION_CLASS),
            Value::from(key),
            Value::from(CEILING_EVENT),
            Value::from(day_start),
        ],
    ))
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 6, h, m, s).unwrap()
    }

    #[test]
    fn midnight_countdown_runs_to_next_utc_day() {
        assert_eq!(secs_until_midnight_utc(at(0, 0, 0)), 86_400);
        assert_eq!(secs_until_midnight_utc(at(23, 59, 59)), 1);
        assert_eq!(secs_until_midnight_utc(at(12, 0, 0)), 43_200);
    }

    #[test]
    fn writes_key_is_per_user() {
        assert_eq!(writes_key(7), "writes:user:7");
        assert_ne!(writes_key(7), writes_key(8));
    }

    #[tokio::test]
    async fn ceiling_admits_exactly_the_quota_then_refuses_with_one_transition() {
        let store = InMemoryStore::default();
        let now = at(10, 0, 0);
        let mut accepted = 0u64;
        let mut transitions = 0u64;
        for _ in 0..(DAILY_WRITE_CEILING + 3) {
            let c = charge(&store, 1, now).await.unwrap();
            if c.accepted {
                accepted += 1;
            }
            if c.transition {
                transitions += 1;
            }
            assert!(c.retry_after >= 1);
        }
        assert_eq!(accepted, DAILY_WRITE_CEILING);
        assert_eq!(transitions, 1, "the first refusal is the only transition");
    }

    #[tokio::test]
    async fn ceiling_is_per_user() {
        let store = InMemoryStore::default();
        let now = at(10, 0, 0);
        for _ in 0..DAILY_WRITE_CEILING {
            charge(&store, 1, now).await.unwrap();
        }
        assert!(!charge(&store, 1, now).await.unwrap().accepted);
        assert!(charge(&store, 2, now).await.unwrap().accepted);
    }

    #[tokio::test]
    async fn ledger_collects_transition_across_charges() {
        let store = InMemoryStore::default();
        let now = at(10, 0, 0);
        let mut ledger = CeilingLedger::default();
        for _ in 0..DAILY_WRITE_CEILING {
            assert!(ledger.charge(&store, 3, now).await.unwrap().accepted);
        }
        assert!(!ledger.transitioned, "reaching the quota is not a refusal");
        assert!(!ledger.charge(&store, 3, now).await.unwrap().accepted);
        assert!(ledger.transitioned);
    }

    #[test]
    fn ceiling_error_is_429_with_retry_after() {
        let err = ceiling_error(120);
        assert_eq!(err.code, ErrorCode::RateLimited);
        assert_eq!(err.retry_after, Some(120));
    }
}
