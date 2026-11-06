//! Ban contract core (A19): one active ban per account, admin create/revoke/renew,
//! session kill on ban, notice survives the account, self-ban and equal-role refusal.
mod reading_common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

use reading_common::{harness, login_cookie, router_with_state, seed_user, Harness};
use rux_request_gate::{AbuseLimiterConfig, GateDecision, RateLimitStore};
use ruxlog::db::sea_models::appeal;
use ruxlog::db::sea_models::audit_log;
use ruxlog::db::sea_models::email_suppression::{self, SuppressionUpsert};
use ruxlog::db::sea_models::enforcement_notice;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::db::sea_models::user_ban;
use ruxlog::middlewares::auth_guard;
use ruxlog::modules::admin_user_bans_v1;
use ruxlog::modules::appeal_v1;
use ruxlog::services::abuse_limiter::{self, Attempt, FailureMode};
use ruxlog::services::mail::provider::{
    MailError, MailProvider, OutboundEmail, SendReceipt, TEMPLATE_ENFORCEMENT_NOTICE,
    TEMPLATE_VERIFICATION,
};
use ruxlog::services::mail::router::{MailRouter, MailRouterLimits};
use ruxlog::AppState;
use ruxlog_types::enums::SuppressionReason;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Set,
    Statement,
};

fn req(method: &str, path: &str, cookie: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie);
    match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

fn expires_in_days(days: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::days(days))
        .fixed_offset()
        .to_rfc3339()
}

fn create_body(user_id: i32, days: i64) -> Value {
    json!({
        "userId": user_id,
        "reasonCode": "spam",
        "reasonText": "repeated spam",
        "corroborator": "credential_reuse",
        "sharedDeviceChecked": true,
        "expiresAt": expires_in_days(days),
    })
}

/// Harness with reader A promoted to Admin. The role is read per request, so
/// the existing cookie stays valid.
async fn admin_harness() -> Harness {
    let h = harness(admin_user_bans_v1::routes()).await;
    promote(&h, h.uid_a, UserRole::Admin).await;
    h
}

/// Set a role through the ORM. The user before-save hook regenerates
/// session_auth_secret whenever it is unset, so carry the current value through
/// to keep existing sessions valid.
async fn promote(h: &Harness, user_id: i32, role: UserRole) {
    let current = user::Entity::find_by_id(user_id)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap();
    user::ActiveModel {
        id: Set(user_id),
        role: Set(role),
        session_auth_secret: Set(current.session_auth_secret),
        ..Default::default()
    }
    .update(&h.state.sea_db)
    .await
    .expect("set role");
}

async fn count(h: &Harness, sql: &str) -> i64 {
    let row = h
        .state
        .sea_db
        .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
        .await
        .unwrap()
        .unwrap();
    row.try_get_by_index(0).unwrap()
}

#[tokio::test]
async fn second_active_ban_is_409_with_existing_id() {
    let h = admin_harness().await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "first ban: {body}");
    let first_id = body["banId"].as_i64().unwrap();

    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 2))),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "second ban: {body}");
    assert_eq!(body["context"]["existingBanId"].as_i64(), Some(first_id));
}

#[tokio::test]
async fn revoked_ban_frees_the_slot_for_a_new_one() {
    let h = admin_harness().await;
    let (_, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let ban_id = body["banId"].as_i64().unwrap();
    let (status, _) = send(
        &h.app,
        req("POST", &format!("/{ban_id}/revoke"), &h.cookie_a, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "re-ban after revoke: {body}");
}

#[tokio::test]
async fn active_ban_lookup_plan_uses_partial_index() {
    let h = harness(admin_user_bans_v1::routes()).await;
    let rows = h
        .state
        .sea_db
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "EXPLAIN QUERY PLAN SELECT id FROM user_bans WHERE user_id = 1 \
             AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > '2030-01-01') \
             ORDER BY created_at DESC LIMIT 1"
                .to_string(),
        ))
        .await
        .unwrap();
    let plan: Vec<String> = rows
        .iter()
        .map(|row| row.try_get_by_index::<String>(3).unwrap())
        .collect();
    // Either the A30/A32 partial index or the A32 (user_id, revoked_at) index is a seek.
    // The planner may pick the latter; the guard is that no full table scan happens.
    assert!(
        plan.iter().any(|d| d.contains("SEARCH user_bans")),
        "plan does not seek user_bans: {plan:?}"
    );
    assert!(
        !plan.iter().any(|d| d.starts_with("SCAN user_bans")),
        "plan scans user_bans: {plan:?}"
    );
}

