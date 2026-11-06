//! Comment moderation hardening (A27): notices and appeal tokens for hide and removal,
//! edit lock on hidden or flagged comments, flag clearing that holds, self-flag refusal
//! and the flag rate limit.
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use reading_common::{harness, login_cookie, seed_user, send, Harness};
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::db::sea_models::{audit_log, category, enforcement_notice, post, post_comment};
use ruxlog::modules::post_comment_v1;
use ruxlog::services::audit::AuditAction;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::json;

/// uid_a becomes Admin (moderator actions); uid_b is the comment author.
async fn admin_harness() -> Harness {
    let h = harness(post_comment_v1::routes()).await;
    let current = user::Entity::find_by_id(h.uid_a)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap();
    user::ActiveModel {
        id: Set(h.uid_a),
        role: Set(UserRole::Admin),
        session_auth_secret: Set(current.session_auth_secret),
        ..Default::default()
    }
    .update(&h.state.sea_db)
    .await
    .expect("promote to admin");
    h
}

async fn seed_post(db: &DatabaseConnection, author: i32) -> i32 {
    let now = Utc::now().fixed_offset();
    let cat = category::ActiveModel {
        name: Set("A27".into()),
        slug: Set("a27".into()),
        color: Set("#ffffff".into()),
        text_color: Set("#000000".into()),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("category seeded");
    post::ActiveModel {
        title: Set("A27 post".into()),
        slug: Set("a27-post".into()),
        content: Set(json!({"time": 0, "blocks": [], "version": "2.30.7"})),
        author_id: Set(author),
        category_id: Set(cat.id),
        status: Set(post::PostStatus::Published),
        view_count: Set(0),
        likes_count: Set(0),
        tag_ids: Set(post::TagIds(vec![])),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("post seeded")
    .id
}

async fn seed_comment(db: &DatabaseConnection, post_id: i32, author: i32) -> i32 {
    let now = Utc::now().fixed_offset();
    post_comment::ActiveModel {
        post_id: Set(post_id),
        user_id: Set(author),
        content: Set("original text".to_string()),
        likes_count: Set(0),
        hidden: Set(false),
        flags_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("comment seeded")
    .id
}

async fn comment(db: &DatabaseConnection, id: i32) -> post_comment::Model {
    post_comment::Entity::find_by_id(id)
        .one(db)
        .await
        .unwrap()
        .unwrap()
}

async fn notices_for_comment(db: &DatabaseConnection, id: i32) -> Vec<enforcement_notice::Model> {
    enforcement_notice::Entity::find()
        .filter(enforcement_notice::Column::CommentId.eq(id))
        .all(db)
        .await
        .unwrap()
}

async fn comment_flag_count(db: &DatabaseConnection, id: i32) -> usize {
    ruxlog::db::sea_models::comment_flag::Entity::find()
        .filter(ruxlog::db::sea_models::comment_flag::Column::CommentId.eq(id))
        .all(db)
        .await
        .unwrap()
        .len()
}

async fn flag_as(app: &axum::Router, comment_id: i32, cookie: &str) -> StatusCode {
    let (status, _) = send(
        app,
        "POST",
        &format!("/flag/{comment_id}"),
        Some(cookie),
        Some(&json!({ "reason": "spam" })),
    )
    .await;
    status
}

async fn edit_as(app: &axum::Router, comment_id: i32, cookie: &str, text: &str) -> StatusCode {
    let (status, _) = send(
        app,
        "POST",
        &format!("/update/{comment_id}"),
        Some(cookie),
        Some(&json!({ "content": text })),
    )
    .await;
    status
}

// ---------- notices and appeal tokens ----------

#[tokio::test]
async fn hiding_a_comment_creates_a_notice_and_an_appeal_token() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/hide/{c}"),
        Some(h.cookie_a.as_str()),
        Some(&json!({ "reasonCode": "spam", "reasonText": "repeated links" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "hide: {body}");
    assert!(body["noticeId"].is_number(), "notice id returned: {body}");

    let notices = notices_for_comment(db, c).await;
    assert_eq!(notices.len(), 1, "one notice for the hide");
    let notice = &notices[0];
    assert_eq!(notice.subject_kind, enforcement_notice::SUBJECT_COMMENT);
    assert_eq!(notice.user_id, h.uid_b, "notice names the author");
    assert_eq!(notice.ban_id, None, "a comment notice has no ban");
    assert_eq!(notice.reason_code, "spam");
    assert_eq!(notice.reason_text.as_deref(), Some("repeated links"));
    assert!(!notice.automated, "admin action is not automated");
    assert!(
        notice.appeal_token_hash.is_some(),
        "appeal token hash stored"
    );

    let rows = audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(AuditAction::CommentNotice.as_str()))
        .filter(audit_log::Column::ResourceId.eq(c.to_string()))
        .all(db)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "notice writes one audit row");

    // Hiding an already hidden comment opens no second case.
    let (again, _) = send(
        &h.app,
        "POST",
        &format!("/admin/hide/{c}"),
        Some(h.cookie_a.as_str()),
        Some(&json!({ "reasonCode": "spam" })),
    )
    .await;
    assert_eq!(again, StatusCode::OK);
    assert_eq!(
        notices_for_comment(db, c).await.len(),
        1,
        "no duplicate notice"
    );
}

#[tokio::test]
async fn removing_a_comment_creates_a_notice_with_default_reason() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{c}"),
        Some(h.cookie_a.as_str()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "remove: {body}");
    let notices = notices_for_comment(db, c).await;
    assert_eq!(notices.len(), 1, "removal opens a case");
    assert_eq!(notices[0].reason_code, "policy_violation", "default reason");
    assert!(notices[0].appeal_token_hash.is_some());
}

#[tokio::test]
async fn unknown_notice_reason_is_refused_before_any_write() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;

    let (status, _) = send(
        &h.app,
        "POST",
        &format!("/admin/hide/{c}"),
        Some(h.cookie_a.as_str()),
        Some(&json!({ "reasonCode": "made_up" })),
    )
    .await;
    assert!(status.is_client_error(), "unknown reason refused: {status}");
    assert!(!comment(db, c).await.hidden, "refused hide changes nothing");
    assert!(notices_for_comment(db, c).await.is_empty());
}

