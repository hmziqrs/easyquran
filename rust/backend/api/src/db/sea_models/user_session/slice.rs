use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserSession {
    pub user_id: i32,
    pub device: Option<String>,
    pub ip_address: Option<String>,
}

impl NewUserSession {
    pub fn new(user_id: i32, device: Option<String>, ip_address: Option<String>) -> Self {
        Self {
            user_id,
            device,
            ip_address,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserSession {
    pub last_seen: Option<DateTimeWithTimeZone>,
    pub revoked_at: Option<DateTimeWithTimeZone>,
}

impl UpdateUserSession {
    pub fn touch() -> Self {
        let now = chrono::Utc::now().fixed_offset();
        Self {
            last_seen: Some(now),
            revoked_at: None,
        }
    }

    pub fn revoke() -> Self {
        let now = chrono::Utc::now().fixed_offset();
        Self {
            last_seen: Some(now),
            revoked_at: Some(now),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUserSessionQuery {
    pub page_no: Option<i64>,
    pub user_id: Option<i32>,
    pub device: Option<String>,
    pub ip_address: Option<String>,
    pub active_only: Option<bool>,
    pub seen_since: Option<DateTimeWithTimeZone>,
    pub revoked_since: Option<DateTimeWithTimeZone>,
    pub sort_by: Option<Vec<String>>,
    pub sort_order: Option<String>,
}