#[tokio::test]
async fn notice_survives_account_deletion() {
    let h = admin_harness().await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(body["noticeId"].as_i64().is_some());

    h.state
        .sea_db
        .execute_unprepared(&format!("DELETE FROM users WHERE id = {}", h.uid_b))
        .await
        .unwrap();

    let notices = enforcement_notice::Entity::find()
        .all(&h.state.sea_db)
        .await
        .unwrap();
    assert_eq!(notices.len(), 1, "notice row must outlive the account");
    assert_eq!(notices[0].user_id, h.uid_b);
    assert_eq!(notices[0].corroborator, "credential_reuse");
    assert_eq!(notices[0].retention_class, "enforcement_12m");
    // No mail backend in this harness: the attempt is recorded, never as sent.
    assert_eq!(notices[0].delivery_state.as_deref(), Some("send_failed"));
}

#[tokio::test]
async fn self_ban_is_refused() {
    let h = admin_harness().await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_a, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "self-ban: {body}");
    assert_eq!(count(&h, "SELECT COUNT(*) FROM user_bans").await, 0);
}

#[tokio::test]
async fn equal_or_higher_role_is_refused() {
    let h = admin_harness().await;
    let peer = seed_user(&h.state, "peeradmin").await;
    promote(&h, peer.id, UserRole::Admin).await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(peer.id, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "peer admin ban: {body}");
}

#[tokio::test]
async fn non_admin_cannot_create_a_ban() {
    let h = harness(admin_user_bans_v1::routes()).await;
    let (status, _) = send(
        &h.app,
        req("POST", "/", &h.cookie_b, Some(create_body(h.uid_a, 1))),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED,
        "non-admin got {status}"
    );
}

#[tokio::test]
async fn expiry_is_required_and_bounded() {
    let h = admin_harness().await;
    let mut no_expiry = create_body(h.uid_b, 1);
    no_expiry.as_object_mut().unwrap().remove("expiresAt");
    let (status, _) = send(&h.app, req("POST", "/", &h.cookie_a, Some(no_expiry))).await;
    assert!(status.is_client_error(), "missing expiresAt: {status}");

    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 400))),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "400-day ban: {body}");
    assert_eq!(count(&h, "SELECT COUNT(*) FROM user_bans").await, 0);
}

