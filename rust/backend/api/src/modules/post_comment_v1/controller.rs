use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_macros::debug_handler;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde_json::json;
use tracing::{error, info, instrument, warn};

use crate::{
    db::sea_models::{comment_flag, enforcement_notice, post, post_comment, user},
    error::{DbResult, ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    modules::{admin_user_bans_v1::notice as delivery, appeal_v1},
    services::{
        abuse_limiter,
        audit::{self, AuditAction, AuditEvent},
        auth::AuthSession,
        incident_capture, registration_admission,
    },
    AppState,
};

/// Flag rate limit per account (A27): 20 flags an hour and 50 a day. The gate blocks
/// on reaching the threshold, so the thresholds are one above the allowance. Every
/// attempt counts, landed or not. Process-local, like the other limiters.
pub fn flag_limit_config() -> abuse_limiter::AbuseLimiterConfig {
    abuse_limiter::AbuseLimiterConfig {
        temp_block_attempts: 21,
        temp_block_range: 60 * 60,
        temp_block_duration: 60 * 60,
        block_retry_limit: 51,
        block_range: 24 * 60 * 60,
        block_duration: 24 * 60 * 60,
    }
}

/// Open flags at which the author can no longer edit a comment (A27).
const EDIT_LOCK_FLAGS: i32 = 3;
const MAX_COMMENT_REASON_CHARS: usize = 500;
const DEFAULT_COMMENT_REASON: &str = "policy_violation";

// Per-account comment throttle: per-IP limiting alone can't stop one account behind rotating IPs from flooding comments.
const COMMENT_ABUSE_CONFIG: abuse_limiter::AbuseLimiterConfig = abuse_limiter::AbuseLimiterConfig {
    temp_block_attempts: 20,
    temp_block_range: 60,
    temp_block_duration: 600,
    block_retry_limit: 100,
    block_range: 3600,
    block_duration: 3600,
};

use super::validator::{
    V1AdminCommentActionPayload, V1AdminCommentFlagListQuery, V1AdminPostCommentListQuery,
    V1ConfirmEvidenceQuery, V1CreatePostCommentPayload, V1FlagCommentPayload,
    V1UpdatePostCommentPayload,
};

/// A hidden comment, or one at the flag lock, is frozen for its author (A27).
fn ensure_editable(comment: &post_comment::Model) -> Result<(), ErrorResponse> {
    if comment.hidden {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Hidden comments cannot be edited."));
    }
    if comment.flags_count >= EDIT_LOCK_FLAGS {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Flagged comments cannot be edited."));
    }
    Ok(())
}

/// Reason for a comment notice. A closed code plus optional free text, refused before
/// any write when either is out of range.
fn comment_notice_reason(
    payload: Option<V1AdminCommentActionPayload>,
) -> Result<(String, Option<String>), ErrorResponse> {
    let payload = payload.unwrap_or_default();
    let code = payload
        .reason_code
        .unwrap_or_else(|| DEFAULT_COMMENT_REASON.to_string());
    if !enforcement_notice::COMMENT_REASON_CODES.contains(&code.as_str()) {
        return Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("reasonCode is not a known reason"));
    }
    let text = payload
        .reason_text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    if text
        .as_ref()
        .is_some_and(|text| text.chars().count() > MAX_COMMENT_REASON_CHARS)
    {
        return Err(
            ErrorResponse::new(ErrorCode::InvalidInput).with_message("reasonText is too long")
        );
    }
    Ok((code, text))
}

/// Case record and audit row for a comment hide or removal (A27). Runs in the caller's
/// transaction, so the action and its notice commit or roll back together.
async fn record_comment_notice<C: ConnectionTrait>(
    conn: &C,
    admin_id: i32,
    comment: &post_comment::Model,
    action: &str,
    reason: (String, Option<String>),
) -> DbResult<enforcement_notice::Model> {
    let notice = enforcement_notice::Entity::insert_comment_notice(
        conn,
        enforcement_notice::NewCommentNotice {
            user_id: comment.user_id,
            comment_id: comment.id,
            reason_code: reason.0.clone(),
            reason_text: reason.1,
            decided_by: admin_id,
            decided_at: chrono::Utc::now().timestamp(),
        },
    )
    .await?;
    audit::record(
        conn,
        AuditEvent {
            actor_id: Some(admin_id),
            action: AuditAction::CommentNotice,
            resource_id: comment.id.to_string(),
            metadata: json!({
                "noticeId": notice.id,
                "userId": comment.user_id,
                "action": action,
                "reasonCode": reason.0,
            }),
        },
    )
    .await?;
    Ok(notice)
}

