use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

use super::model::ScheduledPostStatus;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateScheduledPost {
    pub post_id: i32,
    pub publish_at: DateTimeWithTimeZone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ScheduledPostStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpsertScheduledPost {
    pub post_id: i32,
    pub publish_at: DateTimeWithTimeZone,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledPostStatusQuery {
    pub status: ScheduledPostStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledPostDueQuery {
    pub until: DateTimeWithTimeZone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}
