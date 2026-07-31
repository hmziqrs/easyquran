use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewOauthIdentity {
    pub user_id: i32,
    pub provider: String,
    pub provider_user_id: String,
    pub created_at: DateTimeWithTimeZone,
}
