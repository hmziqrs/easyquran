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
    // No FK in the schema (m000032): ban rows outlive the account (A19b).
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::UserId",
        to = "super::super::user::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::BannedBy",
        to = "super::super::user::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    BannedByUser,
    #[sea_orm(
        belongs_to = "super::super::user::Entity",
        from = "Column::RevokedBy",
        to = "super::super::user::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    RevokedByUser,
}

impl Related<super::super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
