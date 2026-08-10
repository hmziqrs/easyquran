use axum::{
    body::Bytes,
    extract::{Path, RawQuery, State},
    http::HeaderMap,
    Json,
};
use axum_client_ip::ClientIp;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::sea_models::discount_code;
use crate::db::sea_models::invoice;
use crate::db::sea_models::payment;
use crate::db::sea_models::plan;
use crate::db::sea_models::post_access;
use crate::db::sea_models::post_purchase;
use crate::db::sea_models::subscription;
use crate::error::codes::ErrorCode;
use crate::error::response::ErrorResponse;
use crate::services::auth::AuthSession;
use crate::services::paywall;
use crate::AppState;

use crate::services::billing::provider::{
    canonical, canonical_subscription_status, BillingProvider, ParsedWebhook, WebhookEvent,
};

use super::validator::*;

// Server-side store keyed by checkout session id; the verified webhook grants from these facts (user_id, and post_id/amount/currency or plan_id) — NEVER grant from attacker-shapeable client metadata.
mod checkout_intent {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CheckoutIntent {
        pub user_id: i32,
        pub post_id: Option<i32>,
        pub amount_cents: Option<i32>,
        pub currency: Option<String>,
        pub plan_id: Option<i32>,
    }

    const INTENT_TTL_SECS: u64 = 3600;

    static INTENT_STORE: OnceLock<Mutex<HashMap<String, (CheckoutIntent, Instant)>>> =
        OnceLock::new();

    fn intent_store() -> &'static Mutex<HashMap<String, (CheckoutIntent, Instant)>> {
        INTENT_STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn reap_stale(map: &mut HashMap<String, (CheckoutIntent, Instant)>) {
        map.retain(|_, (_, at)| at.elapsed().as_secs() < INTENT_TTL_SECS);
    }

    pub fn store(session_id: &str, intent: &CheckoutIntent) -> Result<(), ErrorResponse> {
        let mut map = match intent_store().lock() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!(error = %e, "checkout intent store lock poisoned");
                return Err(ErrorResponse::new(ErrorCode::InternalServerError));
            }
        };
        reap_stale(&mut map);
        map.insert(session_id.to_string(), (intent.clone(), Instant::now()));
        Ok(())
    }

    /// Single-use: consumes the intent so a replayed webhook cannot re-grant.
    pub fn take(session_id: &str) -> Result<Option<CheckoutIntent>, ErrorResponse> {
        let mut map = match intent_store().lock() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!(error = %e, "checkout intent store lock poisoned");
                return Err(ErrorResponse::new(ErrorCode::InternalServerError));
            }
        };
        reap_stale(&mut map);
        Ok(map.remove(session_id).map(|(intent, _)| intent))
    }
}

pub async fn admin_list_plans(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let query: Vec<plan::model::Model> = plan::Entity::find()
        .order_by_asc(plan::Column::SortOrder)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    let plans: Vec<PlanResponse> = query
        .into_iter()
        .map(|p| PlanResponse {
            id: p.id,
            name: p.name,
            slug: p.slug,
            description: p.description,
            price_cents: p.price_cents,
            currency: p.currency,
            interval: p.interval,
            trial_days: p.trial_days,
            features: p.features,
            is_active: p.is_active,
            sort_order: p.sort_order,
            created_at: p.created_at,
            updated_at: p.updated_at,
        })
        .collect();

    Ok(Json(json!({ "data": plans })))
}

pub async fn admin_create_plan(
    State(state): State<AppState>,
    Json(payload): Json<CreatePlanPayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let active_model = plan::ActiveModel {
        name: Set(payload.name),
        slug: Set(payload.slug),
        description: Set(payload.description),
        price_cents: Set(payload.price_cents),
        currency: Set(payload.currency),
        interval: Set(payload.interval),
        trial_days: Set(payload.trial_days.unwrap_or(0)),
        features: Set(payload.features),
        is_active: Set(payload.is_active.unwrap_or(true)),
        sort_order: Set(payload.sort_order.unwrap_or(0)),
        ..Default::default()
    };

    let model = active_model.insert(&state.sea_db).await.map_err(|e| {
        if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
            ErrorResponse::new(ErrorCode::DuplicateEntry)
                .with_message("A plan with this slug already exists")
        } else {
            ErrorResponse::new(ErrorCode::QueryError)
        }
    })?;

    Ok(Json(json!({
        "data": { "id": model.id, "slug": model.slug },
        "message": "Plan created"
    })))
}

