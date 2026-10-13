use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bookmark_folders")]
pub struct Model {
    /// Composite PK (user_id, id): the same client-minted id under two
    /// accounts is two folders, never a collision.
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    /// Server-owned: true only for the id `default_folder_id(user_id)`.
    pub is_default: bool,
    /// Manual order (LWW integer), NULL = unpositioned.
    pub position: Option<i32>,
    /// Tombstone marker: tombstoned folders resolve as missing.
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
