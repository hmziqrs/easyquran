use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};

use super::SuppressionReason;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewSuppression {
    pub recipient: String,
    pub reason: SuppressionReason,
    pub source: Option<String>,
    pub diagnostic: Option<String>,
    pub permanent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuppressionUpsert {
    pub reason: SuppressionReason,
    pub source: Option<String>,
    pub diagnostic: Option<String>,
    pub permanent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SuppressionQuery {
    #[serde(default)]
    pub page: Option<u64>,
    pub reason: Option<SuppressionReason>,
    pub permanent: Option<bool>,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromQueryResult)]
pub struct SuppressionListItem {
    pub id: i32,
    pub recipient: String,
    pub reason: SuppressionReason,
    pub permanent: bool,
    pub source: Option<String>,
    pub last_seen: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
}
