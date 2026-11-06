//! Audit writer (A23): one `audit_logs` row per audited action, evidence columns,
//! no FK to users, and a source-scan guard that every `AuditAction` has a call
//! site and that nothing writes audit rows around the writer.
mod reading_common;

use std::fs;
use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use axum::Router;
use chrono::Utc;
use migration::{Migrator, MigratorTrait};
use reading_common::{harness, router_with_state, seed_user, send, state, Harness};
use regex::Regex;
use rux_auth::AuthSession as GenAuthSession;
use ruxlog::db::sea_models::audit_log;
use ruxlog::db::sea_models::post_comment;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::db::sea_models::user_session::{self, NewUserSession};
use ruxlog::modules::{
    admin_route_v1, admin_user_bans_v1, device_v1, mail_v1, newsletter_v1, post_comment_v1, user_v1,
};
use ruxlog::services::audit::{self, AuditAction, AuditEvent};
use ruxlog::services::auth::AuthBackend;
use ruxlog::AppState;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, Set, Statement,
};
use serde_json::{json, Value};

// ---------- helpers ----------

async fn admin_harness(routes: Router<AppState>) -> Harness {
    let h = harness(routes).await;
    promote(&h.state.sea_db, h.uid_a, UserRole::Admin).await;
    h
}

/// Set a role through the ORM, carrying the session secret so cookies stay valid.
async fn promote(db: &DatabaseConnection, user_id: i32, role: UserRole) {
    let current = user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    user::ActiveModel {
        id: Set(user_id),
        role: Set(role),
        session_auth_secret: Set(current.session_auth_secret),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("set role");
}

async fn migrate(db: &DatabaseConnection) {
    Migrator::up(db, None).await.expect("migrations");
}

async fn rows_for(db: &DatabaseConnection, action: &str) -> Vec<audit_log::model::Model> {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action))
        .all(db)
        .await
        .unwrap()
}

/// Exactly one row for `action`, attributed to `actor`, with the evidence columns set.
async fn assert_one_row(
    db: &DatabaseConnection,
    action: AuditAction,
    actor: i32,
    resource_id: &str,
) -> audit_log::model::Model {
    let rows = rows_for(db, action.as_str()).await;
    assert_eq!(rows.len(), 1, "{} rows for {}", rows.len(), action.as_str());
    let row = rows.into_iter().next().unwrap();
    assert_eq!(row.user_id, Some(actor), "{} actor", action.as_str());
    assert_eq!(
        row.resource_id,
        resource_id,
        "{} resource_id",
        action.as_str()
    );
    assert_eq!(
        row.resource_type,
        action.resource_type(),
        "{} resource_type",
        action.as_str()
    );
    assert_eq!(row.retention_class, audit::RETENTION_CLASS);
    row
}

async fn count_sql(db: &DatabaseConnection, sql: &str) -> i64 {
    let row = db
        .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
        .await
        .unwrap()
        .unwrap();
    row.try_get_by_index(0).unwrap()
}

fn ban_body(user_id: i32) -> Value {
    json!({
        "userId": user_id,
        "reasonCode": "spam",
        "reasonText": "repeated spam",
        "corroborator": "credential_reuse",
        "sharedDeviceChecked": true,
        "expiresAt": (Utc::now() + chrono::Duration::days(1)).fixed_offset().to_rfc3339(),
    })
}

// ---------- writer behaviour ----------

#[tokio::test]
async fn every_action_writes_one_row_with_twelve_month_evidence_columns() {
    let st = state().await;
    migrate(&st.sea_db).await;
    let actor = seed_user(&st, "auditor").await;
    let before = Utc::now().timestamp();

    for &action in AuditAction::ALL {
        audit::record(
            &st.sea_db,
            AuditEvent {
                actor_id: Some(actor.id),
                action,
                resource_id: "r-1".to_string(),
                metadata: json!({ "probe": action.as_str() }),
            },
        )
        .await
        .unwrap();
        let row = assert_one_row(&st.sea_db, action, actor.id, "r-1").await;
        assert_eq!(row.action, action.as_str());
        let ttl = row.expires_at - before;
        assert!(
            (365 * 86_400 - 60..=365 * 86_400 + 60).contains(&ttl),
            "{} expiry is 12 months from write, got {ttl}s",
            action.as_str()
        );
        assert_eq!(row.metadata, Some(json!({ "probe": action.as_str() })));
    }
}

