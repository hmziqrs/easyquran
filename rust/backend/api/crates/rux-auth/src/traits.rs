use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

use crate::error::AuthError;

#[derive(Debug, Clone)]
pub enum BanStatus {
    NotBanned,
    Banned {
        reason: Option<String>,
        expires_at: Option<DateTime<FixedOffset>>,
        banned_by: Option<i64>,
    },
}

impl BanStatus {
    pub fn is_banned(&self) -> bool {
        match self {
            Self::NotBanned => false,
            Self::Banned { expires_at, .. } => {
                if let Some(expires) = expires_at {
                    chrono::Utc::now().fixed_offset() < *expires
                } else {
                    true // Permanent ban
                }
            }
        }
    }
}

pub trait AuthUser: Clone + Debug + Send + Sync + 'static {
    type Id: Clone + Debug + Send + Sync + Serialize + DeserializeOwned + PartialEq + 'static;

    fn id(&self) -> Self::Id;

    /// Session-invalidation hash: return password-hash bytes (password users) or
    /// email bytes (OAuth users); changing it invalidates existing sessions.
    fn session_auth_hash(&self) -> &[u8];

    fn email_verified(&self) -> bool;

    fn totp_enabled(&self) -> bool;

    /// Higher = more permissions (e.g. User=0, Admin=3); drives hierarchical authz.
    fn role_level(&self) -> i32;
}

#[async_trait]
pub trait AuthBackend: Clone + Send + Sync + 'static {
    type User: AuthUser;

    async fn get_user(
        &self,
        id: &<Self::User as AuthUser>::Id,
    ) -> Result<Option<Self::User>, AuthError>;

    async fn check_ban(
        &self,
        user_id: &<Self::User as AuthUser>::Id,
    ) -> Result<BanStatus, AuthError>;

    async fn verify_password(
        &self,
        user_id: &<Self::User as AuthUser>::Id,
        password: &str,
    ) -> Result<bool, AuthError>;

    async fn on_login(&self, _user: &Self::User) -> Result<(), AuthError> {
        Ok(())
    }

    async fn on_logout(&self, _user_id: &<Self::User as AuthUser>::Id) -> Result<(), AuthError> {
        Ok(())
    }
}
