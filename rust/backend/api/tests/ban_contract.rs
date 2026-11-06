//! Ban contract core (A19): one active ban per account, admin create/revoke/renew,
//! session kill on ban, notice survives the account, self-ban and equal-role refusal.
mod reading_common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

use reading_common::{harness, router_with_state, seed_user, Harness};
use ruxlog::db::sea_models::enforcement_notice;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::middlewares::auth_guard;
use ruxlog::modules::admin_user_bans_v1;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, EntityTrait, Set, Statement};

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
    assert!(
        plan.iter().any(|d| d.contains("idx_user_bans_one_active")),
        "plan does not use the partial index: {plan:?}"
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
    assert!(notices[0].delivery_state.is_none());
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

#[tokio::test]
async fn resend_refuses_until_delivery_lands() {
    let h = admin_harness().await;
    let (_, body) = send(
        &h.app,
        req("POST", "/", &h.cookie_a, Some(create_body(h.uid_b, 1))),
    )
    .await;
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
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
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

