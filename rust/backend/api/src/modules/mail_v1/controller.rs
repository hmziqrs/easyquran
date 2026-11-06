use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use bytes::Bytes;
use serde_json::json;

use crate::{
    db::sea_models::email_suppression::{
        Entity as SuppressionEntity, NewSuppression, SuppressionQuery,
    },
    error::{ErrorCode, ErrorResponse},
    services::{
        audit::{self, AuditAction, AuditEvent},
        auth::AuthSession,
    },
    AppState,
};

use super::validator::{
    V1CreateSuppression, V1DeleteSuppression, V1ListSuppressionsQuery, V1ReviewSuppression,
};

use {
    crate::{
        db::sea_models::email_suppression::{SuppressionReason, SuppressionUpsert},
        services::{abuse_limiter::limiter, mail::provider::WebhookEvent},
    },
    sha2::{Digest, Sha256},
};

/// Webhook intake budget (A29). Keyed by provider name, not IP: the caller is the
/// mail provider's delivery service, so one IP bucket would throttle every event.
/// Sized far above real volume; it stops a flood of forged or replayed posts.
fn webhook_limit_config() -> crate::services::abuse_limiter::AbuseLimiterConfig {
    crate::services::abuse_limiter::AbuseLimiterConfig {
        temp_block_attempts: 600,
        temp_block_range: 60,
        temp_block_duration: 60,
        block_retry_limit: 10_000,
        block_range: 60 * 60,
        block_duration: 60 * 60,
    }
}

pub async fn list_suppressions(
    State(state): State<AppState>,
    Query(q): Query<V1ListSuppressionsQuery>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let query = SuppressionQuery {
        page: q.page,
        reason: q.reason,
        permanent: q.permanent,
        search: q.search,
    };
    let result = SuppressionEntity::find_with_query(&state.sea_db, query).await?;
    Ok(Json(
        json!({ "data": result.data, "total": result.total, "page": result.page, "per_page": result.per_page }),
    ))
}

pub async fn create_suppression(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(payload): Json<V1CreateSuppression>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let actor_id = auth.user_required()?.id;
    let model = SuppressionEntity::create(
        &state.sea_db,
        NewSuppression {
            recipient: payload.recipient,
            reason: payload.reason,
            source: Some("admin".to_string()),
            diagnostic: payload.diagnostic,
            permanent: payload.permanent,
        },
    )
    .await?;
    audit::record(
        &state.sea_db,
        AuditEvent {
            actor_id: Some(actor_id),
            action: AuditAction::SuppressionAdd,
            resource_id: model.recipient.clone(),
            metadata: json!({ "source": "admin", "permanent": model.permanent }),
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": model.id, "recipient": model.recipient, "permanent": model.permanent })),
    ))
}

pub async fn review_suppression(
    State(state): State<AppState>,
    auth: AuthSession,
    Json(payload): Json<V1ReviewSuppression>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let actor_id = auth.user_required()?.id;
    let recipient = payload.recipient.trim().to_lowercase();
    let reviewed = SuppressionEntity::review(&state.sea_db, &recipient)
        .await?
        .is_some();
    if reviewed {
        audit::record(
            &state.sea_db,
            AuditEvent {
                actor_id: Some(actor_id),
                action: AuditAction::SuppressionReview,
                resource_id: recipient.clone(),
                metadata: json!({ "blocks_transactional": true }),
            },
        )
        .await?;
    }
    Ok(Json(
        json!({ "reviewed": reviewed, "recipient": recipient }),
    ))
}

pub async fn delete_suppression(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(q): Query<V1DeleteSuppression>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let actor_id = auth.user_required()?.id;
    let recipient = q.recipient.trim().to_lowercase();
    if recipient.is_empty() {
        return Err(ErrorResponse::new(ErrorCode::InvalidEmailFormat)
            .with_message("recipient query param is required"));
    }
    let removed = SuppressionEntity::delete_by_recipient(&state.sea_db, &recipient).await?;
    if removed {
        audit::record(
            &state.sea_db,
            AuditEvent {
                actor_id: Some(actor_id),
                action: AuditAction::SuppressionDelete,
                resource_id: recipient.clone(),
                metadata: json!({}),
            },
        )
        .await?;
    }
    Ok(Json(json!({ "removed": removed, "recipient": recipient })))
}