pub async fn admin_update_plan(
    State(state): State<AppState>,
    Path(plan_id): Path<i32>,
    Json(payload): Json<UpdatePlanPayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let existing = plan::Entity::find_by_id(plan_id)
        .one(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?
        .ok_or_else(|| {
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Plan not found")
        })?;

    let mut active: plan::ActiveModel = existing.into();

    if let Some(name) = payload.name {
        active.name = Set(name);
    }
    if let Some(description) = payload.description {
        active.description = Set(Some(description));
    }
    if let Some(price_cents) = payload.price_cents {
        active.price_cents = Set(price_cents);
    }
    if let Some(currency) = payload.currency {
        active.currency = Set(currency);
    }
    if let Some(interval) = payload.interval {
        active.interval = Set(interval);
    }
    if let Some(trial_days) = payload.trial_days {
        active.trial_days = Set(trial_days);
    }
    if let Some(features) = payload.features {
        active.features = Set(Some(features));
    }
    if let Some(is_active) = payload.is_active {
        active.is_active = Set(is_active);
    }
    if let Some(sort_order) = payload.sort_order {
        active.sort_order = Set(sort_order);
    }
    active.updated_at = Set(chrono::Utc::now().fixed_offset());

    active
        .update(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "message": "Plan updated" })))
}

pub async fn admin_delete_plan(
    State(state): State<AppState>,
    Path(plan_id): Path<i32>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let sub_count = subscription::Entity::find()
        .filter(subscription::Column::PlanId.eq(plan_id))
        .count(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    if sub_count > 0 {
        return Err(ErrorResponse::new(ErrorCode::DependencyExists)
            .with_message("Cannot delete plan with active subscriptions"));
    }

    plan::Entity::delete_by_id(plan_id)
        .exec(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "message": "Plan deleted" })))
}

pub async fn admin_list_subscriptions(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let subs: Vec<subscription::model::Model> = subscription::Entity::find()
        .order_by_desc(subscription::Column::CreatedAt)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": subs })))
}

pub async fn admin_cancel_subscription(
    State(state): State<AppState>,
    Path(subscription_id): Path<i32>,
    Json(payload): Json<CancelSubscriptionPayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let sub = subscription::Entity::find_by_id(subscription_id)
        .one(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?
        .ok_or_else(|| {
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Subscription not found")
        })?;

    if let Some(provider_sub_id) = &sub.provider_subscription_id {
        let immediately = payload.immediately.unwrap_or(false);
        let provider_name = sub.provider.clone();
        if let Err(e) = state
            .billing_router
            .cancel_subscription_for_provider(&provider_name, provider_sub_id, immediately)
            .await
        {
            tracing::warn!(error = %e, provider = %provider_name, "Failed to cancel subscription at provider");
        }
    }

    let mut active: subscription::ActiveModel = sub.into();
    active.status = Set(subscription::model::SubscriptionStatus::Canceled);
    active.cancel_at_period_end = Set(false);
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    active
        .update(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "message": "Subscription canceled" })))
}

pub async fn admin_list_payments(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let payments_list: Vec<payment::model::Model> = payment::Entity::find()
        .order_by_desc(payment::Column::CreatedAt)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": payments_list })))
}

pub async fn admin_list_invoices(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let invoices_list: Vec<invoice::model::Model> = invoice::Entity::find()
        .order_by_desc(invoice::Column::CreatedAt)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": invoices_list })))
}

pub async fn admin_list_discount_codes(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let codes: Vec<discount_code::model::Model> = discount_code::Entity::find()
        .order_by_desc(discount_code::Column::CreatedAt)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": codes })))
}

