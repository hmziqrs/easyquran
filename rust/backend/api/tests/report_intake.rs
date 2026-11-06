//! Incident intake (A24): one report row and one audit row per submit, closed
//! reason set, per-IP rate limit, P0 snapshot at intake, no IP or device column.
mod reading_common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::{Extension, Router};
use axum_client_ip::ClientIpSource;
use chrono::Utc;
use reading_common::{harness, Harness};
use rux_request_gate::{LimiterDecision, NoHooks};
use ruxlog::db::sea_models::{audit_log, category, post, post_comment, report};
use ruxlog::middlewares::client_ip::resolve_client_ip;
use ruxlog::modules::report_v1;
use ruxlog::AppState;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use tower::ServiceExt;

/// Report routes behind the same client-IP resolution the production stack uses,
/// read from `cf-connecting-ip` so each test picks its own source address.
fn routes() -> Router<AppState> {
    report_v1::routes()
        .layer(middleware::from_fn(resolve_client_ip))
        .layer(Extension(ClientIpSource::CfConnectingIp))
}

async fn submit(app: &Router, ip: &str, cookie: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/submit")
        .header("content-type", "application/json")
        .header("cf-connecting-ip", ip);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let req = req.body(Body::from(body.to_string())).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn all_reports(db: &DatabaseConnection) -> Vec<report::Model> {
    report::Entity::find().all(db).await.unwrap()
}

async fn report_audit_rows(db: &DatabaseConnection) -> Vec<audit_log::model::Model> {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq("report.submit"))
        .all(db)
        .await
        .unwrap()
}

async fn seed_comment(h: &Harness, content: &str) -> i32 {
    let now = Utc::now().fixed_offset();
    let cat = category::ActiveModel {
        name: Set("Report test".into()),
        slug: Set("report-test".into()),
        color: Set("#ffffff".into()),
        text_color: Set("#000000".into()),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&h.state.sea_db)
    .await
    .expect("category seeded");
    let parent = post::ActiveModel {
        title: Set("Report test post".into()),
        slug: Set("report-test-post".into()),
        content: Set(json!({"time": 0, "blocks": [], "version": "2.30.7"})),
        author_id: Set(h.uid_a),
        category_id: Set(cat.id),
        status: Set(post::PostStatus::Published),
        view_count: Set(0),
        likes_count: Set(0),
        tag_ids: Set(post::TagIds(vec![])),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&h.state.sea_db)
    .await
    .expect("post seeded");
    let row = post_comment::ActiveModel {
        post_id: Set(parent.id),
        user_id: Set(h.uid_a),
        content: Set(content.to_string()),
        likes_count: Set(0),
        hidden: Set(false),
        flags_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&h.state.sea_db)
    .await
    .expect("comment seeded");
    row.id
}