/// Sends a comment notice after commit, with a fresh appeal link (A35 rule: a send
/// replaces the stored hash). Returns the recorded delivery state. A failure is logged
/// and the hide or removal stays in place: the row shows the state, not a guess.
async fn deliver_comment_notice(
    state: &AppState,
    notice: &enforcement_notice::Model,
) -> Option<String> {
    let author = user::Entity::find_by_id(notice.user_id)
        .one(&state.sea_db)
        .await
        .inspect_err(|err| warn!(notice_id = notice.id, error = ?err, "author lookup failed"))
        .ok()
        .flatten();
    let contact = author
        .as_ref()
        .map(|author| (author.email.as_str(), author.name.as_str()));
    let appeal_url = if contact.is_some() {
        let token = appeal_v1::controller::new_token();
        let hash = appeal_v1::controller::token_hash(&state.secret_key, &token);
        enforcement_notice::Entity::set_appeal_token_hash(&state.sea_db, notice.id, &hash)
            .await
            .inspect_err(
                |err| warn!(notice_id = notice.id, error = ?err, "appeal token not stored"),
            )
            .ok()
            .map(|()| appeal_v1::controller::appeal_url(&state.settings.site.url, &token))
    } else {
        None
    };
    match delivery::deliver(
        &state.sea_db,
        &state.mailer,
        &state.settings.site.name,
        notice,
        contact,
        appeal_url.as_deref(),
    )
    .await
    {
        Ok(state) => Some(state.as_str().to_string()),
        Err(err) => {
            warn!(notice_id = notice.id, error = ?err, "comment notice delivery record failed");
            None
        }
    }
}

/// Removal guard (A25). A comment with open flags is evidence: removal is refused
/// unless the caller passes `confirm_evidence`. When passed, the case is copied in
/// the same transaction, before the row and its flags cascade away.
async fn guard_open_flags<C: ConnectionTrait>(
    conn: &C,
    comment_id: i32,
    confirm_evidence: bool,
) -> Result<(), ErrorResponse> {
    if comment_flag::Entity::count_for_comment(conn, comment_id).await? == 0 {
        return Ok(());
    }
    if !confirm_evidence {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed).with_message(
            "Comment has open flags. Resend with confirm_evidence=true to remove it and keep the flags as case evidence.",
        ));
    }
    incident_capture::capture_comment(
        conn,
        &incident_capture::removal_case_ref(comment_id),
        comment_id,
        chrono::Utc::now(),
    )
    .await?;
    Ok(())
}

