use std::cmp::Ordering;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::Utc;
use eq_sync_core::{compare_stamps, format_stamp, parse_stamp};
use sea_orm::prelude::DateTimeWithTimeZone;
use serde::Serialize;
use serde_json::json;
use tracing::{instrument, warn};

use crate::{
    db::sea_models::{
        device,
        user_device::{self, NewUserDevice},
        user_session,
    },
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    modules::auth_v1::controller::terminate_session,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{
    check_registration_shape, normalise_device_name, V1DeleteTokenPayload, V1DeviceRefPayload,
    V1RegisterDevicePayload, V1RenameDevicePayload,
};

/// Stamps further ahead of the server than this are rewritten to server + 60 s,
/// so one bad clock cannot pin a name forever.
const CLOCK_SKEW_ALLOWANCE_MS: i64 = 60_000;

/// What the client sees of one device. No push token, no UA, no IP, and no
/// session ids: the list is for the account owner only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub display_name: Option<String>,
    pub platform: String,
    pub client: String,
    pub form_factor: String,
    pub browser: Option<String>,
    pub os_major: Option<i32>,
    pub app_version: Option<String>,
    pub last_seen_at: DateTimeWithTimeZone,
    pub is_current: bool,
}

fn invalid(msg: &'static str) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidInput).with_message(msg)
}

fn removed_device() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::DuplicateEntry)
        .with_message("Device was removed. Register a new device id.")
}

fn bounded_name_stamp(stamp: &str, server_ms: i64) -> Option<String> {
    let parsed = parse_stamp(stamp)?;
    let ceiling = server_ms + CLOCK_SKEW_ALLOWANCE_MS;
    if parsed.ms as i64 > ceiling {
        return Some(format_stamp(ceiling as u64, parsed.counter, &parsed.device));
    }
    Some(stamp.to_string())
}

/// The session the caller is using, as an audit row id. None without a binding.
async fn current_session_id(state: &AppState, auth: &AuthSession) -> Option<i32> {
    let id = auth.session().id()?;
    auth.backend()
        .lookup_session_mapping_by_tower(&state.sea_db, &id.to_string())
        .await
}

/// The device the caller is signed in on, from its session's binding.
async fn current_device_id(state: &AppState, auth: &AuthSession) -> Option<String> {
    let session_id = current_session_id(state, auth).await?;
    let row = <user_session::Entity as sea_orm::EntityTrait>::find_by_id(session_id)
        .one(&state.sea_db)
        .await
        .ok()??;
    row.device_id
}

/// Remove a device's access: push tokens first (a signed-out device must not
/// keep getting notifications), then each live session through the one
/// session-kill path. Idempotent, so a failed call can be retried.
async fn drop_device_access(
    state: &AppState,
    auth: &AuthSession,
    user_id: i32,
    device_id: &str,
) -> Result<(), ErrorResponse> {
    device::Entity::delete_for_device(&state.sea_db, user_id, device_id).await?;
    for session in user_session::Entity::live_for_device(&state.sea_db, user_id, device_id).await? {
        terminate_session(state, auth, session.id).await?;
    }
    Ok(())
}

/// Retire a device that lost its place under the cap: no access, no listing.
async fn retire_device(
    state: &AppState,
    auth: &AuthSession,
    user_id: i32,
    device_id: &str,
) -> Result<(), ErrorResponse> {
    drop_device_access(state, auth, user_id, device_id).await?;
    if let Some(row) = user_device::Entity::find_row(&state.sea_db, user_id, device_id).await? {
        user_device::Entity::retire(&state.sea_db, row).await?;
    }
    Ok(())
}

/// Best-effort, like the old push-token eviction: a failure here must not fail
/// the registration that triggered it.
async fn enforce_cap(state: &AppState, auth: &AuthSession, user_id: i32, caller: &str) {
    let live = match user_device::Entity::live_for_user(&state.sea_db, user_id).await {
        Ok(rows) => rows,
        Err(_) => {
            warn!(user_id, "device cap check failed to load live devices");
            return;
        }
    };
    for victim in user_device::cap_victims(&live, caller, user_device::LIVE_DEVICE_CAP) {
        if retire_device(state, auth, user_id, &victim).await.is_err() {
            warn!(
                user_id,
                "device cap eviction failed; will retry on next register"
            );
        }
    }
}

fn ensure_expected_owner(user_id: i32, expected_user_id: i32) -> Result<(), ErrorResponse> {
    if user_id != expected_user_id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Push subscription account changed"));
    }
    Ok(())
}

/// Idempotent upsert of the caller's device, the optional push token, and the
/// current session's binding. Logs nothing that identifies the device.
#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn register(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1RegisterDevicePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    ensure_expected_owner(user.id, payload.expected_user_id)?;
    check_registration_shape(&payload).map_err(invalid)?;

    let db = &state.sea_db;
    let existing = user_device::Entity::find_row(db, user.id, &payload.device_id).await?;
    if existing
        .as_ref()
        .is_some_and(|row| row.revoked_at.is_some())
    {
        return Err(removed_device());
    }
    let row = user_device::Entity::register(
        db,
        NewUserDevice {
            user_id: user.id,
            id: payload.device_id.clone(),
            platform: payload.platform.clone(),
            client: payload.client.clone(),
            form_factor: payload.form_factor.clone(),
            browser: payload.browser.clone(),
            os_major: payload.os_major,
            app_version: payload.app_version.clone(),
        },
        existing,
    )
    .await?;

    if let Some(token) = payload.token.clone() {
        device::Entity::upsert(
            db,
            device::NewDevice {
                user_id: user.id,
                token,
                platform: payload.platform.clone(),
                device_id: Some(row.id.clone()),
            },
        )
        .await?;
    }

    if let Some(session_id) = current_session_id(&state, &auth).await {
        user_session::Entity::bind_device(db, session_id, user.id, &row.id).await?;
    }

    enforce_cap(&state, &auth, user.id, &row.id).await;
    Ok((StatusCode::OK, Json(json!({ "registered": true }))))
}

