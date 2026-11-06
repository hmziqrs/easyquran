use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Subject of a legal hold. Kind and id are copied at hold time, so the hold still
/// names the subject after the source row is gone. No FK to users.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "legal_hold_subjects")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub hold_id: i64,
    pub subject_kind: String,
    pub subject_id: String,
    pub subject_user_id: Option<i32>,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
