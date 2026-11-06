//! Legal holds (A25). Placing or renewing a hold writes one audit row in the same
//! transaction. The A32 sweeper reads `legal_holds` and `legal_hold_subjects` and
//! skips any row a live hold covers.

use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder, QuerySelect, TransactionTrait};
use serde_json::json;

use crate::db::sea_models::legal_hold::{self, model::Model, subject, NewHold};
use crate::error::{DbResult, ErrorCode, ErrorResponse};

use super::audit::{self, AuditAction, AuditEvent};

pub async fn place(db: &DatabaseConnection, actor: Option<i32>, new: NewHold) -> DbResult<Model> {
    let txn = db.begin().await?;
    let hold = legal_hold::actions::place(&txn, new, Utc::now()).await?;
    audit::record(
        &txn,
        AuditEvent {
            actor_id: actor,
            action: AuditAction::LegalHoldPlace,
            resource_id: hold.id.to_string(),
            metadata: json!({ "retention_class": hold.retention_class }),
        },
    )
    .await?;
    txn.commit().await?;
    Ok(hold)
}

pub async fn renew(db: &DatabaseConnection, actor: Option<i32>, hold_id: i64) -> DbResult<Model> {
    let txn = db.begin().await?;
    let hold = legal_hold::actions::renew(&txn, hold_id, Utc::now())
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::OperationNotAllowed))?;
    audit::record(
        &txn,
        AuditEvent {
            actor_id: actor,
            action: AuditAction::LegalHoldRenew,
            resource_id: hold.id.to_string(),
            metadata: json!({ "renewals": hold.renewals, "preserve_until": hold.preserve_until }),
        },
    )
    .await?;
    txn.commit().await?;
    Ok(hold)
}

/// A hold with its subjects, for the operator list and the CLI.
pub struct HoldView {
    pub hold: Model,
    pub subjects: Vec<subject::Model>,
}

/// Newest holds first, at most `limit`. Read only: no audit row.
pub async fn list(db: &DatabaseConnection, limit: u64) -> DbResult<Vec<HoldView>> {
    let holds = legal_hold::model::Entity::find()
        .order_by_desc(legal_hold::model::Column::Id)
        .limit(limit)
        .all(db)
        .await?;
    let mut out = Vec::with_capacity(holds.len());
    for hold in holds {
        let subjects = legal_hold::actions::subjects_for(db, hold.id).await?;
        out.push(HoldView { hold, subjects });
    }
    Ok(out)
}
