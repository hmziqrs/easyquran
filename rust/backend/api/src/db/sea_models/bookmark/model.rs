use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bookmarks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: i32,
    /// NULL = root / uncategorized.
    pub folder_id: Option<String>,
    pub surah: i32,
    pub ayah: i32,
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
    #[sea_orm(
        belongs_to = "super::super::bookmark_folder::Entity",
        from = "Column::FolderId",
        to = "super::super::bookmark_folder::Column::Id"
    )]
    BookmarkFolder,
}

impl Related<super::super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::super::bookmark_folder::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BookmarkFolder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
