//! `reading_daily` entity (next-features Feature 6) — the retention-proof
//! per-day rollup: ayahs read and seconds per surah per device-local day.
//! Survives the raw-event purge, so long-horizon summaries and day counts
//! stay answerable. Flat sibling file, one Model per table.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_daily")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    /// Client-supplied device-local YYYY-MM-DD (EC-36) — never derived here.
    #[sea_orm(primary_key, auto_increment = false)]
    pub local_day: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub surah: i32,
    pub ayahs_read: i64,
    pub seconds: i64,
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
