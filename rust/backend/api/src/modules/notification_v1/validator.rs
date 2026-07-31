use serde::{Deserialize, Serialize};
use validator::Validate;

use ruxlog_types::enums::NotificationKind;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1ListNotificationsPayload {
    #[validate(range(min = 0, max = 10_000))]
    pub page: Option<u64>,
    #[validate(range(min = 0, max = 100))]
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1MarkReadPayload {
    #[validate(range(min = 1))]
    pub id: i32,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1AdminCreateNotificationPayload {
    #[validate(range(min = 1))]
    pub user_id: i32,
    pub kind: NotificationKind,
    #[validate(length(min = 1, max = 256))]
    pub title: String,
    #[validate(length(min = 1, max = 2048))]
    pub body: String,
    pub data: Option<serde_json::Value>,
}
