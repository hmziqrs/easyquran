use sea_orm::{entity::prelude::*, QueryOrder, Set};

use crate::error::DbResult;

use super::model::ActiveModel;
use super::*;

/// Retention class for ban decisions and their notices (the A32 sweeper reads it).
pub const RETENTION_ENFORCEMENT_12M: &str = "enforcement_12m";

pub const CHANNEL_EMAIL: &str = "email";

/// Outcome of one delivery attempt. Stored as `delivery_state` (CHECK-constrained).
/// `Sent` means the provider accepted the message for delivery or queued it. A
/// provider `Ok` that accepted nothing is `SendFailed`, never `Sent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryState {
    Sent,
    Suppressed,
    NoContactDetails,
    SendFailed,
}

impl DeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Suppressed => "suppressed",
            Self::NoContactDetails => "no_contact_details",
            Self::SendFailed => "send_failed",
        }
    }
}

pub struct NewNotice {
    pub user_id: i32,
    pub ban_id: i32,
    pub reason_code: String,
    pub reason_text: Option<String>,
    pub corroborator: String,
    pub decided_by: i32,
    pub decided_at: i64,
    pub effective_at: i64,
    pub expires_at: i64,
}

impl Entity {
    /// Insert the case record inside the caller's transaction. `delivery_state`
    /// stays NULL here; A21 sets it once the mail provider answers.
    pub async fn insert_notice<C: ConnectionTrait>(conn: &C, new: NewNotice) -> DbResult<Model> {
        let now = chrono::Utc::now().timestamp();
        let row = ActiveModel {
            user_id: Set(new.user_id),
            ban_id: Set(new.ban_id),
            reason_code: Set(new.reason_code),
            reason_text: Set(new.reason_text),
            corroborator: Set(new.corroborator),
            decided_by: Set(new.decided_by),
            decided_at: Set(new.decided_at),
            effective_at: Set(new.effective_at),
            expires_at: Set(new.expires_at),
            delivery_state: Set(None),
            sent_at: Set(None),
            appeal_token_hash: Set(None),
            created_at: Set(now),
            retention_class: Set(RETENTION_ENFORCEMENT_12M.to_string()),
            notice_channel: Set(CHANNEL_EMAIL.to_string()),
            ..Default::default()
        };
        Ok(row.insert(conn).await?)
    }

    /// The most recent notice for a ban decision (the one a resend targets).
    pub async fn find_latest_for_ban<C: ConnectionTrait>(
        conn: &C,
        ban_id: i32,
    ) -> DbResult<Option<Model>> {
        Ok(Self::find()
            .filter(Column::BanId.eq(ban_id))
            .order_by_desc(Column::Id)
            .one(conn)
            .await?)
    }

    /// The notice an appeal token points at. Callers pass the keyed HMAC of the
    /// presented token, never the token.
    pub async fn find_by_appeal_token_hash<C: ConnectionTrait>(
        conn: &C,
        token_hash: &str,
    ) -> DbResult<Option<Model>> {
        Ok(Self::find()
            .filter(Column::AppealTokenHash.eq(token_hash))
            .one(conn)
            .await?)
    }

    /// Replaces the notice's appeal token hash. A new token retires the old link.
    pub async fn set_appeal_token_hash<C: ConnectionTrait>(
        conn: &C,
        notice_id: i64,
        token_hash: &str,
    ) -> DbResult<()> {
        ActiveModel {
            id: Set(notice_id),
            appeal_token_hash: Set(Some(token_hash.to_string())),
            ..Default::default()
        }
        .update(conn)
        .await?;
        Ok(())
    }

    /// Record the provider-confirmed outcome. `sent_at` is set only for `sent`.
    pub async fn record_delivery<C: ConnectionTrait>(
        conn: &C,
        notice_id: i64,
        state: DeliveryState,
        sent_at: Option<i64>,
    ) -> DbResult<()> {
        ActiveModel {
            id: Set(notice_id),
            delivery_state: Set(Some(state.as_str().to_string())),
            sent_at: Set(sent_at),
            ..Default::default()
        }
        .update(conn)
        .await?;
        Ok(())
    }

    pub async fn list_for_user<C: ConnectionTrait>(conn: &C, user_id: i32) -> DbResult<Vec<Model>> {
        Ok(Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::Id)
            .all(conn)
            .await?)
    }
}