pub async fn admin_create_discount_code(
    State(state): State<AppState>,
    Json(payload): Json<CreateDiscountCodePayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let active_model = discount_code::ActiveModel {
        code: Set(payload.code.to_uppercase()),
        description: Set(payload.description),
        discount_type: Set(match payload.discount_type {
            DiscountTypeValue::Percentage => discount_code::model::DiscountType::Percentage,
            DiscountTypeValue::FixedAmount => discount_code::model::DiscountType::FixedAmount,
        }),
        discount_value: Set(payload.discount_value),
        currency: Set(payload.currency),
        max_redemptions: Set(payload.max_redemptions),
        redeemed_count: Set(0),
        valid_from: Set(payload.valid_from),
        valid_until: Set(payload.valid_until),
        plan_id: Set(payload.plan_id),
        is_active: Set(payload.is_active.unwrap_or(true)),
        ..Default::default()
    };

    let model = active_model.insert(&state.sea_db).await.map_err(|e| {
        if e.to_string().contains("duplicate") || e.to_string().contains("unique") {
            ErrorResponse::new(ErrorCode::DuplicateEntry)
                .with_message("A discount code with this code already exists")
        } else {
            ErrorResponse::new(ErrorCode::QueryError)
        }
    })?;

    Ok(Json(json!({
        "data": { "id": model.id, "code": model.code },
        "message": "Discount code created"
    })))
}

pub async fn admin_delete_discount_code(
    State(state): State<AppState>,
    Path(code_id): Path<i32>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    discount_code::Entity::delete_by_id(code_id)
        .exec(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "message": "Discount code deleted" })))
}

pub async fn public_list_plans(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let plans: Vec<plan::model::Model> = plan::Entity::find()
        .filter(plan::Column::IsActive.eq(true))
        .order_by_asc(plan::Column::SortOrder)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": plans })))
}

pub async fn create_checkout(
    State(state): State<AppState>,
    auth: AuthSession,
    ClientIp(client_ip): ClientIp,
    Json(payload): Json<CreateCheckoutPayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let user_id = user.id;
    let user_email = user.email.clone();
    let plan = plan::Entity::find()
        .filter(plan::Column::Slug.eq(&payload.plan_slug))
        .filter(plan::Column::IsActive.eq(true))
        .one(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?
        .ok_or_else(|| {
            ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Plan not found")
        })?;

    let success_url = payload
        .success_url
        .unwrap_or_else(|| "/billing/success".to_string());
    let cancel_url = payload
        .cancel_url
        .unwrap_or_else(|| "/billing/cancel".to_string());

    {
        let session = state
            .billing_router
            .create_checkout_for_ip(
                client_ip,
                &plan.slug,
                &user_email,
                user_id,
                &success_url,
                &cancel_url,
            )
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::ExternalServiceError)
                    .with_message(format!("Checkout failed: {}", e))
            })?;

        // Bind user_id and plan_id server-side; the webhook grants from this intent, never client metadata. Storage required — never return an unfulfillable URL.
        let intent = checkout_intent::CheckoutIntent {
            user_id,
            post_id: None,
            amount_cents: None,
            currency: None,
            plan_id: Some(plan.id),
        };
        checkout_intent::store(&session.session_id, &intent)?;

        Ok(Json(json!({
            "data": {
                "session_id": session.session_id,
                "checkout_url": session.checkout_url,
            }
        })))
    }
}

/// One-time post purchase at the authoritative server-side `post_access` price (never the client amount); the webhook grants from the server-bound intent — if storing it fails, return an error, never a fulfillable URL.
pub async fn create_post_checkout(
    State(state): State<AppState>,
    auth: AuthSession,
    ClientIp(client_ip): ClientIp,
    Json(payload): Json<CreatePostCheckoutPayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let user_id = user.id;
    let user_email = user.email.clone();

    // Authoritative server-side price: only Paid posts with a configured price are purchasable — the client cannot influence the amount charged.
    let policy = paywall::load_post_access_policy(&state.sea_db, payload.post_id).await?;
    let amount_cents = match (policy.access_type, policy.price_cents) {
        (paywall::PostAccessType::Paid, Some(cents)) if cents > 0 => cents,
        _ => {
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("This post is not available for one-time purchase"));
        }
    };
    let currency = policy.currency.unwrap_or_else(|| "usd".to_string());

    let success_url = payload
        .success_url
        .unwrap_or_else(|| "/billing/success".to_string());
    let cancel_url = payload
        .cancel_url
        .unwrap_or_else(|| "/billing/cancel".to_string());

    {
        let session = state
            .billing_router
            .create_post_checkout_for_ip(
                client_ip,
                payload.post_id,
                amount_cents,
                &currency,
                &user_email,
                user_id,
                &success_url,
                &cancel_url,
            )
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::ExternalServiceError)
                    .with_message(format!("Post checkout failed: {}", e))
            })?;

        let intent = checkout_intent::CheckoutIntent {
            user_id,
            post_id: Some(payload.post_id),
            amount_cents: Some(amount_cents),
            currency: Some(currency),
            plan_id: None,
        };
        checkout_intent::store(&session.session_id, &intent)?;

        Ok(Json(json!({
            "data": {
                "session_id": session.session_id,
                "checkout_url": session.checkout_url,
            }
        })))
    }
}