#[tokio::test]
async fn anonymous_submit_writes_one_report_and_one_audit_row() {
    let h = harness(routes()).await;
    let (status, body) = submit(
        &h.app,
        "203.0.113.10",
        None,
        json!({
            "target_kind": "post",
            "target_id": "5",
            "reason": "spam",
            "details": "  repeated links  ",
            "contact_email": "Reporter@Example.com"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let rows = all_reports(&h.state.sea_db).await;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.reporter_user_id, None, "anonymous reporter");
    assert_eq!(row.target_kind, "post");
    assert_eq!(row.target_id, "5");
    assert_eq!(row.reason, "spam");
    assert_eq!(row.priority, "p1");
    assert_eq!(row.status, "open");
    assert_eq!(row.details.as_deref(), Some("repeated links"), "trimmed");
    assert_eq!(
        row.contact_email.as_deref(),
        Some("reporter@example.com"),
        "lowercased"
    );
    assert_eq!(row.snapshot, None, "non-P0 takes no snapshot");
    assert_eq!(row.retention_class, "report_12m");
    let year = 365 * 24 * 60 * 60;
    let now = Utc::now().timestamp();
    assert!(
        (row.expires_at - (now + year)).abs() < 60,
        "12-month expiry from intake"
    );

    let audit = report_audit_rows(&h.state.sea_db).await;
    assert_eq!(audit.len(), 1, "one report.submit row");
    assert_eq!(audit[0].user_id, None);
    assert_eq!(audit[0].resource_type, "report");
    assert_eq!(audit[0].resource_id, row.id.to_string());
    assert_eq!(audit[0].retention_class, "audit_12m");
    // Metadata carries only the closed fields, never details or the contact address.
    let meta = audit[0].metadata.clone().expect("metadata");
    assert_eq!(meta["reason"], "spam");
    assert_eq!(meta["target_kind"], "post");
    assert_eq!(meta["priority"], "p1");
    let text = meta.to_string();
    assert!(!text.contains("repeated links"));
    assert!(!text.contains("reporter@example.com"));
}

#[tokio::test]
async fn signed_in_submit_records_the_reporter_on_both_rows() {
    let h = harness(routes()).await;
    let (status, body) = submit(
        &h.app,
        "203.0.113.11",
        Some(&h.cookie_a),
        json!({ "target_kind": "comment", "target_id": "9", "reason": "harassment" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let rows = all_reports(&h.state.sea_db).await;
    assert_eq!(rows[0].reporter_user_id, Some(h.uid_a));
    let audit = report_audit_rows(&h.state.sea_db).await;
    assert_eq!(audit[0].user_id, Some(h.uid_a));
}

#[tokio::test]
async fn invalid_payloads_are_refused_and_write_nothing() {
    let h = harness(routes()).await;
    let cases = [
        json!({ "target_kind": "user", "target_id": "5", "reason": "spam" }),
        json!({ "target_kind": "post", "target_id": "5", "reason": "bogus" }),
        json!({ "target_kind": "post", "target_id": "abc", "reason": "spam" }),
        json!({ "target_kind": "post", "target_id": "0", "reason": "spam" }),
        json!({ "target_kind": "post", "target_id": "5", "reason": "spam", "contact_email": "not-an-email" }),
        json!({ "target_kind": "post", "target_id": "5", "reason": "spam", "details": "x".repeat(2001) }),
    ];
    for (i, body) in cases.into_iter().enumerate() {
        let (status, _) = submit(&h.app, "203.0.113.12", None, body).await;
        assert!(
            status.is_client_error(),
            "case {i} should be refused, got {status}"
        );
    }
    assert!(all_reports(&h.state.sea_db).await.is_empty(), "no rows");
    assert!(
        report_audit_rows(&h.state.sea_db).await.is_empty(),
        "no audit rows"
    );
}

// The HTTP wrapper skips the limiter outside production, so this drives the
// same config through the gate directly: 5 an hour, then blocked, per IP.
#[tokio::test]
async fn rate_limit_refuses_the_sixth_report_from_one_ip_only() {
    let h = harness(routes()).await;
    let key = "report:submit:ip:203.0.113.20";
    for i in 0..5 {
        let d = rux_request_gate::check(
            &h.state.gate_store,
            &NoHooks,
            key,
            report_v1::controller::report_limit_config(),
        )
        .await
        .expect("gate check");
        assert!(matches!(d, LimiterDecision::Allowed { .. }), "attempt {i}");
    }
    let d = rux_request_gate::check(
        &h.state.gate_store,
        &NoHooks,
        key,
        report_v1::controller::report_limit_config(),
    )
    .await
    .expect("gate check");
    assert!(
        matches!(d, LimiterDecision::Blocked { .. }),
        "sixth report from one IP blocked"
    );

    let other = rux_request_gate::check(
        &h.state.gate_store,
        &NoHooks,
        "report:submit:ip:203.0.113.21",
        report_v1::controller::report_limit_config(),
    )
    .await
    .expect("gate check");
    assert!(
        matches!(other, LimiterDecision::Allowed { .. }),
        "other IP unaffected"
    );
}

#[tokio::test]
async fn p0_report_captures_the_comment_at_intake() {
    let h = harness(routes()).await;
    let comment_id = seed_comment(&h, "original words").await;

    let (status, body) = submit(
        &h.app,
        "203.0.113.30",
        None,
        json!({
            "target_kind": "comment",
            "target_id": comment_id.to_string(),
            "reason": "threat"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let rows = all_reports(&h.state.sea_db).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].priority, "p0");
    let snap = rows[0].snapshot.clone().expect("P0 snapshot written");
    assert_eq!(snap["kind"], "comment");
    assert_eq!(snap["id"], comment_id);
    assert_eq!(snap["content"], "original words");
    assert_eq!(snap["user_id"], h.uid_a);
}

#[tokio::test]
async fn p0_report_for_a_missing_target_is_still_recorded_without_snapshot() {
    let h = harness(routes()).await;
    let (status, body) = submit(
        &h.app,
        "203.0.113.31",
        None,
        json!({ "target_kind": "post", "target_id": "99999", "reason": "csam" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let rows = all_reports(&h.state.sea_db).await;
    assert_eq!(rows[0].priority, "p0");
    assert_eq!(rows[0].snapshot, None);
}

#[tokio::test]
async fn report_table_has_no_fk_no_ip_and_expiry_columns() {
    let h = harness(routes()).await;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let count = |sql: &'static str| {
        let db = h.state.sea_db.clone();
        async move {
            let row = db
                .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
                .await
                .unwrap()
                .unwrap();
            row.try_get_by_index::<i64>(0).unwrap()
        }
    };
    assert_eq!(
        count("SELECT COUNT(*) FROM pragma_foreign_key_list('reports')").await,
        0
    );
    assert_eq!(
        count("SELECT COUNT(*) FROM pragma_table_info('reports') WHERE name IN ('ip_address', 'user_agent', 'device_id')").await,
        0
    );
}

#[tokio::test]
async fn p0_submit_dispatches_the_operator_alert() {
    let h = harness(routes()).await;
    let comment_id = seed_comment(&h, "alert words").await;
    let (status, body) = submit(
        &h.app,
        "203.0.113.90",
        None,
        json!({
            "target_kind": "comment",
            "target_id": comment_id.to_string(),
            "reason": "threat"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let mut rows = Vec::new();
    for _ in 0..50 {
        rows = audit_log::Entity::find()
            .filter(audit_log::Column::Action.eq("alert.dispatch"))
            .all(&h.state.sea_db)
            .await
            .unwrap();
        if !rows.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(rows.len(), 1, "one dispatch row per P0 report");
}
