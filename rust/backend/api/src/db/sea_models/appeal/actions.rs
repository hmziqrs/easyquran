use sea_orm::{entity::prelude::*, Set};

use crate::db::sea_models::enforcement_notice;
use crate::error::DbResult;

use super::model::ActiveModel;
use super::*;

/// Decision clock: a decision is due this many seconds after the appeal is submitted.
pub const DECISION_CLOCK_SECS: i64 = 7 * 24 * 60 * 60;
/// Appeals are evidence kept with the notice they answer (12 months).
pub const APPEAL_RETENTION_SECS: i64 = 365 * 24 * 60 * 60;
pub const RETENTION_CLASS: &str = "enforcement_12m";
pub const STATUS_OPEN: &str = "open";
pub const MAX_STATEMENT_CHARS: usize = 2_000;

impl Entity {
    /// Records the appeal for a notice. One per notice: returns `false` when one
    /// already exists, and writes nothing. The caller answers the same either way.
    pub async fn submit<C: ConnectionTrait>(
        conn: &C,
        notice: &enforcement_notice::Model,
        statement: &str,
        now: i64,
    ) -> DbResult<bool> {
        let existing = Self::find()
            .filter(Column::NoticeId.eq(notice.id))
            .one(conn)
            .await?;
        if existing.is_some() {
            return Ok(false);
        }
        ActiveModel {
            notice_id: Set(notice.id),
            user_id: Set(notice.user_id),
            statement: Set(statement.to_string()),
            status: Set(STATUS_OPEN.to_string()),
            submitted_at: Set(now),
            due_at: Set(now + DECISION_CLOCK_SECS),
            decided_at: Set(None),
            created_at: Set(now),
            expires_at: Set(now + APPEAL_RETENTION_SECS),
            retention_class: Set(RETENTION_CLASS.to_string()),
            ..Default::default()
        }
        .insert(conn)
        .await?;
        Ok(true)
    }

    /// Open appeals whose decision clock has run out.
    pub async fn overdue_count<C: ConnectionTrait>(conn: &C, now: i64) -> DbResult<u64> {
        Ok(Self::find()
            .filter(Column::Status.eq(STATUS_OPEN))
            .filter(Column::DueAt.lt(now))
            .count(conn)
            .await?)
    }

    /// Appeals still awaiting a decision, overdue or not (A26 digest).
    pub async fn open_count<C: ConnectionTrait>(conn: &C) -> DbResult<u64> {
        Ok(Self::find()
            .filter(Column::Status.eq(STATUS_OPEN))
            .count(conn)
            .await?)
    }
}
