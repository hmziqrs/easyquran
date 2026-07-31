use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};

use super::SubscriberStatus;
use crate::utils::SortParam;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewSubscriber {
    pub email: String,
    pub status: SubscriberStatus,
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateSubscriber {
    pub status: Option<SubscriberStatus>,
    pub token: Option<String>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SubscriberQuery {
    pub page: Option<u64>,
    pub search: Option<String>,
    pub status: Option<SubscriberStatus>,
    pub sorts: Option<Vec<SortParam>>,
    pub created_at_gt: Option<DateTimeWithTimeZone>,
    pub created_at_lt: Option<DateTimeWithTimeZone>,
    pub updated_at_gt: Option<DateTimeWithTimeZone>,
    pub updated_at_lt: Option<DateTimeWithTimeZone>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromQueryResult)]
pub struct SubscriberListItem {
    pub id: i32,
    pub email: String,
    pub status: SubscriberStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}