// ---------- edit lock ----------

#[tokio::test]
async fn author_cannot_edit_a_hidden_comment() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;
    let (hide, _) = send(
        &h.app,
        "POST",
        &format!("/admin/hide/{c}"),
        Some(h.cookie_a.as_str()),
        None,
    )
    .await;
    assert_eq!(hide, StatusCode::OK);

    let status = edit_as(&h.app, c, h.cookie_b.as_str(), "edited while hidden").await;
    assert!(status.is_client_error(), "hidden edit refused: {status}");
    assert_eq!(comment(db, c).await.content, "original text");
}

#[tokio::test]
async fn author_edit_locks_at_three_flags_and_not_before() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;
    let f1 = seed_user(&h.state, "a27flag1").await;
    let f2 = seed_user(&h.state, "a27flag2").await;
    let f3 = seed_user(&h.state, "a27flag3").await;
    let k1 = login_cookie(&h.state, &f1).await;
    let k2 = login_cookie(&h.state, &f2).await;
    let k3 = login_cookie(&h.state, &f3).await;

    assert_eq!(flag_as(&h.app, c, k1.as_str()).await, StatusCode::OK);
    assert_eq!(flag_as(&h.app, c, k2.as_str()).await, StatusCode::OK);
    assert_eq!(
        edit_as(&h.app, c, h.cookie_b.as_str(), "two flags").await,
        StatusCode::OK,
        "two flags still editable"
    );

    assert_eq!(flag_as(&h.app, c, k3.as_str()).await, StatusCode::OK);
    let status = edit_as(&h.app, c, h.cookie_b.as_str(), "three flags").await;
    assert!(status.is_client_error(), "three flags lock edits: {status}");
    assert_eq!(comment(db, c).await.content, "two flags");
}

