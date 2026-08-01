use chrono::{DateTime, Duration, FixedOffset, Utc};
use serde::{Deserialize, Serialize};

use crate::traits::BanStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSessionState<UserId> {
    pub user_id: UserId,

    pub authenticated_at: DateTime<FixedOffset>,

    pub email_verified: bool,

    pub ban_checked_at: Option<DateTime<FixedOffset>>,

    pub is_banned: bool,

    /// Security: recomputed per request so a password change or user deletion+recreate invalidates prior sessions — do not drop.
    pub session_auth_hash: Vec<u8>,

    pub device: Option<String>,

    pub ip_address: Option<String>,

    pub last_seen: DateTime<FixedOffset>,
}

impl<UserId: Clone> AuthSessionState<UserId> {
    pub fn new(user_id: UserId, email_verified: bool) -> Self {
        let now = Utc::now().fixed_offset();
        Self {
            user_id,
            authenticated_at: now,
            email_verified,
            ban_checked_at: None,
            is_banned: false,
            session_auth_hash: Vec::new(),
            device: None,
            ip_address: None,
            last_seen: now,
        }
    }

    pub fn with_metadata(mut self, device: Option<String>, ip_address: Option<String>) -> Self {
        self.device = device;
        self.ip_address = ip_address;
        self
    }

    pub fn update_ban_status(&mut self, status: &BanStatus) {
        self.ban_checked_at = Some(Utc::now().fixed_offset());
        self.is_banned = status.is_banned();
    }

    pub fn touch(&mut self) {
        self.last_seen = Utc::now().fixed_offset();
    }

    pub fn ban_cache_stale(&self, max_age: Duration) -> bool {
        self.ban_checked_at
            .map(|t| Utc::now().fixed_offset() - t > max_age)
            .unwrap_or(true)
    }

    pub fn refresh_verification(&mut self, email_verified: bool) {
        self.email_verified = email_verified;
    }
}
