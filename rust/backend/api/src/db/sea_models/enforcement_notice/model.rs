use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Case record for one enforcement decision: a ban (A19) or a comment action (A27). No FK to users or user_bans: the
// notice outlives the account and the ban row. Timestamps are epoch seconds.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "enforcement_notices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i32,
    /// Set for `subject_kind = "ban"`; `None` for comment notices (A27).
    pub ban_id: Option<i32>,
    /// `"ban"` or `"comment"` (A27). Rows written before A27 are `"ban"`.
    pub subject_kind: String,
    /// Set for `subject_kind = "comment"`: the comment hidden or removed.
    pub comment_id: Option<i32>,
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
    /// True only when a machine made the decision. Admin actions are always false.
    pub automated: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
