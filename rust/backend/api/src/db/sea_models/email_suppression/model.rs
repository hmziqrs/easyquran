use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use ruxlog_types::enums::SuppressionReason;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "email_suppression")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub recipient: String,
    pub reason: SuppressionReason,
    pub source: Option<String>,
    pub diagnostic: Option<String>,
    pub permanent: bool,
    pub last_seen: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
