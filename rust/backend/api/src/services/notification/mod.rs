use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::db::sea_models::{device, notification};
use crate::error::DbResult;
use ruxlog_types::enums::NotificationKind;

pub struct NotificationService {
    db: DatabaseConnection,
    fcm: Option<Arc<rux_fcm::FcmClient>>,
}

impl NotificationService {
    pub fn new(db: DatabaseConnection, fcm: Option<Arc<rux_fcm::FcmClient>>) -> Self {
        Self { db, fcm }
    }

    pub async fn create_and_dispatch(
        &self,
        user_id: i32,
        kind: NotificationKind,
        title: impl Into<String>,
        body: impl Into<String>,
        data: Option<serde_json::Value>,
    ) -> DbResult<notification::Model> {
        dispatch(
            &self.db,
            self.fcm.as_ref(),
            user_id,
            kind,
            title.into(),
            body.into(),
            data,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn notify_user(
    db: &DatabaseConnection,
    fcm: Option<&Arc<rux_fcm::FcmClient>>,
    user_id: i32,
    kind: NotificationKind,
    title: impl Into<String>,
    body: impl Into<String>,
    data: Option<serde_json::Value>,
) -> DbResult<notification::Model> {
    dispatch(db, fcm, user_id, kind, title.into(), body.into(), data).await
}

// In-app insert is the durable record; push failures (no FCM, device load/send error, unregistered token) are best-effort and must not fail the call.
async fn dispatch(
    db: &DatabaseConnection,
    fcm: Option<&Arc<rux_fcm::FcmClient>>,
    user_id: i32,
    kind: NotificationKind,
    title: String,
    body: String,
    data: Option<serde_json::Value>,
) -> DbResult<notification::Model> {
    let model = notification::Entity::create(
        db,
        notification::NewNotification {
            user_id,
            kind,
            title: title.clone(),
            body: body.clone(),
            data: data.clone(),
        },
    )
    .await?;

    let Some(client) = fcm else {
        tracing::warn!(
            user_id,
            notification_id = model.id,
            "FCM client not configured; push skipped (in-app notification persisted)"
        );
        return Ok(model);
    };

    let devices = match device::Entity::list_for_user(db, user_id).await {
        Ok(d) => d,
        Err(err) => {
            tracing::warn!(
                error = %err,
                user_id,
                notification_id = model.id,
                "Failed to load devices for push fan-out; in-app notification persisted"
            );
            return Ok(model);
        }
    };

    if devices.is_empty() {
        return Ok(model);
    }

    let data_map = data.as_ref().and_then(|v| v.as_object()).cloned();

    for dev in devices {
        let msg = rux_fcm::FcmMessage {
            token: dev.token.clone(),
            notification: Some(rux_fcm::Notification {
                title: Some(title.clone()),
                body: Some(body.clone()),
            }),
            data: data_map.clone(),
            android: None,
            webpush: None,
        };
        match client.send(msg).await {
            Ok(name) => {
                tracing::debug!(
                    device_id = dev.id,
                    fcm_name = %name,
                    "FCM delivered"
                );
            }
            Err(rux_fcm::FcmError::Unregistered) => {
                tracing::info!(
                    device_id = dev.id,
                    "FCM token unregistered; pruning stale device"
                );
                if let Err(err) = device::Entity::prune_by_id(db, dev.id).await {
                    tracing::warn!(
                        error = %err,
                        device_id = dev.id,
                        "Failed to prune unregistered device"
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    device_id = dev.id,
                    "FCM send failed; device left in place"
                );
            }
        }
    }

    Ok(model)
}
