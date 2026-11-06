use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sea_orm::{EntityTrait, TransactionTrait};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{info, instrument};

use crate::db::sea_models::appeal::actions::{is_decision, DecideOutcome};
use crate::db::sea_models::appeal::{self, Model as AppealModel};
use crate::db::sea_models::report::{self, is_report_status, Model as ReportModel};
use crate::error::{ErrorCode, ErrorResponse};
use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::services::auth::AuthSession;
use crate::AppState;

const DEFAULT_LIMIT: u64 = 50;
const MAX_LIMIT: u64 = 200;
const APPEAL_STATUS_OPEN: &str = "open";
const APPEAL_FILTERS: &[&str] = &[APPEAL_STATUS_OPEN, "upheld", "overturned"];

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AppealQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetStatusPayload {
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPayload {
    pub decision: String,
}

fn actor_id(auth: &AuthSession) -> Result<i32, ErrorResponse> {
    auth.user
        .as_ref()
        .map(|user| user.id)
        .ok_or_else(|| ErrorResponse::new(ErrorCode::Unauthorized))
}

fn invalid(message: &str) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidInput).with_message(message)
}

/// Requested page size: default 50, never below 1 or above 200.
pub fn page_limit(requested: Option<u64>) -> u64 {
    requested.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

fn report_json(row: &ReportModel) -> Value {
    json!({
        "id": row.id,
        "targetKind": row.target_kind,
        "targetId": row.target_id,
        "reason": row.reason,
        "priority": row.priority,
        "status": row.status,
        "details": row.details,
        "snapshot": row.snapshot,
        "createdAt": row.created_at.to_rfc3339(),
        "expiresAt": row.expires_at,
    })
}

fn appeal_json(row: &AppealModel) -> Value {
    json!({
        "id": row.id,
        "noticeId": row.notice_id,
        "statement": row.statement,
        "status": row.status,
        "submittedAt": row.submitted_at,
        "dueAt": row.due_at,
        "decidedAt": row.decided_at,
    })
}

/// `GET /admin/reports/v1?status=&priority=&limit=`. Newest first.
#[instrument(skip(state, auth))]
pub async fn list_reports(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(query): Query<ReportQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    actor_id(&auth)?;
    let status = query.status.filter(|s| !s.is_empty());
    if let Some(status) = status.as_deref() {
        if !is_report_status(status) {
            return Err(invalid("status is not a review status"));
        }
    }
    let priority = query.priority.filter(|p| !p.is_empty());
    if let Some(priority) = priority.as_deref() {
        if priority != "p0" && priority != "p1" {
            return Err(invalid("priority must be p0 or p1"));
        }
    }
    let rows = report::Entity::list_for_review(
        &state.sea_db,
        status.as_deref(),
        priority.as_deref(),
        page_limit(query.limit),
    )
    .await?;
    let body: Vec<Value> = rows.iter().map(report_json).collect();
    Ok((StatusCode::OK, Json(json!(body))))
}

/// `POST /admin/reports/v1/{id}/status`. Audited as `report.status_set` in the same transaction.
#[instrument(skip(state, auth, payload))]
pub async fn set_report_status(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(report_id): Path<i64>,
    Json(payload): Json<SetStatusPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor_id(&auth)?;
    if !is_report_status(&payload.status) {
        return Err(invalid("status is not a review status"));
    }
    let txn = state.sea_db.begin().await?;
    let before = report::Entity::find_by_id(report_id)
        .one(&txn)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    let after = report::Entity::set_status(&txn, report_id, &payload.status)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    audit::record(
        &txn,
        AuditEvent {
            actor_id: Some(admin),
            action: AuditAction::ReportStatusSet,
            resource_id: report_id.to_string(),
            metadata: json!({ "from": before.status, "to": after.status }),
        },
    )
    .await?;
    txn.commit().await?;
    info!(report_id, status = %after.status, "report status set");
    Ok((
        StatusCode::OK,
        Json(json!({ "id": report_id, "status": after.status })),
    ))
}

/// `GET /admin/appeals/v1?status=&limit=`. Defaults to open appeals, oldest deadline first.
#[instrument(skip(state, auth))]
pub async fn list_appeals(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(query): Query<AppealQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    actor_id(&auth)?;
    let status = query
        .status
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| APPEAL_STATUS_OPEN.to_string());
    if !APPEAL_FILTERS.contains(&status.as_str()) {
        return Err(invalid("status must be open, upheld or overturned"));
    }
    let rows = appeal::Entity::list_for_review(
        &state.sea_db,
        Some(status.as_str()),
        page_limit(query.limit),
    )
    .await?;
    let body: Vec<Value> = rows.iter().map(appeal_json).collect();
    Ok((StatusCode::OK, Json(json!(body))))
}

