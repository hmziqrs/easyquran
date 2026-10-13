use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bookmarks")]
pub struct Model {
    /// Composite PK (user_id, id): client-minted ids never cross accounts.
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Always a live folder of this user (unknown ids land in the default).
    pub folder_id: String,
    /// Translation context metadata; never part of the uniqueness key.
    pub source_id: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    /// Range end; stored as `end = start` for single-ayah rows.
    pub end_surah: i32,
    pub end_ayah: i32,
    /// Tombstone marker: deleted rows keep protecting their key.
    pub deleted_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::UserId",
        to = "super::super::user::Column::Id"
    )]
    User,
}

impl Related<super::super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
