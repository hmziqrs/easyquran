use crate::db::sea_models::route_status::Entity as RouteStatus;
use crate::error::ErrorResponse;
use crate::state::AppState;
use axum::extract::State;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{OnceLock, RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

// In-memory route-status snapshot. The middleware ran TWO SQLite queries per
// request (ensure_exists + find_by_pattern); with fail-closed semantics, DB
// contention turned directly into whole-API 503s. The snapshot answers both
// from memory and refreshes from the DB at most once per TTL window.
const SNAPSHOT_TTL: Duration = Duration::from_secs(5);

struct RouteStatusSnapshot {
    blocked: HashMap<String, bool>,
    refreshed_at: Option<Instant>,
}

static SNAPSHOT: OnceLock<RwLock<RouteStatusSnapshot>> = OnceLock::new();

fn snapshot() -> &'static RwLock<RouteStatusSnapshot> {
    SNAPSHOT.get_or_init(|| {
        RwLock::new(RouteStatusSnapshot {
            blocked: HashMap::new(),
            refreshed_at: None,
        })
    })
}

fn lock_poisoned<T>(e: std::sync::PoisonError<T>) -> Box<dyn Error + Send + Sync> {
    format!("route blocker snapshot lock poisoned: {e}").into()
}

fn snapshot_read(
) -> Result<RwLockReadGuard<'static, RouteStatusSnapshot>, Box<dyn Error + Send + Sync>> {
    snapshot().read().map_err(lock_poisoned)
}

async fn refresh_snapshot(db: &DatabaseConnection) -> Result<(), Box<dyn Error + Send + Sync>> {
    let routes = RouteStatus::find().all(db).await?;
    let mut snap = snapshot().write().map_err(lock_poisoned)?;
    snap.blocked = routes
        .into_iter()
        .map(|r| (r.route_pattern, r.is_blocked))
        .collect();
    snap.refreshed_at = Some(Instant::now());
    Ok(())
}

#[cfg(test)]
fn reset_snapshot_for_tests() {
    let mut snap = snapshot().write().unwrap();
    snap.blocked.clear();
    snap.refreshed_at = None;
}

#[cfg(test)]
fn seed_snapshot_for_tests(blocked: HashMap<String, bool>, refreshed_at: Option<Instant>) {
    let mut snap = snapshot().write().unwrap();
    snap.blocked = blocked;
    snap.refreshed_at = refreshed_at;
}

pub struct RouteBlockerService;

impl RouteBlockerService {
    pub const BLOCKED_ROUTES_KEY: &'static str = "blocked_routes";
    pub const KNOWN_ROUTES_KEY: &'static str = "known_routes";

    pub async fn record_route_pattern(
        db: &DatabaseConnection,
        pattern: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Ok(snap) = snapshot_read() {
            if snap.refreshed_at.is_some() && snap.blocked.contains_key(pattern) {
                return Ok(());
            }
        }
        RouteStatus::ensure_exists(db, pattern)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        if let Ok(mut snap) = snapshot().write() {
            snap.blocked.entry(pattern.to_string()).or_insert(false);
        }
        debug!(pattern, "Recorded route pattern (DB-backed route status)");
        Ok(())
    }

    pub async fn is_route_blocked(
        db: &DatabaseConnection,
        path: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let have_snapshot = {
            let snap = snapshot_read()?;
            if snap
                .refreshed_at
                .is_some_and(|at| at.elapsed() < SNAPSHOT_TTL)
            {
                return Ok(snap.blocked.get(path).copied().unwrap_or(false));
            }
            snap.refreshed_at.is_some()
        };
        if let Err(err) = refresh_snapshot(db).await {
            if !have_snapshot {
                return Err(err);
            }
            // Stale-but-present: keep serving the last known state (TTL-bounded
            // retry) instead of amplifying DB contention into whole-API 503s.
            warn!(error = %err, "route blocker snapshot refresh failed; serving stale snapshot");
        }
        Ok(snapshot_read()?.blocked.get(path).copied().unwrap_or(false))
    }

    // Mutations refresh the in-memory snapshot too so an admin block/unblock
    // takes effect immediately, not on the next TTL boundary.
    async fn refresh_snapshot_best_effort(state: &AppState) {
        if let Err(err) = refresh_snapshot(&state.sea_db).await {
            error!(error = %err, "route blocker snapshot refresh after mutation failed (TTL will retry)");
        }
    }

    pub async fn block_route(
        State(state): State<AppState>,
        pattern: String,
        reason: Option<String>,
    ) -> Result<serde_json::Value, ErrorResponse> {
        let route = RouteStatus::create_or_update(&state.sea_db, pattern.clone(), true, reason)
            .await
            .map_err(|e| {
                ErrorResponse::new(crate::error::ErrorCode::InternalServerError)
                    .with_message(e.to_string())
            })?;

        Self::sync_all_routes_to_cache(State(state.clone())).await?;
        Self::refresh_snapshot_best_effort(&state).await;

        info!(pattern, "Route blocked");
        Ok(json!(route))
    }

    pub async fn unblock_route(
        State(state): State<AppState>,
        pattern: String,
    ) -> Result<serde_json::Value, ErrorResponse> {
        let route = RouteStatus::create_or_update(&state.sea_db, pattern.clone(), false, None)
            .await
            .map_err(|e| {
                ErrorResponse::new(crate::error::ErrorCode::InternalServerError)
                    .with_message(e.to_string())
            })?;

        Self::sync_all_routes_to_cache(State(state.clone())).await?;
        Self::refresh_snapshot_best_effort(&state).await;

        info!(pattern, "Route unblocked");
        Ok(json!(route))
    }

