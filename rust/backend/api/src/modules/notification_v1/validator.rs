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

/// Closed set of reminder kinds. Serde refuses any other value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderKind {
    Goal,
}

impl ReminderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ReminderKind::Goal => "goal",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1ReminderKindPayload {
    pub kind: ReminderKind,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1ReminderSetPayload {
    pub kind: ReminderKind,
    pub enabled: bool,
    #[validate(length(min = 1, max = 64))]
    pub time_zone: String,
    #[validate(range(min = 0, max = 1439))]
    pub reminder_minute: i32,
    #[validate(range(min = 0, max = 1439))]
    pub quiet_start_minute: Option<i32>,
    #[validate(range(min = 0, max = 1439))]
    pub quiet_end_minute: Option<i32>,
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