#[tokio::test]
async fn shared_device_check_is_required_on_create() {
    let h = admin_harness().await;
    let mut missing = create_body(h.uid_b, 1);
    missing
        .as_object_mut()
        .unwrap()
        .remove("sharedDeviceChecked");
    let (status, body) = send(&h.app, req("POST", "/", &h.cookie_a, Some(missing))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing flag: {body}");

    let mut unchecked = create_body(h.uid_b, 1);
    unchecked["sharedDeviceChecked"] = json!(false);
    let (status, body) = send(&h.app, req("POST", "/", &h.cookie_a, Some(unchecked))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "false flag: {body}");
    assert_eq!(count(&h, "SELECT COUNT(*) FROM user_bans").await, 0);
    assert_eq!(
        count(&h, "SELECT COUNT(*) FROM enforcement_notices").await,
        0
    );
}

#[tokio::test]
async fn ban_decision_records_the_shared_device_check() {
    let h = admin_harness().await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(
        count(
            &h,
            "SELECT shared_device_checked FROM enforcement_notices ORDER BY id DESC LIMIT 1"
        )
        .await,
        1
    );
}

/// A device signal is advisory: the precedence rule resolves it to a challenge,
/// never to an account ban. The admin route is the only path that writes a ban.
#[test]
fn device_signal_alone_resolves_to_challenge_not_ban() {
    use ruxlog::modules::admin_user_bans_v1::precedence::{resolve, Outcome, Signals};
    let outcome = resolve(Signals {
        device_signal: true,
        ..Signals::default()
    });
    assert_eq!(outcome, Outcome::Challenge);
}

/// The SOP is the operator's checklist for the multi-account decision (A22). The
/// guard keeps each required step in the committed file.
#[test]
fn moderation_sop_lists_the_multi_account_checklist() {
    let sop = include_str!("../../../../deploy/moderation-sop.md").to_lowercase();
    for step in [
        "prior case link",
        "functional test",
        "causal test",
        "admissible corroborators",
        "shared-device check",
        "continuity",
        "closed reason codes",
    ] {
        assert!(sop.contains(step), "SOP missing step: {step}");
    }
}

#[tokio::test]
async fn corroborator_is_required() {
    let h = admin_harness().await;
    let mut body = create_body(h.uid_b, 1);
    body.as_object_mut().unwrap().remove("corroborator");
    let (status, _) = send(&h.app, req("POST", "/", &h.cookie_a, Some(body))).await;
    assert!(status.is_client_error(), "missing corroborator: {status}");
}

#[tokio::test]
async fn live_session_is_rejected_after_ban() {
    let h = admin_harness().await;
    // A guard-protected route that only needs a signed-in, non-banned session.
    let probe = Router::new()
        .route("/me", get(|| async { "ok" }))
        .route_layer(middleware::from_fn(auth_guard::authenticated));
    let probe_app = router_with_state(h.state.clone(), probe);

    let (status, _) = send(&probe_app, req("GET", "/me", &h.cookie_b, None)).await;
    assert_eq!(status, StatusCode::OK, "session valid before ban");

    let (_, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let ban_id = body["banId"].as_i64().unwrap();
    // Lift the ban, then check the old session: only the secret rotation can reject it.
    let (status, _) = send(
        &h.app,
        req("POST", &format!("/{ban_id}/revoke"), &h.cookie_a, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(&probe_app, req("GET", "/me", &h.cookie_b, None)).await;
    assert_ne!(
        status,
        StatusCode::OK,
        "old session must not survive the ban"
    );
}

#[tokio::test]
async fn renew_needs_self_review_and_extends_only() {
    let h = admin_harness().await;
    let (_, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 10))),
    )
    .await;
    let ban_id = body["banId"].as_i64().unwrap();

    let no_review = json!({
        "expiresAt": expires_in_days(20),
        "singleOperatorControl": false,
        "selfReviewNote": "checked",
    });
    let (status, _) = send(
        &h.app,
        req(
            "POST",
            &format!("/{ban_id}/renew"),
            &h.cookie_a,
            Some(no_review),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let shorter = json!({
        "expiresAt": expires_in_days(5),
        "singleOperatorControl": true,
        "selfReviewNote": "checked",
    });
    let (status, _) = send(
        &h.app,
        req(
            "POST",
            &format!("/{ban_id}/renew"),
            &h.cookie_a,
            Some(shorter),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "renew must extend");

    let longer = json!({
        "expiresAt": expires_in_days(20),
        "singleOperatorControl": true,
        "selfReviewNote": "reviewed evidence again",
    });
    let (status, body) = send(
        &h.app,
        req(
            "POST",
            &format!("/{ban_id}/renew"),
            &h.cookie_a,
            Some(longer),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "renew: {body}");
    assert_eq!(
        count(
            &h,
            "SELECT COUNT(*) FROM audit_logs WHERE action = 'ban.renew'"
        )
        .await,
        1
    );
}

/// Body of the last message the recording provider was handed. Tests read the
/// appeal token from it, the only place the raw token exists.
static LAST_HTML: Mutex<Option<String>> = Mutex::new(None);

/// Provider double for notice delivery (A21). Counts sends and reports either an
/// accepted message or an `Ok` that accepted nothing.
struct RecordingProvider {
    accept: bool,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl MailProvider for RecordingProvider {
    fn provider_name(&self) -> &'static str {
        "test"
    }

    async fn send(&self, msg: OutboundEmail) -> Result<SendReceipt, MailError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *LAST_HTML.lock().unwrap() = msg.html.clone();
        Ok(SendReceipt {
            delivered: u32::from(self.accept),
            ..SendReceipt::default()
        })
    }
}

/// Admin harness whose mailer routes to a recording provider. Same DB and session
/// store, so the existing admin cookie still works.
async fn admin_with_provider(accept: bool) -> (Harness, Arc<AtomicUsize>) {
    let h = admin_harness().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let providers: HashMap<String, Arc<dyn MailProvider>> = [(
        "test".to_string(),
        Arc::new(RecordingProvider {
            accept,
            calls: calls.clone(),
        }) as Arc<dyn MailProvider>,
    )]
    .into_iter()
    .collect();
    let mailer = MailRouter::new(
        providers,
        "test".to_string(),
        h.state.gate_store.clone(),
        h.state.sea_db.clone(),
        MailRouterLimits::default(),
        false,
    );
    let state = AppState {
        mailer: Arc::new(mailer),
        ..h.state.clone()
    };
    let app = router_with_state(state.clone(), admin_user_bans_v1::routes());
    (Harness { app, state, ..h }, calls)
}

async fn notice_row_state(h: &Harness) -> (Option<String>, Option<i64>, String) {
    let row = h
        .state
        .sea_db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT delivery_state, sent_at, notice_channel FROM enforcement_notices \
             ORDER BY id DESC LIMIT 1"
                .to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    (
        row.try_get_by_index(0).unwrap(),
        row.try_get_by_index(1).unwrap(),
        row.try_get_by_index(2).unwrap(),
    )
}

#[tokio::test]
async fn notice_is_sent_only_when_the_provider_accepts() {
    let (h, calls) = admin_with_provider(true).await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(body["deliveryState"], "sent");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (state, sent_at, channel) = notice_row_state(&h).await;
    assert_eq!(state.as_deref(), Some("sent"));
    assert!(sent_at.is_some(), "sent_at set on sent");
    assert_eq!(channel, "email");
}

#[tokio::test]
async fn ok_that_accepted_nothing_is_send_failed_not_sent() {
    let (h, calls) = admin_with_provider(false).await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(body["deliveryState"], "send_failed");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (state, sent_at, _) = notice_row_state(&h).await;
    assert_eq!(state.as_deref(), Some("send_failed"));
    assert!(sent_at.is_none(), "no sent_at without delivery");
}

#[tokio::test]
async fn no_mail_backend_records_send_failed() {
    let h = admin_harness().await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(body["deliveryState"], "send_failed");
}

#[tokio::test]
async fn bounced_address_records_suppressed_and_sends_nothing() {
    let (h, calls) = admin_with_provider(true).await;
    let email = user::Entity::find_by_id(h.uid_b)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap()
        .email;
    email_suppression::Entity::upsert(
        &h.state.sea_db,
        &email,
        SuppressionUpsert {
            reason: SuppressionReason::Bounce,
            source: Some("test".to_string()),
            diagnostic: None,
            permanent: true,
            blocks_transactional: true,
        },
    )
    .await
    .unwrap();
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(body["deliveryState"], "suppressed");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "no provider call");
    let (state, sent_at, _) = notice_row_state(&h).await;
    assert_eq!(state.as_deref(), Some("suppressed"));
    assert!(sent_at.is_none());
}

#[tokio::test]
async fn empty_contact_records_no_contact_details() {
    let (h, calls) = admin_with_provider(true).await;
    let current = user::Entity::find_by_id(h.uid_b)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap();
    user::ActiveModel {
        id: Set(h.uid_b),
        email: Set(String::new()),
        session_auth_secret: Set(current.session_auth_secret),
        ..Default::default()
    }
    .update(&h.state.sea_db)
    .await
    .expect("blank email");
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(body["deliveryState"], "no_contact_details");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (state, _, _) = notice_row_state(&h).await;
    assert_eq!(state.as_deref(), Some("no_contact_details"));
}

#[tokio::test]
async fn resend_records_the_new_attempt() {
    let (h, calls) = admin_with_provider(true).await;
    let (_, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let ban_id = body["banId"].as_i64().unwrap();
    let (status, body) = send(
        &h.app,
        req(
            "POST",
            &format!("/{ban_id}/notice/resend"),
            &h.cookie_a,
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "resend: {body}");
    assert_eq!(body["deliveryState"], "sent");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "create + resend");
    let (state, _, _) = notice_row_state(&h).await;
    assert_eq!(state.as_deref(), Some("sent"));
    let audited = audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq("ban.notice_resend"))
        .all(&h.state.sea_db)
        .await
        .unwrap();
    assert_eq!(audited.len(), 1, "resend writes one audit row");
    assert_eq!(audited[0].resource_id, ban_id.to_string());
    assert_eq!(audited[0].user_id, Some(h.uid_a));
}

#[tokio::test]
async fn complaint_suppression_sends_enforcement_but_drops_verification() {
    let (h, calls) = admin_with_provider(true).await;
    let address = "complained@example.test";
    email_suppression::Entity::upsert(
        &h.state.sea_db,
        address,
        SuppressionUpsert {
            reason: SuppressionReason::Complaint,
            source: Some("test".to_string()),
            diagnostic: None,
            permanent: true,
            blocks_transactional: true,
        },
    )
    .await
    .unwrap();
    let router = h.state.mailer.clone();
    let verification = OutboundEmail {
        to: address.to_string(),
        subject: "Email verification code".to_string(),
        html: None,
        text: None,
        template: Some(TEMPLATE_VERIFICATION),
        list_unsubscribe: None,
    };
    let receipt = router.send(verification).await.expect("verification is Ok");
    assert!(receipt.suppressed, "verification dropped by the router");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "verification never reaches a provider"
    );

    let notice = OutboundEmail {
        to: address.to_string(),
        subject: "Notice".to_string(),
        html: None,
        text: None,
        template: Some(TEMPLATE_ENFORCEMENT_NOTICE),
        list_unsubscribe: None,
    };
    let receipt = router.send(notice).await.expect("enforcement is Ok");
    assert!(
        receipt.accepted_by_provider(),
        "enforcement reaches the provider"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn lookup_error_fails_closed_for_enforcement_only() {
    let (h, calls) = admin_with_provider(true).await;
    // Dropping the suppression table makes the lookup fail with a DB error.
    h.state
        .sea_db
        .execute_unprepared("DROP TABLE email_suppression")
        .await
        .unwrap();
    let router = h.state.mailer.clone();
    let enforcement = OutboundEmail {
        to: "any@example.test".to_string(),
        subject: "Notice".to_string(),
        html: None,
        text: None,
        template: Some(TEMPLATE_ENFORCEMENT_NOTICE),
        list_unsubscribe: None,
    };
    let receipt = router.send(enforcement).await.expect("Ok, not sent");
    assert!(receipt.suppressed, "enforcement fails closed");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

// ---- Appeal intake (A35) -------------------------------------------------

/// Raw token from the last notice mail. The mail is the only place it exists.
fn token_from_last_mail() -> String {
    let html = LAST_HTML
        .lock()
        .unwrap()
        .clone()
        .expect("notice body captured");
    let start = html.find("/appeal/").expect("appeal link in notice") + "/appeal/".len();
    html[start..start + appeal_v1::controller::TOKEN_HEX_LEN].to_string()
}

fn intake_req(token: &str, statement: &str) -> Request<Body> {
    req(
        "POST",
        "/submit",
        "",
        Some(json!({ "token": token, "statement": statement })),
    )
}

#[tokio::test]
async fn notice_token_opens_intake_and_is_stored_only_as_a_hash() {
    let (h, _calls) = admin_with_provider(true).await;
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    let token = token_from_last_mail();
    assert_eq!(token.len(), 64, "256-bit token, hex");

    let stored = enforcement_notice::Entity::find()
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap()
        .appeal_token_hash
        .expect("hash stored");
    assert_eq!(
        stored,
        appeal_v1::controller::token_hash(&h.state.secret_key, &token)
    );
    assert_ne!(stored, token, "raw token never stored");

    let intake = router_with_state(h.state.clone(), appeal_v1::routes());
    let (status, body) = send(&intake, intake_req(&token, "I was not spamming")).await;
    assert_eq!(status, StatusCode::ACCEPTED, "intake: {body}");
    assert_eq!(body["received"], true);
    assert_eq!(count(&h, "SELECT COUNT(*) FROM appeals").await, 1);
    assert_eq!(
        count(&h, "SELECT due_at - submitted_at FROM appeals").await,
        appeal::DECISION_CLOCK_SECS,
        "decision due 7 days after submit"
    );
}

#[tokio::test]
async fn unknown_known_and_malformed_tokens_get_the_same_answer() {
    let (h, _calls) = admin_with_provider(true).await;
    send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let token = token_from_last_mail();
    let intake = router_with_state(h.state.clone(), appeal_v1::routes());

    let known = send(&intake, intake_req(&token, "appeal text")).await;
    let unknown = send(&intake, intake_req(&"f".repeat(64), "appeal text")).await;
    let malformed = send(&intake, intake_req("not-a-token", "appeal text")).await;
    assert_eq!(known, unknown, "known and unknown answer identically");
    assert_eq!(known, malformed, "malformed answers identically");
    assert_eq!(count(&h, "SELECT COUNT(*) FROM appeals").await, 1);
}

#[tokio::test]
async fn repeat_submit_for_one_notice_writes_nothing_new() {
    let (h, _calls) = admin_with_provider(true).await;
    send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let token = token_from_last_mail();
    let intake = router_with_state(h.state.clone(), appeal_v1::routes());
    let first = send(&intake, intake_req(&token, "first")).await;
    let second = send(&intake, intake_req(&token, "second")).await;
    assert_eq!(first, second);
    assert_eq!(count(&h, "SELECT COUNT(*) FROM appeals").await, 1);
}

#[tokio::test]
async fn empty_statement_is_refused() {
    let (h, _calls) = admin_with_provider(true).await;
    let intake = router_with_state(h.state.clone(), appeal_v1::routes());
    let (status, _) = send(&intake, intake_req(&"a".repeat(64), "   ")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn resend_retires_the_earlier_appeal_link() {
    let (h, _calls) = admin_with_provider(true).await;
    let (_, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let old_token = token_from_last_mail();
    let ban_id = body["banId"].as_i64().unwrap();
    let (status, _) = send(
        &h.app,
        req(
            "POST",
            &format!("/{ban_id}/notice/resend"),
            &h.cookie_a,
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let new_token = token_from_last_mail();
    assert_ne!(old_token, new_token);

    let intake = router_with_state(h.state.clone(), appeal_v1::routes());
    send(&intake, intake_req(&old_token, "old link")).await;
    assert_eq!(count(&h, "SELECT COUNT(*) FROM appeals").await, 0);
    send(&intake, intake_req(&new_token, "new link")).await;
    assert_eq!(count(&h, "SELECT COUNT(*) FROM appeals").await, 1);
}

#[tokio::test]
async fn appeal_clock_alert_fires_only_past_the_decision_clock() {
    let (h, _calls) = admin_with_provider(true).await;
    send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    let token = token_from_last_mail();
    let intake = router_with_state(h.state.clone(), appeal_v1::routes());
    send(&intake, intake_req(&token, "appeal text")).await;

    let now = chrono::Utc::now().timestamp();
    let before = appeal_v1::controller::check_appeal_clock(&h.state.sea_db, now)
        .await
        .unwrap();
    assert_eq!(before, 0, "inside the 7-day clock: no alert");
    let after = appeal_v1::controller::check_appeal_clock(
        &h.state.sea_db,
        now + appeal::DECISION_CLOCK_SECS + 1,
    )
    .await
    .unwrap();
    assert_eq!(after, 1, "past the clock: alert counts the open appeal");
}

// Source-scan guards (house pattern): the ban contract writes no DELETE and
// adds no ON DELETE CASCADE from users.
#[test]
fn ban_contract_source_has_no_delete_or_user_cascade() {
    let sources = [
        include_str!("../migration/src/m000030_user_ban_contract.rs"),
        include_str!("../src/db/sea_models/enforcement_notice/actions.rs"),
        include_str!("../src/db/sea_models/enforcement_notice/model.rs"),
        include_str!("../src/modules/admin_user_bans_v1/controller.rs"),
        include_str!("../src/modules/admin_user_bans_v1/notice.rs"),
        include_str!("../src/modules/appeal_v1/controller.rs"),
    ];
    for src in sources {
        assert!(
            !src.to_ascii_uppercase().contains("DELETE FROM"),
            "ban contract source must not delete rows"
        );
        assert!(
            !src.to_ascii_uppercase().contains("ON DELETE CASCADE"),
            "no ON DELETE CASCADE in ban contract source"
        );
    }
    // The gate itself defaults off.
    assert_eq!(admin_user_bans_v1::controller::MAX_BAN_DAYS, 365);
}

#[tokio::test]
async fn ban_history_survives_account_deletion() {
    let h = admin_harness().await;
    let (status, _) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    h.state
        .sea_db
        .execute_unprepared(&format!("DELETE FROM users WHERE id = {}", h.uid_b))
        .await
        .unwrap();
    assert_eq!(
        count(
            &h,
            &format!("SELECT COUNT(*) FROM user_bans WHERE user_id = {}", h.uid_b)
        )
        .await,
        1,
        "ban row must outlive the account (A19b)"
    );
}

// A19b source guard: the rebuilt user_bans migration (non-test part) and the
// entity carry no ON DELETE CASCADE / Cascade relation from users.
#[test]
fn user_bans_has_no_user_cascade() {
    let migration = include_str!("../migration/src/m000032_user_bans_evidence_safe.rs");
    let migration_body = migration.split("#[cfg(test)]").next().unwrap_or_default();
    assert!(
        !migration_body
            .to_ascii_uppercase()
            .contains("ON DELETE CASCADE"),
        "m000032 must not add ON DELETE CASCADE"
    );
    let model = include_str!("../src/db/sea_models/user_ban/model.rs");
    assert!(
        !model.contains("on_delete = \"Cascade\""),
        "user_ban entity must not declare a cascade on delete"
    );
}

/// Insert an active ban row directly. Unlike the admin route this leaves the
/// session secret alone, so only the ban check can reject a live session.
async fn ban_directly(h: &Harness, user_id: i32) {
    user_ban::ActiveModel {
        user_id: Set(user_id),
        reason: Set(Some("spam".to_string())),
        banned_by: Set(None),
        expires_at: Set(None),
        created_at: Set(chrono::Utc::now().fixed_offset()),
        revoked_at: Set(None),
        revoked_by: Set(None),
        ..Default::default()
    }
    .insert(&h.state.sea_db)
    .await
    .expect("ban inserted");
}

async fn unverify(h: &Harness, user_id: i32) {
    let current = user::Entity::find_by_id(user_id)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap();
    user::ActiveModel {
        id: Set(user_id),
        is_verified: Set(false),
        session_auth_secret: Set(current.session_auth_secret),
        ..Default::default()
    }
    .update(&h.state.sea_db)
    .await
    .expect("unverify");
}

/// One probe router per guard class. The flag marks the guard that only admits
/// unverified accounts, so its probe needs an unverified session.
fn guard_probes() -> Vec<(&'static str, Router<AppState>, bool)> {
    fn ok_route() -> Router<AppState> {
        Router::new().route("/me", get(|| async { "ok" }))
    }
    vec![
        (
            "authenticated",
            ok_route().route_layer(middleware::from_fn(auth_guard::authenticated)),
            false,
        ),
        (
            "unverified",
            ok_route().route_layer(middleware::from_fn(auth_guard::unverified)),
            true,
        ),
        (
            "verified",
            ok_route().route_layer(middleware::from_fn(auth_guard::verified)),
            false,
        ),
        (
            "verified_with_role",
            ok_route().route_layer(middleware::from_fn(
                auth_guard::verified_with_role::<{ auth_guard::ROLE_USER }>,
            )),
            false,
        ),
    ]
}

// A20: every guard class rejects a banned session. Each probe gets a fresh
// session that has not been checked for bans yet (the guard caches a clean
// ban check for 5 minutes), and the ban is inserted without rotating the
// secret, so the ban check itself must reject.
#[tokio::test]
async fn banned_session_is_rejected_by_every_guard() {
    let h = admin_harness().await;
    let unverified = seed_user(&h.state, "unverifiedguard").await;
    unverify(&h, unverified.id).await;
    let verified_b = user::Entity::find_by_id(h.uid_b)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap();

    // Baseline: every probe admits a healthy session.
    for (name, probe, needs_unverified) in guard_probes() {
        let app = router_with_state(h.state.clone(), probe);
        let cookie = if needs_unverified {
            login_cookie(&h.state, &unverified).await
        } else {
            login_cookie(&h.state, &verified_b).await
        };
        let (status, body) = send(&app, req("GET", "/me", &cookie, None)).await;
        assert_eq!(status, StatusCode::OK, "{name}: valid before ban: {body}");
    }

    ban_directly(&h, h.uid_b).await;
    ban_directly(&h, unverified.id).await;

    for (name, probe, needs_unverified) in guard_probes() {
        let app = router_with_state(h.state.clone(), probe);
        let cookie = if needs_unverified {
            login_cookie(&h.state, &unverified).await
        } else {
            login_cookie(&h.state, &verified_b).await
        };
        let (status, body) = send(&app, req("GET", "/me", &cookie, None)).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{name} must reject a banned session: {body}"
        );
    }
}

// Same thresholds as the auth controller's ABUSE_LIMITER_CONFIG.
const LOGIN_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 3,
    temp_block_range: 360,
    temp_block_duration: 3600,
    block_retry_limit: 5,
    block_range: 900,
    block_duration: 86400,
};

// The limiter is a no-op outside production (dev_bypass), so this test turns
// enforcement on the same way login_limiter.rs does.
fn enforce_limiter() {
    std::env::set_var("APP_ENV", "production");
    std::env::set_var(
        "VERIFICATION_IDENTITY_KEY",
        "0123456789abcdef0123456789abcdef",
    );
}

// A20: a new ban lifts the account's login throttle, so the next attempt gets
// the banned response rather than a stale delay.
#[tokio::test]
async fn creating_a_ban_lifts_the_login_throttle() {
    enforce_limiter();
    let h = admin_harness().await;
    let key = "login:readerb@example.com";
    let store = &h.state.gate_store;
    for _ in 0..3 {
        abuse_limiter::settle(
            store,
            key,
            LOGIN_CFG,
            FailureMode::Credential,
            Attempt::Rejected,
        )
        .await;
    }
    assert!(
        matches!(
            store
                .abuse_peek(key, LOGIN_CFG, FailureMode::Credential)
                .await
                .unwrap(),
            GateDecision::Delay { .. }
        ),
        "three failures must delay the login key"
    );
    let (status, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "ban: {body}");
    assert_eq!(
        store
            .abuse_peek(key, LOGIN_CFG, FailureMode::Credential)
            .await
            .unwrap(),
        GateDecision::Pass,
        "ban must clear the login throttle"
    );
}
