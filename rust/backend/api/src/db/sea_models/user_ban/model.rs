use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_bans")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub reason: Option<String>,
    pub banned_by: Option<i32>,
    pub expires_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub revoked_by: Option<i32>,
}

impl Model {
    pub fn is_active(&self) -> bool {
        if self.revoked_at.is_some() {
            return false;
        }

        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now().fixed_offset() < expires_at
        } else {
            true
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::UserId",
        to = "super::super::user::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::BannedBy",
        to = "super::super::user::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    BannedByUser,
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::RevokedBy",
        to = "super::super::user::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    RevokedByUser,
}

impl Related<super::super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