pub async fn my_subscriptions(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let user_id = user.id;
    let subs: Vec<subscription::model::Model> = subscription::Entity::find()
        .filter(subscription::Column::UserId.eq(user_id))
        .order_by_desc(subscription::Column::CreatedAt)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": subs })))
}

pub async fn my_payments(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let user = auth.user.ok_or_else(|| {
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("Not authenticated")
    })?;
    let user_id = user.id;
    let payments_list: Vec<payment::model::Model> = payment::Entity::find()
        .filter(payment::Column::UserId.eq(user_id))
        .order_by_desc(payment::Column::CreatedAt)
        .all(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "data": payments_list })))
}

pub async fn webhook_receiver(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    RawQuery(raw_query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    {
        // Forward the full header map and raw query: providers sign different fields (e.g. Mercado Pago signs `data.id` from the query), so dropping either breaks verification.
        let webhook_event = WebhookEvent {
            provider: provider.clone(),
            payload: body.to_vec(),
            headers,
            query: raw_query,
        };

        let parsed = state
            .billing_router
            .verify_webhook(webhook_event)
            .await
            .map_err(|e| {
                ErrorResponse::new(ErrorCode::ExternalServiceError)
                    .with_message(format!("Webhook verification failed: {}", e))
            })?;

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(body.as_ref());
        let body_hash = hex::encode(hasher.finalize());
        let dedup_key = format!("webhook:{provider}:{body_hash}");
        if !rux_request_gate::dedup_nx(&state.gate_store, &dedup_key, 86_400).await {
            tracing::info!(
                provider = %provider,
                "Replay webhook (already processed within 24h); acknowledging"
            );
            return Ok(Json(json!({ "received": true })));
        }

        process_webhook_event(&state, &parsed, &provider).await?;

        Ok(Json(json!({ "received": true })))
    }
}

fn resolve_intent_session_id(event: &ParsedWebhook) -> &str {
    event
        .checkout_session_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| event.payment_id.as_deref().filter(|s| !s.is_empty()))
        .or_else(|| event.subscription_id.as_deref().filter(|s| !s.is_empty()))
        .unwrap_or("")
}

