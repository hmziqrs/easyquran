use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::error::DbResult;

use super::model::ActiveModel;
use super::*;

/// Retention class for captured case evidence (A32 sweeper reads it): 12 months.
pub const RETENTION_CASE_12M: &str = "enforcement_12m";
const RETENTION_SECS: i64 = 365 * 24 * 60 * 60;

/// One snapshot object to append to a case.
pub struct NewEvidence {
    pub case_ref: String,
    pub kind: &'static str,
    pub subject_id: String,
    pub snapshot: serde_json::Value,
}

impl Entity {
    /// Append the rows inside the caller's transaction, so the evidence and the
    /// action it precedes commit or roll back together.
    pub async fn append<C: ConnectionTrait>(
        conn: &C,
        rows: Vec<NewEvidence>,
        now: DateTime<Utc>,
    ) -> DbResult<usize> {
        let count = rows.len();
        let captured_at = now.timestamp();
        for row in rows {
            ActiveModel {
                case_ref: Set(row.case_ref),
                kind: Set(row.kind.to_string()),
                subject_id: Set(row.subject_id),
                snapshot: Set(row.snapshot),
                captured_at: Set(captured_at),
                expires_at: Set(captured_at + RETENTION_SECS),
                retention_class: Set(RETENTION_CASE_12M.to_string()),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }
        Ok(count)
    }

    pub async fn list_for_case<C: ConnectionTrait>(
        conn: &C,
        case_ref: &str,
    ) -> DbResult<Vec<Model>> {
        Ok(Entity::find()
            .filter(Column::CaseRef.eq(case_ref))
            .order_by_asc(Column::Id)
            .all(conn)
            .await?)
    }
}
