use sea_orm::{entity::prelude::*, QueryOrder, QuerySelect, Set};

use crate::error::DbResult;

use super::model::{ActiveModel, ReportReason, ReportTargetKind};
use super::*;

/// Retention class for incident reports (the A32 sweeper reads it).
pub const RETENTION_REPORT_12M: &str = "report_12m";
const RETENTION_SECS: i64 = 365 * 24 * 60 * 60;

pub struct NewReport {
    pub reporter_user_id: Option<i32>,
    pub target_kind: ReportTargetKind,
    pub target_id: String,
    pub reason: ReportReason,
    pub details: Option<String>,
    pub contact_email: Option<String>,
    pub snapshot: Option<serde_json::Value>,
}

impl Entity {
    /// Insert the report inside the caller's transaction, so the row and its
    /// audit row commit or roll back together.
    pub async fn insert_report<C: ConnectionTrait>(conn: &C, new: NewReport) -> DbResult<Model> {
        let now = chrono::Utc::now();
        let row = ActiveModel {
            reporter_user_id: Set(new.reporter_user_id),
            target_kind: Set(new.target_kind.as_str().to_string()),
            target_id: Set(new.target_id),
            reason: Set(new.reason.as_str().to_string()),
            priority: Set(new.reason.priority().to_string()),
            details: Set(new.details),
            contact_email: Set(new.contact_email),
            snapshot: Set(new.snapshot),
            status: Set("open".to_string()),
            created_at: Set(now.fixed_offset()),
            expires_at: Set(now.timestamp() + RETENTION_SECS),
            retention_class: Set(RETENTION_REPORT_12M.to_string()),
            ..Default::default()
        };
        Ok(row.insert(conn).await?)
    }

    /// Open reports at one priority (`p0` or `p1`). Used by the daily digest (A26).
    pub async fn count_open<C: ConnectionTrait>(conn: &C, priority: &str) -> DbResult<u64> {
        Ok(Self::find()
            .filter(Column::Status.eq("open"))
            .filter(Column::Priority.eq(priority))
            .count(conn)
            .await?)
    }

    /// Reports for the operator review list, newest first. Filters apply when set.
    pub async fn list_for_review<C: ConnectionTrait>(
        conn: &C,
        status: Option<&str>,
        priority: Option<&str>,
        limit: u64,
    ) -> DbResult<Vec<Model>> {
        let mut query = Self::find()
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id);
        if let Some(status) = status {
            query = query.filter(Column::Status.eq(status));
        }
        if let Some(priority) = priority {
            query = query.filter(Column::Priority.eq(priority));
        }
        Ok(query.limit(limit).all(conn).await?)
    }

    /// Sets the review status of one report. None when the report does not exist.
    pub async fn set_status<C: ConnectionTrait>(
        conn: &C,
        id: i64,
        status: &str,
    ) -> DbResult<Option<Model>> {
        let Some(existing) = Self::find_by_id(id).one(conn).await? else {
            return Ok(None);
        };
        let mut active: ActiveModel = existing.into();
        active.status = Set(status.to_string());
        Ok(Some(active.update(conn).await?))
    }
}

/// Review statuses an operator may set. `open` is the intake state; the rest close review.
pub const REPORT_STATUSES: &[&str] = &["open", "reviewing", "actioned", "dismissed"];

pub fn is_report_status(raw: &str) -> bool {
    REPORT_STATUSES.contains(&raw)
}
