use serde::{Deserialize, Serialize};

// Stored as TEXT in the DB: each `string_value` is a stable contract — renaming
// breaks existing rows. Do not rename once shipped.
#[cfg_attr(
    feature = "backend",
    derive(sea_orm::DeriveActiveEnum, strum::EnumIter)
)]
#[cfg_attr(feature = "backend", sea_orm(rs_type = "String", db_type = "Text"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationKind {
    #[serde(rename = "NewComment")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "new_comment"))]
    NewComment,

    #[serde(rename = "NewFollower")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "new_follower"))]
    NewFollower,

    #[serde(rename = "PaymentReceived")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "payment_received"))]
    PaymentReceived,

    #[serde(rename = "PostPublished")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "post_published"))]
    PostPublished,

    #[serde(rename = "Mention")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "mention"))]
    Mention,

    #[serde(rename = "System")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "system"))]
    System,
}
