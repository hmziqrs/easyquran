use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Append-only evidence row (A25). One row per captured object. No FK to users and
/// no cascade: an account delete leaves the case intact. `expires_at` (epoch seconds)
/// and `retention_class` drive the A32 sweeper.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "case_evidence")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub case_ref: String,
    pub kind: String,
    pub subject_id: String,
    pub snapshot: Json,
    pub captured_at: i64,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
