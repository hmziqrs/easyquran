use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use tracing::{info, instrument};

use crate::error::{ErrorCode, ErrorResponse};
use crate::services::auth::AuthSession;
use crate::services::legal_hold::{self, HoldView};
use crate::AppState;

use super::dto::{validate_place, PlaceHoldPayload};

const LIST_LIMIT: u64 = 200;

fn actor_id(auth: &AuthSession) -> Result<i32, ErrorResponse> {
    auth.user
        .as_ref()
        .map(|user| user.id)
        .ok_or_else(|| ErrorResponse::new(ErrorCode::Unauthorized))
}

fn hold_json(view: &HoldView) -> Value {
    let hold = &view.hold;
    let subjects: Vec<Value> = view
        .subjects
        .iter()
        .map(|s| {
            json!({
                "kind": s.subject_kind,
                "id": s.subject_id,
                "userId": s.subject_user_id,
            })
        })
        .collect();
    json!({
        "id": hold.id,
        "retentionClass": hold.retention_class,
        "authority": hold.authority,
        "reason": hold.reason,
        "owner": hold.owner,
        "caseRef": hold.case_ref,
        "autoRelease": hold.auto_release,
        "placedAt": hold.placed_at,
        "expiresAt": hold.expires_at,
        "preserveUntil": hold.preserve_until,
        "renewals": hold.renewals,
        "releasedAt": hold.released_at,
        "subjects": subjects,
    })
}

/// `GET /admin/legal-holds/v1`. Newest first, at most 200.
#[instrument(skip(state, auth))]
pub async fn list_holds(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    actor_id(&auth)?;
    let views = legal_hold::list(&state.sea_db, LIST_LIMIT).await?;
    let rows: Vec<Value> = views.iter().map(hold_json).collect();
    Ok((StatusCode::OK, Json(json!(rows))))
}

/// `POST /admin/legal-holds/v1`. Places a hold and its subjects; audited as `legal_hold.place`.
#[instrument(skip(state, auth, payload))]
pub async fn place_hold(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(payload): Json<PlaceHoldPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor_id(&auth)?;
    let new = validate_place(payload)
        .map_err(|msg| ErrorResponse::new(ErrorCode::InvalidInput).with_message(msg))?;
    let hold = legal_hold::place(&state.sea_db, Some(admin), new).await?;
    info!(hold_id = hold.id, "legal hold placed");
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": hold.id,
            "retentionClass": hold.retention_class,
            "expiresAt": hold.expires_at,
        })),
    ))
}

/// `POST /admin/legal-holds/v1/{id}/renew`. Renewable holds only; audited as `legal_hold.renew`.
#[instrument(skip(state, auth))]
pub async fn renew_hold(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(hold_id): Path<i64>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let admin = actor_id(&auth)?;
    let hold = legal_hold::renew(&state.sea_db, Some(admin), hold_id).await?;
    info!(hold_id, "legal hold renewed");
    Ok((
        StatusCode::OK,
        Json(json!({
            "id": hold.id,
            "renewals": hold.renewals,
            "expiresAt": hold.expires_at,
        })),
    ))
}
