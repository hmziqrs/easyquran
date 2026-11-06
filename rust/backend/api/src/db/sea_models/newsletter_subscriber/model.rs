use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use ruxlog_types::enums::SubscriberStatus;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "newsletter_subscribers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub email: String,
    pub status: SubscriberStatus,
    /// HMAC of the confirm/unsubscribe token. Never the plaintext (A29).
    pub token: String,
    /// Epoch seconds: the sweeper (A32) may delete the row after this (A29).
    pub expires_at: i64,
    pub retention_class: String,

    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
