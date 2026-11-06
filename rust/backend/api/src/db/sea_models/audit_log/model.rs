use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Evidence table (A23). No FK to users: an account delete keeps its audit rows.
/// `expires_at` (epoch seconds) and `retention_class` drive the sweeper.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: Option<i32>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub metadata: Option<Json>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
