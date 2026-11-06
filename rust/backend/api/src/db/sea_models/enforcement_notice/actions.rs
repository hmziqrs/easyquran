use sea_orm::{entity::prelude::*, QueryOrder, Set};

use crate::error::DbResult;

use super::model::ActiveModel;
use super::*;

/// Retention class for ban decisions and their notices (the A32 sweeper reads it).
pub const RETENTION_ENFORCEMENT_12M: &str = "enforcement_12m";

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
            ..Default::default()
        };
        Ok(row.insert(conn).await?)
    }

    pub async fn list_for_user<C: ConnectionTrait>(conn: &C, user_id: i32) -> DbResult<Vec<Model>> {
        Ok(Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::Id)
            .all(conn)
            .await?)
    }
}
