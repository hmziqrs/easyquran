use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewDevice {
    pub user_id: i32,
    pub token: String,
    pub platform: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceListItem {
    pub id: i32,
    pub platform: String,
    pub token: String,
    pub created_at: DateTimeWithTimeZone,
    pub last_seen_at: DateTimeWithTimeZone,
}
