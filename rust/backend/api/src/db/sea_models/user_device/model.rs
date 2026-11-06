use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_devices")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub display_name: Option<String>,
    pub platform: String,
    pub client: String,
    pub form_factor: String,
    pub browser: Option<String>,
    pub os_major: Option<i32>,
    pub app_version: Option<String>,
    pub name_stamp: Option<String>,
    pub merged_into: Option<String>,
    /// JWK text of the device key, public half only. Null for `key_store = none`.
    pub public_key: Option<String>,
    /// RFC 7638 thumbprint of `public_key`. The match key for recognition.
    pub key_thumbprint: Option<String>,
    /// Self-reported store label (`browser`, `secure_hardware`, `os_keystore`, `file`, `none`).
    pub key_store: String,
    /// Per-account HMAC continuity hint. Never logged, never a limiter or ban key.
    pub continuity_hint: Option<String>,
    pub hint_conflict: bool,
    /// Thumbprint of the key this row held before a rung-3 merge.
    pub merged_key_thumbprint: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub last_seen_at: DateTimeWithTimeZone,
    pub retired_at: Option<DateTimeWithTimeZone>,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