// ---------- flag clearing ----------

#[tokio::test]
async fn cleared_flags_stay_cleared_after_a_new_flag() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;
    let f1 = seed_user(&h.state, "a27clear1").await;
    let f2 = seed_user(&h.state, "a27clear2").await;
    let f3 = seed_user(&h.state, "a27clear3").await;
    let k1 = login_cookie(&h.state, &f1).await;
    let k2 = login_cookie(&h.state, &f2).await;
    let k3 = login_cookie(&h.state, &f3).await;
    assert_eq!(flag_as(&h.app, c, k1.as_str()).await, StatusCode::OK);
    assert_eq!(flag_as(&h.app, c, k2.as_str()).await, StatusCode::OK);
    assert_eq!(comment(db, c).await.flags_count, 2);

    let (clear, _) = send(
        &h.app,
        "POST",
        &format!("/admin/flags/clear/{c}"),
        Some(h.cookie_a.as_str()),
        None,
    )
    .await;
    assert_eq!(clear, StatusCode::OK);
    assert_eq!(
        comment(db, c).await.flags_count,
        0,
        "clear zeroes the count"
    );
    assert_eq!(
        comment_flag_count(db, c).await,
        2,
        "flag rows stay as evidence"
    );

    // One new flag counts as one. The two cleared flags do not come back.
    assert_eq!(flag_as(&h.app, c, k3.as_str()).await, StatusCode::OK);
    assert_eq!(comment(db, c).await.flags_count, 1, "new flag only");
    assert_eq!(comment_flag_count(db, c).await, 3);

    // A cleared flagger who flags again counts again, as one new flag.
    assert_eq!(flag_as(&h.app, c, k1.as_str()).await, StatusCode::OK);
    assert_eq!(comment(db, c).await.flags_count, 2);
}

// ---------- self-flag and rate limit ----------

#[tokio::test]
async fn author_cannot_flag_their_own_comment() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    let post_id = seed_post(db, h.uid_b).await;
    let c = seed_comment(db, post_id, h.uid_b).await;

    let status = flag_as(&h.app, c, h.cookie_b.as_str()).await;
    assert!(status.is_client_error(), "self-flag refused: {status}");
    assert_eq!(comment(db, c).await.flags_count, 0);
    assert_eq!(comment_flag_count(db, c).await, 0);
}

// The HTTP wrapper skips the limiter outside production (integration tests run as
// development), so this drives the same config through the gate directly, as
// report_intake does.
#[tokio::test]
async fn flag_limit_allows_twenty_an_hour_and_refuses_the_twenty_first() {
    let h = admin_harness().await;
    let key = format!("comment:flag:user:{}", h.uid_b);
    let cfg = post_comment_v1::controller::flag_limit_config();
    assert_eq!(cfg.temp_block_range, 60 * 60, "hourly window");
    assert_eq!(cfg.block_range, 24 * 60 * 60, "daily window");
    assert_eq!(cfg.block_retry_limit, 51, "50 a day allowed");

    for i in 0..20 {
        let d = rux_request_gate::check(&h.state.gate_store, &rux_request_gate::NoHooks, &key, cfg)
            .await
            .expect("gate check");
        assert!(
            matches!(d, rux_request_gate::LimiterDecision::Allowed { .. }),
            "flag {i}"
        );
    }
    let d = rux_request_gate::check(&h.state.gate_store, &rux_request_gate::NoHooks, &key, cfg)
        .await
        .expect("gate check");
    assert!(
        matches!(d, rux_request_gate::LimiterDecision::Blocked { .. }),
        "21st flag in an hour refused"
    );
}
