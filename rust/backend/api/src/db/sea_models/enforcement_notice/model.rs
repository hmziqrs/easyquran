use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Case record for one ban decision (A19). No FK to users or user_bans: the
// notice outlives the account and the ban row. Timestamps are epoch seconds.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "enforcement_notices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i32,
    pub ban_id: i32,
    pub reason_code: String,
    pub reason_text: Option<String>,
    pub corroborator: String,
    pub decided_by: i32,
    pub decided_at: i64,
    pub effective_at: i64,
    pub expires_at: i64,
    pub delivery_state: Option<String>,
    pub sent_at: Option<i64>,
    pub appeal_token_hash: Option<String>,
    pub created_at: i64,
    pub retention_class: String,
    pub notice_channel: String,
    pub shared_device_checked: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
