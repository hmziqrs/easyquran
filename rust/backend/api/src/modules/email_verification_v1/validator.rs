use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::db::sea_models::email_verification::AdminEmailVerificationQuery;

// Must match `email_verification::Entity::generate_code`'s 8-char code length.
const CODE_LEN: u64 = 8;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1VerifyPayload {
    #[validate(length(min = CODE_LEN, max = CODE_LEN))]
    pub code: String,
}

#[derive(Debug, Deserialize, Serialize, Validate, Clone)]
pub struct V1AdminEmailVerificationListPayload {
    #[serde(default)]
    pub page_no: Option<i64>,
    #[serde(default)]
    pub user_id: Option<i32>,
    #[serde(default)]
    pub code_hash: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTimeWithTimeZone>,
    #[serde(default)]
    pub updated_at: Option<DateTimeWithTimeZone>,
    #[serde(default)]
    pub sort_by: Option<Vec<String>>,
    #[serde(default)]
    pub sort_order: Option<String>,
}

impl V1AdminEmailVerificationListPayload {
    pub fn into_query(self) -> AdminEmailVerificationQuery {
        AdminEmailVerificationQuery {
            page_no: self.page_no,
            user_id: self.user_id,
            code_hash: self.code_hash,
            created_at: self.created_at,
            updated_at: self.updated_at,
            sort_by: self.sort_by,
            sort_order: self.sort_order,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1AdminEmailVerificationUserPayload {
    #[validate(range(min = 1))]
    pub user_id: i32,
}