/// `POST /admin/appeals/v1/{id}/decision`. Records `upheld` or `overturned` on an open appeal.
/// The row leaves `open`, so the decision clock (A35) stops. Audited as `appeal.decision`.
#[instrument(skip(state, auth, payload))]
pub async fn decide_appeal(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(appeal_id): Path<i64>,
    Json(payload): Json<DecisionPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor_id(&auth)?;
    if !is_decision(&payload.decision) {
        return Err(invalid("decision must be upheld or overturned"));
    }
    let now = chrono::Utc::now().timestamp();
    let txn = state.sea_db.begin().await?;
    let decided = match appeal::Entity::decide(&txn, appeal_id, &payload.decision, now).await? {
        DecideOutcome::NotFound => {
            return Err(ErrorResponse::new(ErrorCode::RecordNotFound));
        }
        DecideOutcome::AlreadyDecided => {
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("appeal already has a decision"));
        }
        DecideOutcome::Decided(row) => row,
    };
    audit::record(
        &txn,
        AuditEvent {
            actor_id: Some(admin),
            action: AuditAction::AppealDecision,
            resource_id: appeal_id.to_string(),
            metadata: json!({ "decision": decided.status, "notice_id": decided.notice_id }),
        },
    )
    .await?;
    txn.commit().await?;
    info!(appeal_id, "appeal decision recorded");
    Ok((StatusCode::OK, Json(appeal_json(&decided))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sea_models::appeal::actions::DECISION_UPHELD;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
    };

    #[test]
    fn page_limit_defaults_and_stays_in_bounds() {
        assert_eq!(page_limit(None), DEFAULT_LIMIT);
        assert_eq!(page_limit(Some(0)), 1);
        assert_eq!(page_limit(Some(10_000)), MAX_LIMIT);
    }

    #[test]
    fn review_statuses_and_decisions_are_closed_sets() {
        assert!(is_report_status("actioned"));
        assert!(!is_report_status("deleted"));
        assert!(is_decision(DECISION_UPHELD));
        assert!(!is_decision("pending"));
    }

    async fn test_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn insert_open_appeal(db: &DatabaseConnection, due_at: i64) -> i64 {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "INSERT INTO appeals (notice_id, user_id, statement, status, submitted_at, due_at, \
                 created_at, expires_at, retention_class) \
                 VALUES (1, 5, 'please review', 'open', {due_at}, {due_at}, {due_at}, {due_at}, 'enforcement_12m')"
            ),
        ))
        .await
        .unwrap();
        db.query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT MAX(id) FROM appeals".to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<i64>(0)
        .unwrap()
    }

    #[tokio::test]
    async fn recording_a_decision_stops_the_appeal_clock() {
        let db = test_db().await;
        let now = 1_900_000_000;
        let id = insert_open_appeal(&db, now - 10).await;
        assert_eq!(appeal::Entity::overdue_count(&db, now).await.unwrap(), 1);

        let outcome = appeal::Entity::decide(&db, id, DECISION_UPHELD, now)
            .await
            .unwrap();

        assert!(matches!(outcome, DecideOutcome::Decided(_)));
        assert_eq!(appeal::Entity::overdue_count(&db, now).await.unwrap(), 0);
        assert_eq!(appeal::Entity::open_count(&db).await.unwrap(), 0);
        let again = appeal::Entity::decide(&db, id, "overturned", now)
            .await
            .unwrap();
        assert!(matches!(again, DecideOutcome::AlreadyDecided));
    }

    #[tokio::test]
    async fn deciding_a_missing_appeal_reports_not_found() {
        let db = test_db().await;
        let outcome = appeal::Entity::decide(&db, 999, DECISION_UPHELD, 1)
            .await
            .unwrap();
        assert!(matches!(outcome, DecideOutcome::NotFound));
    }
}
