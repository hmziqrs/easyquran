use crate::db::sea_models::route_status::Entity as RouteStatus;
use crate::error::ErrorResponse;
use crate::state::AppState;
use axum::extract::State;
use serde_json::json;
use std::error::Error;
use tracing::{debug, info};

pub struct RouteBlockerService;

impl RouteBlockerService {
    pub const BLOCKED_ROUTES_KEY: &'static str = "blocked_routes";
    pub const KNOWN_ROUTES_KEY: &'static str = "known_routes";

    pub async fn record_route_pattern(
        state: &AppState,
        pattern: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Idempotent: ensure_exists returns the existing row without writing if
        // the pattern is already known, so the prior Redis SET dedup is no
        // longer needed. The in-memory route cache picks the row up on the next
        // `sync_all_to_cache` tick; a freshly recorded route is not blocked.
        RouteStatus::ensure_exists(&state.sea_db, pattern)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        debug!(pattern, "Recorded route pattern (DB-backed route status)");
        Ok(())
    }

    pub async fn is_route_blocked(
        State(state): State<AppState>,
        path: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // The blocked/known sets previously lived in Redis (SISMEMBER); with the
        // cache now an internal detail of `route_status::actions` (no public
        // single-key reader), this consults the DB — the source of truth. The
        // middleware owns the call site and may short-circuit via `gate_store`
        // dedup if the per-request lookup becomes a concern.
        let is_blocked = RouteStatus::find_by_pattern(&state.sea_db, path)
            .await
            .map(|model| model.map(|m| m.is_blocked).unwrap_or(false))?;

        Ok(is_blocked)
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

    /// Rebuild the process-global in-memory route cache from the DB. Replaces the
    /// prior `sync_all_to_redis` path — there is no Redis round-trip on the
    /// default build; the cache lives in `route_status::actions::ROUTE_CACHE`,
    /// keyed by `{KNOWN_ROUTES_KEY}:{pattern}` / `{BLOCKED_ROUTES_KEY}:{pattern}`.
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

    /// Initialize (or refresh) the in-memory route cache at startup and on each
    /// periodic sync tick. Replaces the prior `initialize_redis_sync`; no Redis.
    pub async fn initialize_cache(
        state: &AppState,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match Self::sync_all_routes_to_cache(State(state.clone())).await {
            Ok(_) => Ok(()),
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
