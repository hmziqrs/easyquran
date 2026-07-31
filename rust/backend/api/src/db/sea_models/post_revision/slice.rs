use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatePostRevision {
    pub post_id: i32,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostRevisionListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromQueryResult)]
pub struct PostRevisionSummary {
    pub id: i32,
    pub post_id: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestorePostRevision {
    pub post_id: i32,
    pub revision_id: i32,
}
