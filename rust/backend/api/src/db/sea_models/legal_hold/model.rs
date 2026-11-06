use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Legal hold (A25). The sweeper (A32) must skip any row here whose hold is active.
/// `auto_release` is 0 for the CSAM hold: it is never released by expiry. No FK to users.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "legal_holds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub case_ref: Option<String>,
    pub authority: String,
    pub reason: String,
    pub owner: String,
    pub auto_release: bool,
    pub placed_at: i64,
    pub preserve_until: Option<i64>,
    pub renewals: i64,
    pub released_at: Option<i64>,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
