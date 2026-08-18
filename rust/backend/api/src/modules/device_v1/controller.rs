use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use serde_json::json;
use tracing::{instrument, warn};

use crate::{
    db::sea_models::device,
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{V1DeleteDevicePayload, V1RegisterDevicePayload};

/// Hard ceiling per user; registration beyond it evicts the oldest devices so token churn can't grow the fan-out set unbounded.
const MAX_DEVICES_PER_USER: usize = 20;

/// list_for_user is created_at DESC — everything past the cap is the oldest tail.
async fn evict_beyond_cap(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
) -> crate::error::DbResult<()> {
    let devices = device::Entity::list_for_user(db, user_id).await?;
    if let Some(stale) = devices.get(MAX_DEVICES_PER_USER..) {
        for dev in stale {
            device::Entity::prune_by_id(db, dev.id).await?;
        }
    }
    Ok(())
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn register(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1RegisterDevicePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let model = device::Entity::upsert(
        &state.sea_db,
        device::NewDevice {
            user_id: user.id,
            token: payload.token.clone(),
            platform: payload.platform.clone(),
        },
    )
    .await?;

    // Best-effort: an eviction failure must not fail the registration itself.
    if let Err(err) = evict_beyond_cap(&state.sea_db, user.id).await {
        warn!(
            error = %err,
            user_id = user.id,
            "Failed to evict devices over per-user cap"
        );
    }

    Ok((StatusCode::OK, Json(json!({ "id": model.id }))))
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn list(
    state: State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let devices = device::Entity::list_for_user(&state.sea_db, user.id).await?;
    Ok((StatusCode::OK, Json(json!({ "devices": devices }))))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn delete(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1DeleteDevicePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let removed = device::Entity::delete_for_user(&state.sea_db, user.id, &payload.token).await?;
    if removed == 0 {
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
            .with_message("No device found for this token"));
    }
    Ok((StatusCode::OK, Json(json!({ "removed": removed }))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    async fn test_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite connects");
        db.execute_unprepared(
            r#"CREATE TABLE "devices" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER NOT NULL,
                "token" TEXT NOT NULL,
                "platform" TEXT NOT NULL,
                "created_at" TEXT NOT NULL,
                "updated_at" TEXT NOT NULL,
                "last_seen_at" TEXT NOT NULL
            )"#,
        )
        .await
        .expect("devices table created");
        db
    }

    async fn seed(db: &sea_orm::DatabaseConnection, user_id: i32, token: &str) {
        device::Entity::upsert(
            db,
            device::NewDevice {
                user_id,
                token: token.to_string(),
                platform: "web".to_string(),
            },
        )
        .await
        .expect("seed upsert");
    }

    #[tokio::test]
    async fn evict_beyond_cap_keeps_20_newest() {
        let db = test_db().await;
        for i in 0..(MAX_DEVICES_PER_USER + 3) {
            seed(&db, 7, &format!("t{i}")).await;
        }

        evict_beyond_cap(&db, 7).await.expect("evict");

        let devices = device::Entity::list_for_user(&db, 7).await.expect("list");
        assert_eq!(devices.len(), MAX_DEVICES_PER_USER);
        assert!(
            !devices.iter().any(|d| d.token == "t0"),
            "oldest device must be evicted first"
        );
        assert!(
            devices.iter().any(|d| d.token == "t22"),
            "newest device must survive"
        );
    }

    #[tokio::test]
    async fn evict_beyond_cap_is_noop_at_or_below_cap() {
        let db = test_db().await;
        seed(&db, 1, "only").await;

        evict_beyond_cap(&db, 1).await.expect("evict");

        let devices = device::Entity::list_for_user(&db, 1).await.expect("list");
        assert_eq!(devices.len(), 1);
    }
}