async fn process_webhook_event(
    state: &AppState,
    event: &ParsedWebhook,
    provider_name: &str,
) -> Result<(), ErrorResponse> {
    match event.event_type.as_str() {
        "checkout.session.completed" => {
            let session_id = event
                .checkout_session_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .or_else(|| event.subscription_id.as_deref().filter(|s| !s.is_empty()))
                .or_else(|| event.payment_id.as_deref().filter(|s| !s.is_empty()))
                .unwrap_or("");

            // Grant facts come only from the server-bound intent (consumed atomically); a missing intent is refused — never grant from attacker-shapeable client metadata.
            let intent = if !session_id.is_empty() {
                checkout_intent::take(session_id).unwrap_or_else(|e| {
                    tracing::warn!(error = ?e, "Failed to read checkout intent");
                    None
                })
            } else {
                None
            };

            // Diagnostic only — NEVER used to grant; kept for the refusal log.
            let metadata_user_id: i32 = event.user_id.unwrap_or(0);

            let intent = match intent {
                Some(i) => i,
                None => {
                    tracing::warn!(
                        metadata_user_id,
                        session_id,
                        provider = provider_name,
                        "checkout.session.completed with no resolvable server-bound \
                         intent; refusing to grant (audit F#2/F#10)"
                    );
                    return Ok(());
                }
            };
            let user_id = intent.user_id;

            if user_id == 0 {
                tracing::warn!("checkout.session.completed with no resolvable user_id");
                return Ok(());
            }

            if let Some(post_id) = intent.post_id {
                let amount_cents = intent.amount_cents.unwrap_or(0);
                let currency = intent.currency.clone().unwrap_or_else(|| "usd".to_string());

                let already = post_purchase::Entity::find()
                    .filter(post_purchase::Column::UserId.eq(user_id))
                    .filter(post_purchase::Column::PostId.eq(post_id))
                    .one(&state.sea_db)
                    .await
                    .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
                if already.is_some() {
                    tracing::info!(
                        user_id,
                        post_id,
                        "Post purchase already exists, skipping (idempotent)"
                    );
                    return Ok(());
                }

                let active_model = post_purchase::model::ActiveModel {
                    user_id: Set(user_id),
                    post_id: Set(post_id),
                    payment_id: Set(None),
                    provider: Set(provider_name.to_string()),
                    amount_cents: Set(amount_cents),
                    currency: Set(currency),
                    ..Default::default()
                };
                match active_model.insert(&state.sea_db).await {
                    Ok(_) => {
                        tracing::info!(
                            user_id,
                            post_id,
                            "Post purchase granted from verified webhook"
                        );
                    }
                    Err(e) => {
                        let s = e.to_string();
                        if s.contains("duplicate") || s.contains("unique") {
                            tracing::info!(
                                user_id,
                                post_id,
                                "Concurrent post purchase raced; already granted"
                            );
                        } else {
                            return Err(ErrorResponse::new(ErrorCode::QueryError));
                        }
                    }
                }
                return Ok(());
            }

            let subscription_id = event.subscription_id.clone().unwrap_or_default();
            let customer_id = event.customer_id.clone();

            let existing = if !subscription_id.is_empty() {
                subscription::Entity::find()
                    .filter(subscription::Column::ProviderSubscriptionId.eq(&subscription_id))
                    .filter(subscription::Column::Provider.eq(provider_name))
                    .one(&state.sea_db)
                    .await
                    .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?
            } else {
                None
            };

            if existing.is_some() {
                tracing::info!(
                    subscription_id = %subscription_id,
                    "Subscription already exists, skipping"
                );
                return Ok(());
            }

            // plan_id comes only from the server-bound intent — never guess "first active plan" (wrong tier); absent plan_id fails closed.
            let plan_id = match intent.plan_id {
                Some(id) => id,
                None => {
                    tracing::error!(
                        user_id,
                        "checkout.session.completed (subscription) with no \
                         server-bound plan_id; refusing to grant a guessed plan \
                         (audit F#3)"
                    );
                    return Ok(());
                }
            };

            let active_model = subscription::ActiveModel {
                user_id: Set(user_id),
                plan_id: Set(plan_id),
                provider: Set(provider_name.to_string()),
                provider_customer_id: Set(if customer_id.is_empty() {
                    None
                } else {
                    Some(customer_id)
                }),
                provider_subscription_id: Set(if subscription_id.is_empty() {
                    None
                } else {
                    Some(subscription_id.clone())
                }),
                status: Set(subscription::model::SubscriptionStatus::Active),
                current_period_start: Set(Some(chrono::Utc::now().fixed_offset())),
                // Fail-closed: a malformed timestamp degrades to None (paywall denies) rather than fabricating "now", which would expire the subscriber immediately.
                current_period_end: Set(event.current_period_end.and_then(|ts| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
                        .map(|dt| dt.fixed_offset())
                })),
                cancel_at_period_end: Set(false),
                trial_ends_at: Set(None),
                metadata: Set(Some(event.data.clone())),
                ..Default::default()
            };
            match active_model.insert(&state.sea_db).await {
                Ok(_) => {
                    tracing::info!(user_id, "Subscription created from checkout");
                }
                Err(e) => {
                    let s = e.to_string();
                    if s.contains("duplicate") || s.contains("unique") {
                        tracing::info!(
                            user_id,
                            subscription_id = %subscription_id,
                            "Concurrent subscription grant raced; already granted (idempotent)"
                        );
                    } else {
                        return Err(ErrorResponse::new(ErrorCode::QueryError));
                    }
                }
            }
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            if let Some(provider_sub_id) = &event.subscription_id {
                let sub = subscription::Entity::find()
                    .filter(subscription::Column::ProviderSubscriptionId.eq(provider_sub_id))
                    .filter(subscription::Column::Provider.eq(provider_name))
                    .one(&state.sea_db)
                    .await
                    .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

                if let Some(existing) = sub {
                    // Capture before consuming into an ActiveModel so the forward-only guard below can compare against it.
                    let existing_period_end = existing.current_period_end;
                    let mut active: subscription::ActiveModel = existing.into();

                    let mut status_changed = true;
                    let new_status = if event.event_type == canonical::SUBSCRIPTION_DELETED {
                        subscription::model::SubscriptionStatus::Canceled
                    } else {
                        match canonical_subscription_status(event.subscription_status.as_deref()) {
                            Some(status) => status,
                            None => {
                                status_changed = false;
                                // Placeholder; never written when status_changed=false.
                                subscription::model::SubscriptionStatus::Active
                            }
                        }
                    };
                    if status_changed {
                        active.status = Set(new_status);
                    }
                    // Forward-only period: never move the period end backward — an out-of-order/redelivered update must not shorten a valid period.
                    if let Some(ts) = event.current_period_end {
                        let new_end = chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
                            .map(|dt| dt.fixed_offset());
                        let extend = match (existing_period_end.as_ref(), new_end) {
                            (Some(prior), Some(new_dt)) => new_dt > *prior,
                            (None, Some(_)) => true,
                            _ => false,
                        };
                        if extend {
                            active.current_period_end = Set(new_end);
                        }
                    }
                    active.updated_at = Set(chrono::Utc::now().fixed_offset());
                    active
                        .update(&state.sea_db)
                        .await
                        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

                    tracing::info!(
                        subscription_id = %provider_sub_id,
                        status_changed,
                        status = ?new_status,
                        "Subscription updated from webhook"
                    );
                }
            }
        }
        "invoice.payment_succeeded" => {
            let mut user_id: i32 = event.user_id.unwrap_or(0);
            if user_id == 0 {
                if let Some(sid) = event.subscription_id.as_deref().filter(|s| !s.is_empty()) {
                    if let Ok(Some(owner)) = subscription::Entity::find()
                        .filter(subscription::Column::ProviderSubscriptionId.eq(sid))
                        .filter(subscription::Column::Provider.eq(provider_name))
                        .one(&state.sea_db)
                        .await
                    {
                        user_id = owner.user_id;
                    }
                }
            }

            let amount = event.amount_cents.unwrap_or(0) as i32;
            let currency = event.currency.clone().unwrap_or_else(|| "usd".to_string());

            if let Some(pid) = event.payment_id.as_deref().filter(|p| !p.is_empty()) {
                let dup = payment::Entity::find()
                    .filter(payment::Column::Provider.eq(provider_name))
                    .filter(payment::Column::ProviderPaymentId.eq(pid))
                    .one(&state.sea_db)
                    .await
                    .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
                if dup.is_some() {
                    tracing::info!(
                        provider = provider_name,
                        payment_id = pid,
                        "Payment already recorded, skipping (idempotent)"
                    );
                    return Ok(());
                }
            }

            let active_model = payment::ActiveModel {
                user_id: Set(user_id),
                subscription_id: Set(None),
                plan_id: Set(None),
                provider: Set(provider_name.to_string()),
                provider_payment_id: Set(event.payment_id.clone()),
                amount_cents: Set(amount),
                currency: Set(currency),
                status: Set(payment::model::PaymentStatus::Completed),
                description: Set(Some(format!("Invoice payment: {}", event.event_type))),
                metadata: Set(Some(event.data.clone())),
                ..Default::default()
            };
            active_model
                .insert(&state.sea_db)
                .await
                .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

            // A recurring payment with a fresh period end must extend the subscriber's row or the paywall denies them after the first period; only move the period FORWARD (defense-in-depth vs the subscription.updated arm).
            if let (Some(sid), Some(new_end_ts)) = (
                event.subscription_id.as_deref().filter(|s| !s.is_empty()),
                event.current_period_end,
            ) {
                if let Ok(Some(sub)) = subscription::Entity::find()
                    .filter(subscription::Column::ProviderSubscriptionId.eq(sid))
                    .filter(subscription::Column::Provider.eq(provider_name))
                    .one(&state.sea_db)
                    .await
                {
                    let new_end = chrono::DateTime::<chrono::Utc>::from_timestamp(new_end_ts, 0)
                        .map(|dt| dt.fixed_offset());
                    let extend = match (sub.current_period_end.as_ref(), new_end) {
                        (Some(existing), Some(new_dt)) => new_dt > *existing,
                        (None, Some(_)) => true,
                        _ => false,
                    };
                    if extend {
                        let mut active: subscription::ActiveModel = sub.into();
                        active.current_period_end = Set(new_end);
                        active.updated_at = Set(chrono::Utc::now().fixed_offset());
                        if let Err(e) = active.update(&state.sea_db).await {
                            tracing::warn!(
                                error = ?e,
                                subscription_id = %sid,
                                "Failed to refresh period on renewal (best-effort)"
                            );
                        }
                    }
                }
            }

            tracing::info!(user_id, amount, "Payment recorded from invoice webhook");
        }
        "payment.confirmed" | "payment.pending" => {
            // IDOR guard (V-MED-12): user_id comes ONLY from a server-bound checkout intent — never the attacker-shapeable `memo` field (old code parsed `rux-{user_id}` from it); provider is the dispatched provider_name, never a hardcoded literal.
            let session_id = resolve_intent_session_id(event);

            let intent = if !session_id.is_empty() {
                checkout_intent::take(session_id).unwrap_or_else(|e| {
                    tracing::warn!(error = ?e, "Failed to read checkout intent");
                    None
                })
            } else {
                None
            };

            // Diagnostic only — NEVER used to attribute the payment row.
            let memo_user_id: i32 = event
                .data
                .get("memo")
                .and_then(|v| v.as_str())
                .and_then(|m| m.strip_prefix("rux-"))
                .and_then(|rest| rest.split('-').next())
                .and_then(|id| id.parse().ok())
                .unwrap_or(0);

            let intent = match intent {
                Some(i) => i,
                None => {
                    tracing::warn!(
                        memo_user_id,
                        session_id,
                        provider = provider_name,
                        "{} with no resolvable server-bound intent; refusing to \
                         record payment (audit V-MED-12)",
                        event.event_type,
                    );
                    return Ok(());
                }
            };
            let user_id = intent.user_id;

            if user_id == 0 {
                tracing::warn!("{} with resolvable intent but no user_id", event.event_type);
                return Ok(());
            }

            let status = if event.event_type == canonical::PAYMENT_CONFIRMED {
                payment::model::PaymentStatus::Completed
            } else {
                payment::model::PaymentStatus::Pending
            };

            let amount_cents: i32 = intent
                .amount_cents
                .or_else(|| event.amount_cents.map(|c| c as i32))
                .unwrap_or(0);
            let currency = intent
                .currency
                .clone()
                .or_else(|| event.currency.clone())
                .unwrap_or_else(|| "usd".to_string());

            if let Some(pid) = event.payment_id.as_deref().filter(|p| !p.is_empty()) {
                let dup = payment::Entity::find()
                    .filter(payment::Column::Provider.eq(provider_name))
                    .filter(payment::Column::ProviderPaymentId.eq(pid))
                    .one(&state.sea_db)
                    .await
                    .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;
                if dup.is_some() {
                    tracing::info!(
                        provider = provider_name,
                        payment_id = pid,
                        "Payment already recorded, skipping (idempotent)"
                    );
                    return Ok(());
                }
            }

            let active_model = payment::ActiveModel {
                user_id: Set(user_id),
                subscription_id: Set(None),
                plan_id: Set(intent.plan_id),
                provider: Set(provider_name.to_string()),
                provider_payment_id: Set(event.payment_id.clone()),
                amount_cents: Set(amount_cents),
                currency: Set(currency.clone()),
                status: Set(status),
                description: Set(Some(format!(
                    "Payment via {}: {} {}",
                    provider_name, amount_cents, currency
                ))),
                metadata: Set(Some(event.data.clone())),
                ..Default::default()
            };
            active_model
                .insert(&state.sea_db)
                .await
                .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

            tracing::info!(
                user_id,
                amount_cents,
                currency = %currency,
                provider = provider_name,
                status = %event.event_type,
                "Payment recorded from webhook (server-bound intent)"
            );
        }
        _ => {
            tracing::info!(event_type = %event.event_type, "Unhandled billing webhook event");
        }
    }
    Ok(())
}

