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

    for dev in devices {
        let msg = private_push_hint(&dev.token);
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
                if let Err(err) = device::Entity::prune_owned_by_id(db, dev.id, user_id).await {
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

fn private_push_hint(token: &str) -> rux_fcm::FcmMessage {
    rux_fcm::FcmMessage {
        token: token.into(),
        notification: Some(rux_fcm::Notification {
            title: Some("EasyQuran".into()),
            body: Some("Open EasyQuran to view notifications.".into()),
        }),
        data: None,
        android: None,
        webpush: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_hint_contains_no_account_or_notification_details() {
        let message = serde_json::to_value(private_push_hint("device-token")).unwrap();
        assert_eq!(
            message,
            serde_json::json!({
                "token": "device-token",
                "notification": {
                    "title": "EasyQuran",
                    "body": "Open EasyQuran to view notifications."
                }
            })
        );
    }

    #[tokio::test]
    async fn private_details_remain_durable_and_scoped_to_recipient() {
        use sea_orm::{ConnectionTrait, Database};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE notifications (
                id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, kind TEXT NOT NULL,
                title TEXT NOT NULL, body TEXT NOT NULL, data TEXT, read_at TEXT,
                created_at TEXT NOT NULL
            )",
        )
        .await
        .unwrap();
        let private_data = serde_json::json!({"payment_id":"private-payment", "url":"/private"});
        NotificationService::new(db.clone(), None)
            .create_and_dispatch(
                7,
                NotificationKind::PaymentReceived,
                "Private title",
                "Private body",
                Some(private_data.clone()),
            )
            .await
            .unwrap();
        let recipient = notification::Entity::list_for_user(&db, 7, 1, 20)
            .await
            .unwrap();
        assert_eq!(recipient.data[0].title, "Private title");
        assert_eq!(recipient.data[0].body, "Private body");
        assert_eq!(recipient.data[0].data.as_ref(), Some(&private_data));
        assert!(notification::Entity::list_for_user(&db, 8, 1, 20)
            .await
            .unwrap()
            .data
            .is_empty());
    }
}