    pub async fn delete_route(
        State(state): State<AppState>,
        pattern: String,
    ) -> Result<serde_json::Value, ErrorResponse> {
        RouteStatus::delete_by_pattern(&state.sea_db, &pattern)
            .await
            .map_err(|e| {
                ErrorResponse::new(crate::error::ErrorCode::InternalServerError)
                    .with_message(e.to_string())
            })?;

        Self::sync_all_routes_to_cache(State(state.clone())).await?;
        Self::refresh_snapshot_best_effort(&state).await;

        Ok(json!({ "message": "Route deleted successfully" }))
    }

    pub async fn list_blocked_routes(
        State(state): State<AppState>,
    ) -> Result<Vec<crate::db::sea_models::route_status::Model>, ErrorResponse> {
        RouteStatus::find_blocked_routes(&state.sea_db)
            .await
            .map_err(|e| {
                ErrorResponse::new(crate::error::ErrorCode::InternalServerError)
                    .with_message(e.to_string())
            })
    }

    pub async fn sync_all_routes_to_cache(
        State(state): State<AppState>,
    ) -> Result<serde_json::Value, ErrorResponse> {
        RouteStatus::sync_all_to_cache(
            &state.sea_db,
            Self::KNOWN_ROUTES_KEY,
            Self::BLOCKED_ROUTES_KEY,
        )
        .await
        .map_err(|e| {
            ErrorResponse::new(crate::error::ErrorCode::InternalServerError)
                .with_message(format!("Route cache sync failed: {}", e))
        })?;

        Ok(json!({ "message": "All routes synced to cache successfully" }))
    }

    pub async fn initialize_cache(state: &AppState) -> Result<(), Box<dyn Error + Send + Sync>> {
        match Self::sync_all_routes_to_cache(State(state.clone())).await {
            Ok(_) => {
                refresh_snapshot(&state.sea_db).await?;
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to initialize route blocker cache: {}", e);
                Err(Box::new(std::io::Error::other(format!(
                    "Route cache sync failed: {}",
                    e
                ))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::config::settings::TEST_ENV_MUTEX.lock().unwrap()
    }

    // sqlite::memory: with no route_status table → every DB read fails, so any
    // answer that comes back was served from the snapshot, not the DB.
    async fn broken_db() -> DatabaseConnection {
        sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db")
    }

    #[tokio::test]
    async fn no_snapshot_and_broken_db_fails_closed() {
        let _g = env_lock();
        reset_snapshot_for_tests();
        let db = broken_db().await;
        let result = RouteBlockerService::is_route_blocked(&db, "/admin/route/v1/x").await;
        assert!(
            result.is_err(),
            "never-refreshed snapshot + DB error must fail closed"
        );
        reset_snapshot_for_tests();
    }

    #[tokio::test]
    async fn fresh_snapshot_answers_without_touching_the_db() {
        let _g = env_lock();
        reset_snapshot_for_tests();
        let mut blocked = HashMap::new();
        blocked.insert("/admin/route/v1/x".to_string(), true);
        seed_snapshot_for_tests(blocked, Some(Instant::now()));

        let db = broken_db().await;
        let is_blocked = RouteBlockerService::is_route_blocked(&db, "/admin/route/v1/x")
            .await
            .expect("fresh snapshot must answer without a DB round-trip");
        assert!(
            is_blocked,
            "blocked pattern must be served from the snapshot"
        );
        let other = RouteBlockerService::is_route_blocked(&db, "/some/other/route")
            .await
            .expect("unknown pattern must resolve to not-blocked from the snapshot");
        assert!(!other);
        reset_snapshot_for_tests();
    }

    #[tokio::test]
    async fn stale_snapshot_serves_last_known_state_on_db_error() {
        let _g = env_lock();
        reset_snapshot_for_tests();
        let mut blocked = HashMap::new();
        blocked.insert("/admin/route/v1/x".to_string(), true);
        seed_snapshot_for_tests(
            blocked,
            Some(Instant::now() - SNAPSHOT_TTL - Duration::from_secs(1)),
        );

        let db = broken_db().await;
        let is_blocked = RouteBlockerService::is_route_blocked(&db, "/admin/route/v1/x")
            .await
            .expect("stale-but-present snapshot must serve last known state, not 503");
        assert!(is_blocked);
        reset_snapshot_for_tests();
    }

    #[tokio::test]
    async fn record_skips_db_once_the_pattern_is_known() {
        let _g = env_lock();
        reset_snapshot_for_tests();
        let mut blocked = HashMap::new();
        blocked.insert("/known/route".to_string(), false);
        seed_snapshot_for_tests(blocked, Some(Instant::now()));

        let db = broken_db().await;
        RouteBlockerService::record_route_pattern(&db, "/known/route")
            .await
            .expect("known pattern must not hit the DB");
        // Unknown pattern with a broken DB: best-effort write fails but the
        // snapshot itself must record the attempt path taken (error surfaces to
        // the middleware's existing error log).
        assert!(
            RouteBlockerService::record_route_pattern(&db, "/unknown/route").await.is_err(),
            "unknown pattern + broken DB must surface the write error"
        );
        reset_snapshot_for_tests();
    }
}
