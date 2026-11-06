//! Case evidence, legal holds and the comment removal guard (A25).
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use reading_common::{harness, send, Harness};
use ruxlog::db::sea_models::comment_flag::slice::NewCommentFlag;
use ruxlog::db::sea_models::legal_hold::{self, HoldRule, HoldSubject, NewHold};
use ruxlog::db::sea_models::report::{self, NewReport, ReportReason, ReportTargetKind};
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::db::sea_models::{
    audit_log, case_evidence, comment_flag, media, post_comment, post_view,
};
use ruxlog::modules::post_comment_v1;
use ruxlog::services::audit::{self, AuditAction, AuditEvent};
use ruxlog::services::{incident_capture, legal_hold as holds};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;

const DAY: i64 = 24 * 60 * 60;

const CAPTURE_KINDS: [&str; 7] = [
    "comment",
    "comment_flags",
    "user",
    "sessions",
    "views",
    "media",
    "audit",
];

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

/// Comments here point at no post: this test is about the guard, not post integrity.
async fn no_fk(db: &DatabaseConnection) {
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .unwrap();
}

async fn insert_comment(db: &DatabaseConnection, author: i32) -> i32 {
    let now = Utc::now().fixed_offset();
    post_comment::ActiveModel {
        post_id: Set(1),
        user_id: Set(author),
        content: Set("audit probe".to_string()),
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

async fn flag(db: &DatabaseConnection, comment_id: i32, by: i32) {
    comment_flag::Entity::create(
        db,
        NewCommentFlag {
            comment_id,
            user_id: by,
            reason: Some("spam".to_string()),
        },
    )
    .await
    .expect("flag recorded");
}

async fn comment_exists(db: &DatabaseConnection, id: i32) -> bool {
    post_comment::Entity::find_by_id(id)
        .one(db)
        .await
        .unwrap()
        .is_some()
}

fn removal_ref(comment_id: i32) -> String {
    format!("comment-{comment_id}")
}

async fn audit_rows(db: &DatabaseConnection, action: AuditAction) -> Vec<audit_log::model::Model> {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action.as_str()))
        .all(db)
        .await
        .unwrap()
}

// ---------- removal guard ----------

#[tokio::test]
async fn author_removal_of_flagged_comment_needs_confirm_evidence() {
    let h = harness(post_comment_v1::routes()).await;
    let db = &h.state.sea_db;
    no_fk(db).await;
    let comment = insert_comment(db, h.uid_a).await;
    flag(db, comment, h.uid_b).await;
    let uri = format!("/delete/{comment}");

    let (status, body) = send(&h.app, "POST", &uri, Some(h.cookie_a.as_str()), None).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "refused without confirm: {body}"
    );
    assert!(
        comment_exists(db, comment).await,
        "refused removal keeps the comment"
    );
    assert!(
        case_evidence::Entity::list_for_case(db, &removal_ref(comment))
            .await
            .unwrap()
            .is_empty(),
        "refusal writes no evidence"
    );

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("{uri}?confirm_evidence=true"),
        Some(h.cookie_a.as_str()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "confirmed removal: {body}");
    assert!(
        !comment_exists(db, comment).await,
        "confirmed removal deletes the comment"
    );

    let rows = case_evidence::Entity::list_for_case(db, &removal_ref(comment))
        .await
        .unwrap();
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds, CAPTURE_KINDS, "evidence copied before the delete");
    let flags = rows.iter().find(|r| r.kind == "comment_flags").unwrap();
    assert_eq!(flags.snapshot.as_array().map(Vec::len), Some(1));
    assert_eq!(audit_rows(db, AuditAction::CommentDelete).await.len(), 1);
}