#[tokio::test]
async fn system_action_without_actor_writes_null_user() {
    let st = state().await;
    migrate(&st.sea_db).await;
    audit::record(
        &st.sea_db,
        AuditEvent {
            actor_id: None,
            action: AuditAction::SeedRun,
            resource_id: "7".to_string(),
            metadata: json!({}),
        },
    )
    .await
    .unwrap();
    let rows = rows_for(&st.sea_db, AuditAction::SeedRun.as_str()).await;
    assert_eq!(rows.len(), 1, "one seed.run row");
    assert_eq!(rows[0].user_id, None);
}

#[tokio::test]
async fn audit_rows_have_no_foreign_key_and_survive_account_delete() {
    let st = state().await;
    migrate(&st.sea_db).await;
    let actor = seed_user(&st, "gone-soon").await;
    audit::record(
        &st.sea_db,
        AuditEvent {
            actor_id: Some(actor.id),
            action: AuditAction::BanCreate,
            resource_id: "1".to_string(),
            metadata: json!({}),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        count_sql(
            &st.sea_db,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('audit_logs')"
        )
        .await,
        0,
        "audit_logs must carry no FK to users"
    );
    user::Entity::delete_by_id(actor.id)
        .exec(&st.sea_db)
        .await
        .unwrap();
    assert_eq!(
        count_sql(
            &st.sea_db,
            &format!(
                "SELECT COUNT(*) FROM audit_logs WHERE user_id = {}",
                actor.id
            )
        )
        .await,
        1,
        "audit row must outlive the actor"
    );
}

// ---------- per-action behaviour through the real routes ----------

#[tokio::test]
async fn ban_create_and_revoke_each_write_one_row() {
    let h = admin_harness(admin_user_bans_v1::routes()).await;
    let cookie = h.cookie_a.clone();
    let (status, body) = send(
        &h.app,
        "POST",
        "/",
        Some(cookie.as_str()),
        Some(&ban_body(h.uid_b)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let ban_id = body["banId"].as_i64().unwrap();
    assert_one_row(
        &h.state.sea_db,
        AuditAction::BanCreate,
        h.uid_a,
        &ban_id.to_string(),
    )
    .await;

    let uri = format!("/{ban_id}/revoke");
    let (status, body) = send(&h.app, "POST", &uri, Some(cookie.as_str()), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_one_row(
        &h.state.sea_db,
        AuditAction::BanRevoke,
        h.uid_a,
        &ban_id.to_string(),
    )
    .await;
}

#[tokio::test]
async fn route_block_unblock_and_delete_each_write_one_row() {
    let h = admin_harness(admin_route_v1::routes()).await;
    let cookie = h.cookie_a.clone();
    let pattern = "/audit-probe";
    let (status, body) = send(
        &h.app,
        "POST",
        "/block",
        Some(cookie.as_str()),
        Some(&json!({ "pattern": pattern })),
    )
    .await;
    assert!(status.is_success(), "block: {status} {body}");
    assert_one_row(&h.state.sea_db, AuditAction::RouteBlock, h.uid_a, pattern).await;

    let (status, body) = send(
        &h.app,
        "POST",
        "/unblock",
        Some(cookie.as_str()),
        Some(&json!({ "pattern": pattern })),
    )
    .await;
    assert!(status.is_success(), "unblock: {status} {body}");
    assert_one_row(&h.state.sea_db, AuditAction::RouteUnblock, h.uid_a, pattern).await;

    let (status, body) = send(
        &h.app,
        "POST",
        "/delete",
        Some(cookie.as_str()),
        Some(&json!({ "pattern": pattern })),
    )
    .await;
    assert!(status.is_success(), "delete: {status} {body}");
    assert_one_row(&h.state.sea_db, AuditAction::RouteDelete, h.uid_a, pattern).await;
}

#[tokio::test]
async fn suppression_add_and_delete_each_write_one_row() {
    let h = admin_harness(mail_v1::routes()).await;
    let cookie = h.cookie_a.clone();
    let recipient = "audit-probe@example.com";
    let (status, body) = send(
        &h.app,
        "POST",
        "/suppression",
        Some(cookie.as_str()),
        Some(&json!({ "recipient": recipient, "reason": "manual", "permanent": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_one_row(
        &h.state.sea_db,
        AuditAction::SuppressionAdd,
        h.uid_a,
        recipient,
    )
    .await;

    let uri = format!("/suppression?recipient={recipient}");
    let (status, body) = send(&h.app, "DELETE", &uri, Some(cookie.as_str()), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["removed"], json!(true));
    assert_one_row(
        &h.state.sea_db,
        AuditAction::SuppressionDelete,
        h.uid_a,
        recipient,
    )
    .await;
}

#[tokio::test]
async fn admin_user_delete_writes_one_row_and_keeps_it() {
    let h = admin_harness(user_v1::routes()).await;
    let cookie = h.cookie_a.clone();
    let victim = seed_user(&h.state, "victim").await;
    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{}", victim.id),
        Some(cookie.as_str()),
        Some(&json!({ "confirmEmail": victim.email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let row = assert_one_row(
        &h.state.sea_db,
        AuditAction::UserDelete,
        h.uid_a,
        &victim.id.to_string(),
    )
    .await;
    let snapshot = row.metadata.expect("pre-state snapshot");
    assert_eq!(snapshot["user"]["email"], json!(victim.email));
    assert_eq!(
        count_sql(
            &h.state.sea_db,
            &format!("SELECT COUNT(*) FROM users WHERE id = {}", victim.id)
        )
        .await,
        0,
        "victim is gone"
    );
}

/// A real login with its tower binding, as device_registry.rs does. Without the
/// binding the session has no device to attach to.
async fn bound_login(state: &AppState, u: &user::Model) -> String {
    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    let session = tower_sessions::Session::new(None, state.session_store.clone(), None);
    let mut auth = GenAuthSession::new(backend, session).await;
    auth.login(u).await.expect("login succeeds");
    auth.session().save().await.expect("session persisted");
    let tower_sid = auth
        .session()
        .id()
        .expect("session id after save")
        .to_string();
    let audit = user_session::Entity::create(&state.sea_db, NewUserSession::new(u.id, None, None))
        .await
        .expect("session row");
    auth.backend()
        .record_session_mapping(&state.sea_db, audit.id, &tower_sid)
        .await
        .expect("binding recorded");
    drop(auth);
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let mut jar = tower_sessions::cookie::CookieJar::new();
    jar.private_mut(&cookie_key)
        .add(tower_sessions::cookie::Cookie::new("ruxlog.sid", tower_sid));
    jar.get("ruxlog.sid")
        .expect("forged session cookie present")
        .to_string()
}

#[tokio::test]
async fn device_remove_writes_one_row_with_device_class_and_no_device_id() {
    const DEVICE: &str = "0190f2c4-7b1a-7c3d-9e4f-0123456789ab";
    let st = state().await;
    migrate(&st.sea_db).await;
    let owner = seed_user(&st, "devicerm").await;
    let cookie = bound_login(&st, &owner).await;
    let app = router_with_state(st.clone(), device_v1::routes(&st));

    let registration = json!({
        "expected_user_id": owner.id,
        "device_id": DEVICE,
        "platform": "web",
        "client": "web",
        "form_factor": "desktop",
        "browser": "chrome",
        "key_store": "none",
    });
    let (status, body) = send(
        &app,
        "POST",
        "/register",
        Some(cookie.as_str()),
        Some(&registration),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "register: {body}");

    let (status, body) = send(
        &app,
        "POST",
        "/delete",
        Some(cookie.as_str()),
        Some(&json!({ "expected_user_id": owner.id, "device_id": DEVICE })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delete: {body}");

    let rows = rows_for(&st.sea_db, AuditAction::DeviceRemove.as_str()).await;
    assert_eq!(rows.len(), 1, "one device.remove row");
    assert_eq!(rows[0].user_id, Some(owner.id));
    assert_eq!(rows[0].resource_type, "device");
    assert_eq!(rows[0].resource_id, owner.id.to_string());
    assert_eq!(
        rows[0].metadata,
        Some(json!({ "platform": "web", "client": "web", "formFactor": "desktop" }))
    );
    // The device id must not appear in any audit column.
    let everything = format!("{:?}", rows[0]);
    assert!(
        !everything.contains(DEVICE),
        "device id leaked into audit row"
    );
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

#[tokio::test]
async fn comment_hide_unhide_delete_flags_clear_each_write_one_row() {
    let h = admin_harness(post_comment_v1::routes()).await;
    let cookie = h.cookie_a.clone();
    let db = &h.state.sea_db;
    // Seed comments without a parent post: this test is about the audit row, not
    // post integrity. The pool is one connection, so the pragma holds for the test.
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .unwrap();
    let by_admin = insert_comment(db, h.uid_b).await;
    let flagged = insert_comment(db, h.uid_b).await;
    let own = insert_comment(db, h.uid_a).await;

    for (uri, action) in [
        (format!("/admin/hide/{by_admin}"), AuditAction::CommentHide),
        (
            format!("/admin/unhide/{by_admin}"),
            AuditAction::CommentUnhide,
        ),
        (
            format!("/admin/flags/clear/{flagged}"),
            AuditAction::CommentFlagsClear,
        ),
    ] {
        let (status, body) = send(&h.app, "POST", &uri, Some(cookie.as_str()), None).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_one_row(db, action, h.uid_a, &action_resource(&uri)).await;
    }

    // Author deletes own comment: the actor is the author.
    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/delete/{own}"),
        Some(cookie.as_str()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "author delete: {body}");
    let author_rows = rows_for_resource(db, AuditAction::CommentDelete, &own.to_string()).await;
    assert_eq!(author_rows.len(), 1, "author delete row");
    assert_eq!(author_rows[0].user_id, Some(h.uid_a));
    assert_eq!(author_rows[0].metadata, Some(json!({ "by": "author" })));

    // Admin deletes another account's comment.
    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{by_admin}"),
        Some(cookie.as_str()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "admin delete: {body}");
    let admin_rows = rows_for_resource(db, AuditAction::CommentDelete, &by_admin.to_string()).await;
    assert_eq!(admin_rows.len(), 1, "admin delete row");
    assert_eq!(admin_rows[0].metadata, Some(json!({ "by": "admin" })));
}

/// The comment id at the end of an admin URI.
fn action_resource(uri: &str) -> String {
    uri.rsplit('/').next().unwrap_or_default().to_string()
}

async fn rows_for_resource(
    db: &DatabaseConnection,
    action: AuditAction,
    resource_id: &str,
) -> Vec<audit_log::model::Model> {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(action.as_str()))
        .filter(audit_log::Column::ResourceId.eq(resource_id))
        .all(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn newsletter_send_is_audited_before_the_202() {
    let h = admin_harness(newsletter_v1::routes()).await;
    let cookie = h.cookie_a.clone();
    let (status, body) = send(
        &h.app,
        "POST",
        "/send",
        Some(cookie.as_str()),
        Some(&json!({ "subject": "Audit probe", "text": "body" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let rows = rows_for(&h.state.sea_db, AuditAction::NewsletterSend.as_str()).await;
    assert_eq!(rows.len(), 1, "newsletter.send row exists when 202 returns");
    assert_eq!(rows[0].user_id, Some(h.uid_a));
    assert_eq!(rows[0].metadata, Some(json!({ "subject": "Audit probe" })));
}

// ---------- source-scan guard ----------

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn is_writer_file(rel: &str) -> bool {
    rel == "services/audit.rs" || rel.starts_with("db/sea_models/audit_log/")
}

/// Variant names of `pub enum AuditAction { ... }`, one per line.
fn enum_variants(source: &str) -> Vec<String> {
    let Some(start) = source.find("pub enum AuditAction {") else {
        return Vec::new();
    };
    let body = &source[start..];
    let body = &body[..body.find('}').unwrap_or(body.len())];
    let variant = Regex::new(r"^\s*([A-Z][A-Za-z0-9]*),?\s*$").unwrap();
    body.lines()
        .skip(1)
        .filter_map(|line| variant.captures(line).map(|c| c[1].to_string()))
        .collect()
}

/// Every `AuditAction::Variant` reference in a source text.
fn call_sites(source: &str) -> Vec<String> {
    let site = Regex::new(r"AuditAction::([A-Z][A-Za-z0-9]*)").unwrap();
    site.captures_iter(source)
        .map(|c| c[1].to_string())
        .collect()
}

/// Raw audit writes that bypass the writer: entity inserts or SQL inserts.
fn direct_writes(source: &str) -> usize {
    let entity = Regex::new(r"audit_log::model::ActiveModel|audit_log::ActiveModel").unwrap();
    let sql = Regex::new(r#"(?i)insert\s+into\s+"?audit_logs"?"#).unwrap();
    entity.find_iter(source).count() + sql.find_iter(source).count()
}

#[test]
fn guard_self_tests_detect_what_they_claim() {
    let enum_src = "pub enum AuditAction {\n    RouteBlock,\n    BanCreate,\n}\n";
    assert_eq!(enum_variants(enum_src), vec!["RouteBlock", "BanCreate"]);
    assert_eq!(
        call_sites("record(AuditAction::RouteBlock, x) AuditAction::SeedRun"),
        vec!["RouteBlock", "SeedRun"]
    );
    assert_eq!(
        direct_writes("audit_log::model::ActiveModel { }"),
        1,
        "entity insert is a direct write"
    );
    assert_eq!(
        direct_writes(r#"INSERT INTO "audit_logs" (id) VALUES (1)"#),
        1,
        "SQL insert is a direct write"
    );
    assert_eq!(
        direct_writes("audit::record(&txn, AuditEvent { .. })"),
        0,
        "the writer itself is not a direct write"
    );
}

#[test]
fn every_audit_action_has_a_call_site_and_nothing_bypasses_the_writer() {
    let root = src_root();
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        !files.is_empty(),
        "source scan found no files under {root:?}"
    );

    let writer_src = fs::read_to_string(root.join("services/audit.rs")).unwrap();
    let variants = enum_variants(&writer_src);
    assert_eq!(
        variants.len(),
        AuditAction::ALL.len(),
        "AuditAction::ALL must list every variant"
    );

    let mut used: Vec<String> = Vec::new();
    let mut bypass = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if is_writer_file(&rel) {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap_or_default();
        used.extend(call_sites(&text));
        if direct_writes(&text) > 0 {
            bypass.push(rel);
        }
    }

    let missing: Vec<&String> = variants.iter().filter(|v| !used.contains(v)).collect();
    let unknown: Vec<&String> = used.iter().filter(|u| !variants.contains(u)).collect();
    assert!(
        missing.is_empty(),
        "audit actions without a call site: {missing:?}"
    );
    assert!(
        unknown.is_empty(),
        "call sites name no variant: {unknown:?}"
    );
    assert!(
        bypass.is_empty(),
        "audit rows written outside services/audit.rs: {bypass:?}"
    );
}