/// The caller's devices that still have a live session. Retired and revoked rows
/// are hidden. Newest first.
#[debug_handler]
#[instrument(skip(state, auth), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn list(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    let db = &state.sea_db;
    let with_session = user_session::Entity::live_device_ids(db, user.id).await?;
    let current = current_device_id(&state, &auth).await;
    let rows = user_device::Entity::live_for_user(db, user.id).await?;

    let mut devices: Vec<DeviceView> = rows
        .into_iter()
        .filter(|row| with_session.binary_search(&row.id).is_ok())
        .map(|row| DeviceView {
            is_current: current.as_deref() == Some(row.id.as_str()),
            id: row.id,
            display_name: row.display_name,
            platform: row.platform,
            client: row.client,
            form_factor: row.form_factor,
            browser: row.browser,
            os_major: row.os_major,
            app_version: row.app_version,
            last_seen_at: row.last_seen_at,
        })
        .collect();
    devices.reverse();
    Ok((StatusCode::OK, Json(json!({ "devices": devices }))))
}

/// Highest stamp wins. A stale rename returns `renamed: false` and changes nothing.
#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn rename(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1RenameDevicePayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    ensure_expected_owner(user.id, payload.expected_user_id)?;
    let name = normalise_device_name(&payload.name).map_err(invalid)?;
    let stamp = bounded_name_stamp(&payload.name_stamp, Utc::now().timestamp_millis())
        .ok_or_else(|| invalid("name_stamp is not a valid stamp"))?;

    let row = user_device::Entity::find_row(&state.sea_db, user.id, &payload.device_id)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    if row.revoked_at.is_some() {
        return Err(removed_device());
    }
    if compare_stamps(Some(stamp.as_str()), row.name_stamp.as_deref()) != Ordering::Greater {
        return Ok((StatusCode::OK, Json(json!({ "renamed": false }))));
    }
    user_device::Entity::set_name(&state.sea_db, row, name, stamp).await?;
    Ok((StatusCode::OK, Json(json!({ "renamed": true }))))
}

/// Revoke a device: kill its sessions through the sessions_terminate path, delete
/// its push tokens, then stamp `revoked_at`. Its id can never register again.
#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn delete(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1DeviceRefPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    ensure_expected_owner(user.id, payload.expected_user_id)?;
    let row = user_device::Entity::find_row(&state.sea_db, user.id, &payload.device_id)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    if row.revoked_at.is_none() {
        drop_device_access(&state, &auth, user.id, &row.id).await?;
        user_device::Entity::revoke(&state.sea_db, row).await?;
    }
    Ok((StatusCode::OK, Json(json!({ "revoked": true }))))
}

/// Sign-out on a device: drop that device's push token only. The device row and
/// its sessions are left for the sign-out path to end.
#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn delete_token(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1DeleteTokenPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    ensure_expected_owner(user.id, payload.expected_user_id)?;
    let removed = device::Entity::delete_for_user(&state.sea_db, user.id, &payload.token).await?;
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
                "token" TEXT NOT NULL UNIQUE,
                "platform" TEXT NOT NULL,
                "device_id" TEXT,
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
                device_id: None,
            },
        )
        .await
        .expect("seed upsert");
    }

    #[tokio::test]
    async fn push_token_registration_transfers_ownership_without_duplicate_delivery() {
        let db = test_db().await;
        seed(&db, 1, "shared-browser").await;
        seed(&db, 2, "shared-browser").await;
        let transferred = device::Entity::list_for_user(&db, 2).await.unwrap();
        device::Entity::prune_owned_by_id(&db, transferred[0].id, 1)
            .await
            .unwrap();
        assert!(device::Entity::list_for_user(&db, 1)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            device::Entity::delete_for_user(&db, 1, "shared-browser")
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            device::Entity::list_for_user(&db, 2).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn push_tokens_drop_with_their_device() {
        let db = test_db().await;
        device::Entity::upsert(
            &db,
            device::NewDevice {
                user_id: 3,
                token: "t-owned".into(),
                platform: "web".into(),
                device_id: Some("dev-a".into()),
            },
        )
        .await
        .unwrap();
        seed(&db, 3, "t-other").await;
        assert_eq!(
            device::Entity::delete_for_device(&db, 3, "dev-a")
                .await
                .unwrap(),
            1
        );
        let left = device::Entity::list_for_user(&db, 3).await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].token, "t-other");
    }

    #[test]
    fn stale_registration_and_deletion_owner_are_rejected() {
        assert!(ensure_expected_owner(2, 1).is_err());
        assert!(ensure_expected_owner(2, 2).is_ok());
    }

    #[test]
    fn far_future_name_stamps_are_pulled_back_to_server_plus_skew() {
        let now = 1_700_000_000_000_i64;
        let far = format_stamp((now + 10_000_000) as u64, 3, "dev-1");
        let bounded = bounded_name_stamp(&far, now).unwrap();
        let parsed = parse_stamp(&bounded).unwrap();
        assert_eq!(parsed.ms as i64, now + CLOCK_SKEW_ALLOWANCE_MS);
        assert_eq!(parsed.device, "dev-1");

        let sane = format_stamp(now as u64, 0, "dev-1");
        assert_eq!(bounded_name_stamp(&sane, now).unwrap(), sane);
        assert!(bounded_name_stamp("not-a-stamp", now).is_none());
    }
}