async fn require_post_visibility<C: ConnectionTrait>(
    conn: &C,
    post_id: i32,
    viewer: Option<&user::Model>,
) -> Result<(), ErrorResponse> {
    let post = post::Entity::find_by_id(post_id)
        .one(conn)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    let can_view_hidden = viewer
        // A28: hidden-post comments are visible to the author and Admin, not Moderator.
        .map(|viewer| viewer.id == post.author_id || viewer.is_admin())
        .unwrap_or(false);
    if post.status != post::PostStatus::Published && !can_view_hidden {
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound));
    }
    Ok(())
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id), post_id, comment_id))]
pub async fn create(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1CreatePostCommentPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();

    // Minors (A34): commenting is closed for self-declared under-18 accounts.
    if user.is_minor {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Comments are not available for this account"));
    }

    abuse_limiter::limiter(
        &state.gate_store,
        &format!("comment:create:user:{}", user.id),
        COMMENT_ABUSE_CONFIG,
    )
    .await?;

    let new_comment = payload.0.into_new_post_comment(user.id);
    tracing::Span::current().record("post_id", new_comment.post_id);
    let tx = state.sea_db.begin().await?;
    require_post_visibility(&tx, new_comment.post_id, Some(&user)).await?;
    // New-account limits (A9): one comment and no links in the first 24 hours.
    registration_admission::check_fresh_comment(
        &tx,
        &user,
        &new_comment.content,
        chrono::Utc::now(),
        true,
    )
    .await?;

    match post_comment::Entity::create(&tx, new_comment).await {
        Ok(comment) => {
            tx.commit().await?;
            tracing::Span::current().record("comment_id", comment.id);
            info!(
                user_id = user.id,
                post_id = comment.post_id,
                comment_id = comment.id,
                "Comment created"
            );
            Ok((StatusCode::CREATED, Json(json!(comment))))
        }
        Err(err) => {
            error!(user_id = user.id, "Failed to create comment: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id), comment_id))]
pub async fn update(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
    payload: ValidatedJson<V1UpdatePostCommentPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let current = post_comment::Entity::find_by_id(comment_id)
        .filter(post_comment::Column::UserId.eq(user.id))
        .one(&state.sea_db)
        .await?;
    if let Some(comment) = current.as_ref() {
        ensure_editable(comment)?;
    }
    let update_comment = payload.0.into_update_post_comment();
    if let Some(content) = update_comment.content.as_deref() {
        registration_admission::check_fresh_comment(
            &state.sea_db,
            &user,
            content,
            chrono::Utc::now(),
            false,
        )
        .await?;
    }

    match post_comment::Entity::update(&state.sea_db, comment_id, user.id, update_comment).await {
        Ok(Some(comment)) => {
            info!(user_id = user.id, comment_id, "Comment updated");
            Ok((StatusCode::OK, Json(json!(comment))))
        }
        Ok(None) => {
            warn!(
                user_id = user.id,
                comment_id, "Comment not found for update"
            );
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("Comment does not exist"))
        }
        Err(err) => {
            error!(
                user_id = user.id,
                comment_id, "Failed to update comment: {}", err
            );
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id = auth.user.as_ref().map(|u| u.id), comment_id))]
pub async fn delete(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
    Query(evidence): Query<V1ConfirmEvidenceQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let deleted = async {
        let txn = state.sea_db.begin().await?;
        // Only the author's own comment reaches the guard, so no one can trigger a capture of another's comment.
        let owned = post_comment::Entity::find_by_id(comment_id)
            .filter(post_comment::Column::UserId.eq(user.id))
            .one(&txn)
            .await?;
        let affected = match owned {
            Some(_) => {
                guard_open_flags(&txn, comment_id, evidence.confirm_evidence).await?;
                post_comment::Entity::delete(&txn, comment_id, user.id).await?
            }
            None => 0,
        };
        if affected > 0 {
            audit::record(
                &txn,
                AuditEvent {
                    actor_id: Some(user.id),
                    action: AuditAction::CommentDelete,
                    resource_id: comment_id.to_string(),
                    metadata: json!({ "by": "author" }),
                },
            )
            .await?;
        }
        txn.commit().await?;
        Ok::<u64, ErrorResponse>(affected)
    }
    .await;
    match deleted {
        Ok(1) => {
            info!(user_id = user.id, comment_id, "Comment deleted");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "Comment deleted successfully" })),
            ))
        }
        Ok(0) => {
            warn!(
                user_id = user.id,
                comment_id, "Comment not found for delete"
            );
            Err(ErrorResponse::new(ErrorCode::RecordNotFound)
                .with_message("Comment does not exist"))
        }
        Ok(_) => {
            info!(user_id = user.id, comment_id, "Comment deleted");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "Comment deleted successfully" })),
            ))
        }
        Err(err) => {
            error!(
                user_id = user.id,
                comment_id, "Failed to delete comment: {}", err
            );
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(post_id))]
pub async fn find_all_by_post(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(post_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let tx = state.sea_db.begin().await?;
    require_post_visibility(&tx, post_id, auth.user.as_ref()).await?;
    match post_comment::Entity::find_all_by_post(&tx, &state.storage.config.public_url, post_id)
        .await
    {
        Ok(comments) => {
            tx.commit().await?;
            info!(
                post_id,
                count = comments.len(),
                "Comments retrieved for post"
            );
            Ok((StatusCode::OK, Json(json!(comments))))
        }
        Err(err) => {
            error!(post_id, "Failed to retrieve comments: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, payload))]
pub async fn find_with_query(
    State(state): State<AppState>,
    payload: ValidatedJson<V1AdminPostCommentListQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let comment_query = payload.0.into_post_comment_query();
    let page = comment_query.page_no.unwrap_or(1);

    match post_comment::Entity::find_with_query(
        &state.sea_db,
        &state.storage.config.public_url,
        comment_query,
    )
    .await
    {
        Ok(result) => {
            info!(total = result.total, page, "Admin listed comments");
            Ok((
                StatusCode::OK,
                Json(json!({
                    "data": result.data,
                    "total": result.total,
                    "per_page": post_comment::Entity::PER_PAGE,
                    "page": page,
                })),
            ))
        }
        Err(err) => {
            error!("Failed to list comments: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(comment_id, admin_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn admin_hide(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
    payload: Option<Json<V1AdminCommentActionPayload>>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let reason = comment_notice_reason(payload.map(|Json(body)| body))?;
    let result = async {
        let txn = state.sea_db.begin().await?;
        let before = post_comment::Entity::find_by_id(comment_id)
            .one(&txn)
            .await?;
        let updated = post_comment::Entity::admin_hide(&txn, comment_id).await?;
        let mut notice = None;
        if updated.is_some() {
            audit::record(
                &txn,
                AuditEvent {
                    actor_id: Some(user.id),
                    action: AuditAction::CommentHide,
                    resource_id: comment_id.to_string(),
                    metadata: json!({}),
                },
            )
            .await?;
            // Only a visible comment opens a case: re-hiding adds no second notice (A27).
            let was_visible = before.as_ref().is_some_and(|comment| !comment.hidden);
            if let (true, Some(comment)) = (was_visible, updated.as_ref()) {
                notice = Some(record_comment_notice(&txn, user.id, comment, "hide", reason).await?);
            }
        }
        txn.commit().await?;
        Ok::<_, ErrorResponse>((updated, notice))
    }
    .await;

    match result {
        Ok((Some(_), notice)) => {
            info!(comment_id, "Admin hid comment");
            let delivery_state = match notice.as_ref() {
                Some(notice) => deliver_comment_notice(&state, notice).await,
                None => None,
            };
            Ok((
                StatusCode::OK,
                Json(json!({
                    "message": "Hidden successfully",
                    "noticeId": notice.as_ref().map(|n| n.id),
                    "deliveryState": delivery_state,
                })),
            ))
        }
        Ok((None, _)) => {
            warn!(comment_id, "Comment not found for hide");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound))
        }
        Err(err) => {
            error!(comment_id, "Failed to hide comment: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(comment_id, admin_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn admin_unhide(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let result = async {
        let txn = state.sea_db.begin().await?;
        let updated = post_comment::Entity::admin_unhide(&txn, comment_id).await?;
        if updated.is_some() {
            audit::record(
                &txn,
                AuditEvent {
                    actor_id: Some(user.id),
                    action: AuditAction::CommentUnhide,
                    resource_id: comment_id.to_string(),
                    metadata: json!({}),
                },
            )
            .await?;
        }
        txn.commit().await?;
        Ok::<_, ErrorResponse>(updated)
    }
    .await;

    match result {
        Ok(Some(_)) => {
            info!(comment_id, "Admin unhid comment");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "Unhidden successfully" })),
            ))
        }
        Ok(None) => {
            warn!(comment_id, "Comment not found for unhide");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound))
        }
        Err(err) => {
            error!(comment_id, "Failed to unhide comment: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(comment_id, admin_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn admin_delete(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
    Query(evidence): Query<V1ConfirmEvidenceQuery>,
    payload: Option<Json<V1AdminCommentActionPayload>>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let reason = comment_notice_reason(payload.map(|Json(body)| body))?;
    let result = async {
        let txn = state.sea_db.begin().await?;
        let existing = post_comment::Entity::find_by_id(comment_id)
            .one(&txn)
            .await?;
        let affected = if existing.is_some() {
            guard_open_flags(&txn, comment_id, evidence.confirm_evidence).await?;
            post_comment::Entity::admin_delete(&txn, comment_id).await?
        } else {
            0
        };
        let mut notice = None;
        if affected > 0 {
            audit::record(
                &txn,
                AuditEvent {
                    actor_id: Some(user.id),
                    action: AuditAction::CommentDelete,
                    resource_id: comment_id.to_string(),
                    metadata: json!({ "by": "admin" }),
                },
            )
            .await?;
            if let Some(comment) = existing.as_ref() {
                notice =
                    Some(record_comment_notice(&txn, user.id, comment, "delete", reason).await?);
            }
        }
        txn.commit().await?;
        Ok::<(u64, Option<enforcement_notice::Model>), ErrorResponse>((affected, notice))
    }
    .await;

    match result {
        Ok((affected, notice)) if affected > 0 => {
            info!(comment_id, "Admin deleted comment");
            let delivery_state = match notice.as_ref() {
                Some(notice) => deliver_comment_notice(&state, notice).await,
                None => None,
            };
            Ok((
                StatusCode::OK,
                Json(json!({
                    "message": "Deleted successfully",
                    "noticeId": notice.as_ref().map(|n| n.id),
                    "deliveryState": delivery_state,
                })),
            ))
        }
        Ok(_) => {
            warn!(comment_id, "Comment not found for admin delete");
            Err(ErrorResponse::new(ErrorCode::RecordNotFound))
        }
        Err(err) => {
            error!(comment_id, "Failed to admin delete comment: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth), fields(comment_id, admin_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn admin_flags_clear(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let result = async {
        let txn = state.sea_db.begin().await?;
        let updated = post_comment::Entity::admin_flags_clear(&txn, comment_id).await?;
        if updated.is_some() {
            audit::record(
                &txn,
                AuditEvent {
                    actor_id: Some(user.id),
                    action: AuditAction::CommentFlagsClear,
                    resource_id: comment_id.to_string(),
                    metadata: json!({}),
                },
            )
            .await?;
        }
        txn.commit().await?;
        Ok::<_, ErrorResponse>(updated)
    }
    .await;

    match result {
        Ok(Some(_)) | Ok(None) => {
            info!(comment_id, "Admin cleared comment flags");
            Ok((
                StatusCode::OK,
                Json(json!({ "message": "Flags cleared successfully" })),
            ))
        }
        Err(err) => {
            error!(comment_id, "Failed to clear flags: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id), comment_id))]
pub async fn flag(
    State(state): State<AppState>,
    auth: AuthSession,
    Path(comment_id): Path<i32>,
    payload: ValidatedJson<V1FlagCommentPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user.unwrap();
    let p = payload.0;

    // Every attempt counts toward the flag budget, whether or not it lands (A27).
    abuse_limiter::limiter(
        &state.gate_store,
        &format!("comment:flag:user:{}", user.id),
        flag_limit_config(),
    )
    .await?;

    let tx = state.sea_db.begin().await?;
    let comment = post_comment::Entity::find_by_id(comment_id)
        .one(&tx)
        .await?
        .ok_or_else(|| ErrorResponse::new(ErrorCode::RecordNotFound))?;
    require_post_visibility(&tx, comment.post_id, Some(&user)).await?;
    if comment.user_id == user.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("You cannot flag your own comment."));
    }

    let new_flag = comment_flag::slice::NewCommentFlag {
        comment_id,
        user_id: user.id,
        reason: p.reason,
    };

    match comment_flag::Entity::create(&tx, new_flag).await {
        Ok(_flag) => {
            let count = comment_flag::Entity::sync_flags_count(&tx, comment_id).await?;
            tx.commit().await?;
            info!(
                user_id = user.id,
                comment_id,
                flags_count = count,
                "Comment flagged"
            );
            Ok(Json(
                json!({ "message": "Flag recorded", "flags_count": count }),
            ))
        }
        Err(err) => {
            error!(
                user_id = user.id,
                comment_id, "Failed to flag comment: {}", err
            );
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, _auth, payload))]
pub async fn admin_flags_list(
    State(state): State<AppState>,
    _auth: AuthSession,
    payload: ValidatedJson<V1AdminCommentFlagListQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let q = comment_flag::slice::CommentFlagQuery {
        page_no: payload.page,
        comment_id: payload.comment_id,
        user_id: payload.user_id,
        search_term: payload.search.clone(),
        sort_by: payload.sort_by.clone(),
        sort_order: payload.sort_order.clone(),
    };

    match comment_flag::Entity::list(&state.sea_db, &state.storage.config.public_url, q).await {
        Ok(result) => {
            info!(
                total = result.total,
                page = payload.page.unwrap_or(1),
                "Admin listed comment flags"
            );
            Ok(Json(json!({
                "data": result.data,
                "total": result.total,
                "page": payload.page.unwrap_or(1)
            })))
        }
        Err(err) => {
            error!("Failed to list comment flags: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, _auth), fields(comment_id))]
pub async fn admin_flags_summary(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(comment_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    match comment_flag::Entity::summary_for_comment(&state.sea_db, comment_id).await {
        Ok(summary) => {
            info!(comment_id, "Admin viewed comment flags summary");
            Ok(Json(json!(summary)))
        }
        Err(err) => {
            error!(comment_id, "Failed to get flags summary: {}", err);
            Err(err)
        }
    }
}

#[debug_handler]
#[instrument(skip(state, _auth), fields(comment_id))]
pub async fn admin_flags_details(
    State(state): State<AppState>,
    _auth: AuthSession,
    Path(comment_id): Path<i32>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let q = comment_flag::slice::CommentFlagQuery {
        comment_id: Some(comment_id),
        ..Default::default()
    };

    match comment_flag::Entity::list(&state.sea_db, &state.storage.config.public_url, q).await {
        Ok(result) => {
            info!(
                comment_id,
                count = result.data.len(),
                "Admin viewed comment flags details"
            );
            Ok(Json(json!(result.data)))
        }
        Err(err) => {
            error!(comment_id, "Failed to get flags details: {}", err);
            Err(err)
        }
    }
}