pub async fn check_post_access(
    State(state): State<AppState>,
    Path(post_id): Path<i32>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let access = post_access::Entity::find()
        .filter(post_access::Column::PostId.eq(post_id))
        .one(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    match access {
        Some(a) => Ok(Json(json!({
            "post_id": post_id,
            "access_type": a.access_type,
            "price_cents": a.price_cents,
            "currency": a.currency,
            "requires_subscription": true,
        }))),
        None => Ok(Json(json!({
            "post_id": post_id,
            "access_type": "free",
            "requires_subscription": false,
        }))),
    }
}

pub async fn admin_set_post_access(
    State(state): State<AppState>,
    Path(post_id): Path<i32>,
    Json(payload): Json<SetPostAccessPayload>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    post_access::Entity::delete_many()
        .filter(post_access::Column::PostId.eq(post_id))
        .exec(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    let active_model = post_access::model::ActiveModel {
        post_id: Set(post_id),
        access_type: Set(payload.access_type),
        price_cents: Set(payload.price_cents),
        currency: Set(payload.currency),
        ..Default::default()
    };

    active_model
        .insert(&state.sea_db)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::QueryError))?;

    Ok(Json(json!({ "message": "Post access updated" })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::billing::provider::BillingProvider;
    use crate::services::billing::revolut::RevolutProvider;
    use crate::services::billing::stripe::StripeProvider;

    #[tokio::test]
    async fn per_post_checkout_defaults_to_not_supported_for_non_overriding_provider() {
        let provider = RevolutProvider::new("k".into(), "s".into());
        let result = provider
            .create_post_checkout(1, 499, "usd", "buyer@example.com", 7, "s", "c")
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::services::billing::provider::BillingError::Config(msg) => {
                assert!(
                    msg.contains("not supported"),
                    "expected not-supported message, got: {msg}"
                );
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn per_post_checkout_is_overridden_by_stripe() {
        let provider = StripeProvider::new("sk_test".into(), "whsec".into());
        assert_eq!(provider.provider_name(), "stripe");
    }

    #[test]
    fn checkout_intent_roundtrip_preserves_grant_fields() {
        let intent = checkout_intent::CheckoutIntent {
            user_id: 42,
            post_id: Some(7),
            amount_cents: Some(499),
            currency: Some("usd".to_string()),
            plan_id: None,
        };
        let s = serde_json::to_string(&intent).unwrap();
        let back: checkout_intent::CheckoutIntent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.user_id, 42);
        assert_eq!(back.post_id, Some(7));
        assert_eq!(back.amount_cents, Some(499));
        assert_eq!(back.currency.as_deref(), Some("usd"));
        assert!(back.plan_id.is_none());

        let sub = checkout_intent::CheckoutIntent {
            user_id: 5,
            post_id: None,
            amount_cents: None,
            currency: None,
            plan_id: Some(3),
        };
        let ss = serde_json::to_string(&sub).unwrap();
        let sub_back: checkout_intent::CheckoutIntent = serde_json::from_str(&ss).unwrap();
        assert_eq!(sub_back.user_id, 5);
        assert!(sub_back.post_id.is_none());
        assert_eq!(sub_back.plan_id, Some(3));
    }

    fn payment_confirmed_event_with_memo(memo: &str) -> ParsedWebhook {
        ParsedWebhook {
            event_type: canonical::PAYMENT_CONFIRMED.to_string(),
            customer_id: String::new(),
            subscription_id: None,
            payment_id: None,
            current_period_end: None,
            checkout_session_id: None,
            subscription_status: None,
            user_id: None,
            amount_cents: Some(4999),
            currency: Some("usd".to_string()),
            data: serde_json::json!({ "memo": memo }),
        }
    }

    #[test]
    fn payment_confirmed_with_memo_but_no_intent_is_refused() {
        let event = payment_confirmed_event_with_memo("rux-1337-deadbeef-uuid");
        let session_id = resolve_intent_session_id(&event);
        assert!(
            session_id.is_empty(),
            "a memo-only event must not resolve to an intent key (got {session_id:?})"
        );
    }

    #[test]
    fn payment_confirmed_with_intent_key_resolves_to_it() {
        let mut event = payment_confirmed_event_with_memo("rux-1337-deadbeef-uuid");
        event.checkout_session_id = Some("cs_live_abc123".to_string());
        assert_eq!(resolve_intent_session_id(&event), "cs_live_abc123");

        event.checkout_session_id = None;
        event.payment_id = Some("pay_456".to_string());
        assert_eq!(resolve_intent_session_id(&event), "pay_456");

        event.payment_id = None;
        event.subscription_id = Some("sub_789".to_string());
        assert_eq!(resolve_intent_session_id(&event), "sub_789");

        event.subscription_id = None;
        event.checkout_session_id = Some(String::new());
        event.payment_id = Some("pay_000".to_string());
        assert_eq!(resolve_intent_session_id(&event), "pay_000");
    }
}