#[allow(unused_variables)]
pub async fn mail_webhook_receiver(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    {
        use crate::services::mail::MailProvider;

        limiter(&state.gate_store, "mail:webhook", webhook_limit_config()).await?;

        let event = WebhookEvent {
            provider: provider.clone(),
            payload: body.to_vec(),
            headers,
            query: None,
        };

        let parsed = state.mailer.verify_webhook(event).await.map_err(|e| {
            tracing::warn!(error = %e, "mail webhook verification failed");
            crate::services::mail::mail_error_to_response(&e)
        })?;

        // Apply before claiming dedup key: if apply fails, retry re-processes instead of losing the message; upsert idempotent.
        apply_mail_event(&state, &parsed).await?;

        let mut hasher = Sha256::new();
        hasher.update(body.as_ref());
        let dedup_key = format!(
            "mail:webhook:{}:{}",
            provider,
            hex::encode(hasher.finalize())
        );
        if !rux_request_gate::dedup_nx(&state.gate_store, &dedup_key, 86_400).await {
            tracing::info!(%provider, "Duplicate mail webhook (already processed); acknowledging");
            return Ok(Json(json!({ "received": true, "duplicate": true })));
        }

        Ok(Json(json!({ "received": true })))
    }
}

async fn apply_mail_event(
    state: &AppState,
    ev: &crate::services::mail::provider::ParsedMailEvent,
) -> Result<(), ErrorResponse> {
    use crate::services::mail::provider::canonical;

    // PII: log only the recipient domain, never the full address.
    let recipient_domain = ev.recipient.split('@').nth(1).unwrap_or("unknown");
    match ev.event_type.as_str() {
        canonical::BOUNCED => {
            tracing::info!(%recipient_domain, permanent = ev.permanent, "mail bounce event -> suppressing");
            SuppressionEntity::upsert(
                &state.sea_db,
                &ev.recipient,
                SuppressionUpsert {
                    reason: SuppressionReason::Bounce,
                    source: Some("mail-webhook".to_string()),
                    diagnostic: ev.diagnostic.clone(),
                    permanent: ev.permanent,
                    blocks_transactional: false,
                },
            )
            .await?;
            state.mailer.note_bounce().await;
            audit::record(
                &state.sea_db,
                AuditEvent {
                    actor_id: None,
                    action: AuditAction::SuppressionAdd,
                    resource_id: ev.recipient.trim().to_lowercase(),
                    metadata: json!({ "source": "mail-webhook", "reason": "bounce" }),
                },
            )
            .await?;
        }
        canonical::COMPLAINED => {
            tracing::info!(%recipient_domain, "mail complaint event -> suppressing (permanent)");
            SuppressionEntity::upsert(
                &state.sea_db,
                &ev.recipient,
                SuppressionUpsert {
                    reason: SuppressionReason::Complaint,
                    source: Some("mail-webhook".to_string()),
                    diagnostic: ev.diagnostic.clone(),
                    permanent: true,
                    blocks_transactional: false,
                },
            )
            .await?;
            audit::record(
                &state.sea_db,
                AuditEvent {
                    actor_id: None,
                    action: AuditAction::SuppressionAdd,
                    resource_id: ev.recipient.trim().to_lowercase(),
                    metadata: json!({ "source": "mail-webhook", "reason": "complaint" }),
                },
            )
            .await?;
        }
        canonical::DELIVERED => {
            tracing::debug!(%recipient_domain, "mail delivered event");
        }
        other => {
            tracing::warn!(event_type = %other, "unknown mail event type; ignoring");
        }
    }
    Ok(())
}