#[tokio::test]
async fn admin_removal_of_flagged_comment_needs_confirm_evidence() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    no_fk(db).await;
    let comment = insert_comment(db, h.uid_b).await;
    flag(db, comment, h.uid_b).await;
    let cookie = h.cookie_a.as_str();

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{comment}"),
        Some(cookie),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "admin refused without confirm: {body}"
    );
    assert!(comment_exists(db, comment).await);

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{comment}?confirm_evidence=true"),
        Some(cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "admin confirmed removal: {body}");
    assert!(!comment_exists(db, comment).await);
    let rows = case_evidence::Entity::list_for_case(db, &removal_ref(comment))
        .await
        .unwrap();
    assert_eq!(rows.len(), CAPTURE_KINDS.len());
}

#[tokio::test]
async fn flags_clear_keeps_flag_rows_so_removal_stays_guarded() {
    let h = admin_harness().await;
    let db = &h.state.sea_db;
    no_fk(db).await;
    let comment = insert_comment(db, h.uid_b).await;
    flag(db, comment, h.uid_b).await;
    let cookie = h.cookie_a.as_str();

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/flags/clear/{comment}"),
        Some(cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "flags clear: {body}");
    assert_eq!(
        comment_flag::Entity::count_for_comment(db, comment)
            .await
            .unwrap(),
        1,
        "flags-clear zeroes the cached count, not the evidence rows"
    );

    let (status, _) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{comment}"),
        Some(cookie),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "removal still guarded after flags clear"
    );
}

// ---------- incident capture ----------

