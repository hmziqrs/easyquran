use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PasskeyCredentialView {
    pub id: i32,
    pub credential_id: String,
    pub device_type: Option<String>,
    pub transports: Option<serde_json::Value>,
    pub created_at: DateTimeWithTimeZone,
    pub last_used_at: Option<DateTimeWithTimeZone>,
}

impl super::Model {
    pub fn into_view(self) -> PasskeyCredentialView {
        PasskeyCredentialView {
            id: self.id,
            credential_id: self.credential_id,
            device_type: self.device_type,
            transports: self.transports,
            created_at: self.created_at,
            last_used_at: self.last_used_at,
        }
    }
}
