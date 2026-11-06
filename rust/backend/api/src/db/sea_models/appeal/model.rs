use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Appeal against one enforcement notice (A35). No FK to users or notices: the
// appeal outlives the account. Timestamps are epoch seconds. Retention follows
// the notice it answers (enforcement_12m).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "appeals")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub notice_id: i64,
    pub user_id: i32,
    pub statement: String,
    pub status: String,
    pub submitted_at: i64,
    pub due_at: i64,
    pub decided_at: Option<i64>,
    pub created_at: i64,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
