//! `reading_events` entity (next-features Feature 6) — append-only raw
//! reading events. Flat sibling file, one Model per table (same style as the
//! reading-session trio); all SQL lives in `user_analytics_v1` (same split as
//! `analytics_v1`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// `kind` is the wire literal ("ayah.read" | "ayah.open"); `seconds` only
/// ever set on reads. Rows are purged after the bounded retention window
/// (controller-owned), so nothing may treat this table as durable history —
/// that is `reading_daily`'s job.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub surah: i32,
    pub ayah: i32,
    pub source_id: Option<String>,
    pub kind: String,
    pub seconds: Option<i64>,
    /// Client-supplied device-local YYYY-MM-DD (EC-36) — never derived here.
    pub local_day: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