#[tokio::test]
async fn incident_capture_copies_every_object_and_writes_one_audit_row() {
    let h = harness(post_comment_v1::routes()).await;
    let db = &h.state.sea_db;
    no_fk(db).await;
    let now = Utc::now();
    let comment = insert_comment(db, h.uid_a).await;
    flag(db, comment, h.uid_b).await;

    let avatar = media::model::ActiveModel {
        object_key: Set("avatars/probe.webp".to_string()),
        mime_type: Set("image/webp".to_string()),
        width: Set(Some(64)),
        height: Set(Some(64)),
        size: Set(1234),
        is_optimized: Set(false),
        created_at: Set(now.fixed_offset()),
        updated_at: Set(now.fixed_offset()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("avatar seeded");
    user::ActiveModel {
        id: Set(h.uid_a),
        avatar_id: Set(Some(avatar.id)),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("avatar linked");

    post_view::ActiveModel {
        post_id: Set(1),
        ip_address: Set(Some("203.0.113.9".to_string())),
        user_agent: Set(Some("ua-probe".to_string())),
        user_id: Set(Some(h.uid_a)),
        created_at: Set(now.fixed_offset()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("view seeded");

    audit::record(
        db,
        AuditEvent {
            actor_id: Some(h.uid_b),
            action: AuditAction::CommentHide,
            resource_id: comment.to_string(),
            metadata: json!({}),
        },
    )
    .await
    .unwrap();

    let reported = report::Entity::insert_report(
        db,
        NewReport {
            reporter_user_id: Some(h.uid_b),
            target_kind: ReportTargetKind::Comment,
            target_id: comment.to_string(),
            reason: ReportReason::Harassment,
            details: None,
            contact_email: None,
            snapshot: None,
        },
    )
    .await
    .expect("report seeded");

    let outcome = incident_capture::capture_report(db, reported.id, now)
        .await
        .expect("capture");
    let case_ref = format!("report-{}", reported.id);
    assert_eq!(outcome.case_ref, case_ref);
    assert_eq!(outcome.rows, CAPTURE_KINDS.len());

    let rows = case_evidence::Entity::list_for_case(db, &case_ref)
        .await
        .unwrap();
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds, CAPTURE_KINDS);
    for row in &rows {
        assert_eq!(
            row.expires_at - row.captured_at,
            365 * DAY,
            "12-month expiry"
        );
        assert_eq!(row.retention_class, case_evidence::RETENTION_CASE_12M);
    }

    let text: String = rows.iter().map(|r| r.snapshot.to_string()).collect();
    assert!(text.contains("audit probe"), "comment content captured");
    assert!(
        text.contains("avatars/probe.webp"),
        "media metadata captured"
    );
    assert!(text.contains("203.0.113.9"), "view ip captured");
    assert!(!text.contains("ua-probe"), "view user agent not captured");
    assert!(
        !text.contains("device"),
        "no device identity in any snapshot"
    );

    let incident = audit_rows(db, AuditAction::IncidentCapture).await;
    assert_eq!(incident.len(), 1, "one incident.capture row");
    assert_eq!(incident[0].resource_id, case_ref);
    assert_eq!(incident[0].user_id, None, "CLI capture has no actor");

    let post_report = report::Entity::insert_report(
        db,
        NewReport {
            reporter_user_id: Some(h.uid_b),
            target_kind: ReportTargetKind::Post,
            target_id: "1".to_string(),
            reason: ReportReason::Spam,
            details: None,
            contact_email: None,
            snapshot: None,
        },
    )
    .await
    .unwrap();
    assert!(
        incident_capture::capture_report(db, post_report.id, now)
            .await
            .is_err(),
        "only comment reports are captured in A25"
    );
}

// ---------- legal holds ----------

#[tokio::test]
async fn legal_hold_classes_set_expiry_renewal_and_copied_subjects() {
    let h = harness(post_comment_v1::routes()).await;
    let db = &h.state.sea_db;
    let hold = |rule: HoldRule, subjects: Vec<HoldSubject>| NewHold {
        rule,
        authority: "ops".to_string(),
        reason: "test".to_string(),
        owner: "operator".to_string(),
        case_ref: Some("report-1".to_string()),
        subjects,
    };

    let csam = holds::place(
        db,
        Some(h.uid_b),
        hold(
            HoldRule::CsamHold1y,
            vec![HoldSubject {
                kind: "comment".to_string(),
                id: "7".to_string(),
                user_id: Some(h.uid_a),
            }],
        ),
    )
    .await
    .expect("csam hold");
    assert_eq!(csam.expires_at - csam.placed_at, 365 * DAY);
    assert!(!csam.auto_release, "CSAM hold never auto-releases");
    assert_eq!(csam.retention_class, "csam_hold_1y");
    assert_eq!(csam.preserve_until, None);
    let subjects = legal_hold::actions::subjects_for(db, csam.id)
        .await
        .unwrap();
    assert_eq!(subjects.len(), 1);
    assert_eq!(subjects[0].subject_kind, "comment");
    assert_eq!(subjects[0].subject_id, "7");
    assert_eq!(subjects[0].subject_user_id, Some(h.uid_a));
    assert_eq!(subjects[0].expires_at, csam.expires_at);

    let le = holds::place(db, Some(h.uid_b), hold(HoldRule::LawEnforcement90d, vec![]))
        .await
        .expect("law-enforcement hold");
    assert_eq!(le.expires_at - le.placed_at, 90 * DAY);
    assert_eq!(le.preserve_until, Some(le.expires_at));
    assert!(le.auto_release);
    let renewed = holds::renew(db, Some(h.uid_b), le.id).await.expect("renew");
    assert_eq!(renewed.preserve_until, Some(le.expires_at + 90 * DAY));
    assert_eq!(renewed.expires_at, le.expires_at + 90 * DAY);
    assert_eq!(renewed.renewals, 1);

    assert!(
        holds::renew(db, Some(h.uid_b), csam.id).await.is_err(),
        "CSAM hold is not renewable"
    );

    let enforcement = holds::place(db, Some(h.uid_b), hold(HoldRule::Enforcement12m, vec![]))
        .await
        .expect("enforcement hold");
    assert_eq!(enforcement.expires_at - enforcement.placed_at, 365 * DAY);
    assert_eq!(enforcement.retention_class, "enforcement_12m");

    assert_eq!(audit_rows(db, AuditAction::LegalHoldPlace).await.len(), 3);
    assert_eq!(audit_rows(db, AuditAction::LegalHoldRenew).await.len(), 1);
}
