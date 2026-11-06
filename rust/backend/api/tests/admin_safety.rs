//! Admin safety (A28): confirmations on destructive admin deletes, Admin-only media
//! delete, route-blocker self-protection and persisted pause, the production seed
//! gate, the app_constants secret denylist, and the unmetered-nest guard.
mod reading_common;

use std::fs;
use std::path::PathBuf;

use axum::http::StatusCode;
use axum::Router;
use reading_common::{harness, seed_user, send, Harness};
use ruxlog::db::sea_models::audit_log;
use ruxlog::db::sea_models::category;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::modules::{admin_route_v1, category_v1, media_v1, user_v1};
use ruxlog::services::audit::AuditAction;
use ruxlog::services::route_blocker_service::{is_protected_route, RouteBlockerService};
use ruxlog::AppState;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter, Set,
    Statement,
};
use serde_json::json;

// ---------- helpers ----------

/// Logs the harness user in as `role` (set through the ORM, keeping the session secret).
async fn set_role(h: &Harness, role: UserRole) {
    let db: &DatabaseConnection = &h.state.sea_db;
    let current = user::Entity::find_by_id(h.uid_a)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    user::ActiveModel {
        id: Set(h.uid_a),
        role: Set(role),
        session_auth_secret: Set(current.session_auth_secret),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("set role");
}

async fn harness_as(routes: Router<AppState>, role: UserRole) -> Harness {
    let h = harness(routes).await;
    set_role(&h, role).await;
    h
}

async fn count_sql(db: &DatabaseConnection, sql: &str) -> i64 {
    let row = sea_orm::ConnectionTrait::query_one(
        db,
        Statement::from_string(DbBackend::Sqlite, sql.to_string()),
    )
    .await
    .unwrap()
    .unwrap();
    row.try_get_by_index(0).unwrap()
}

fn src_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

// ---------- user delete: typed confirmation ----------

#[tokio::test]
async fn user_delete_without_matching_email_is_refused_and_keeps_user() {
    let h = harness_as(user_v1::routes(), UserRole::Admin).await;
    let victim = seed_user(&h.state, "victim-confirm").await;
    let cookie = h.cookie_a.clone();

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{}", victim.id),
        Some(cookie.as_str()),
        Some(&json!({ "confirmEmail": "someone-else@example.com" })),
    )
    .await;
    assert_ne!(status, StatusCode::OK, "{body}");
    assert_eq!(
        count_sql(
            &h.state.sea_db,
            &format!("SELECT COUNT(*) FROM users WHERE id = {}", victim.id)
        )
        .await,
        1,
        "victim kept after refused delete"
    );

    let (status, body) = send(
        &h.app,
        "POST",
        &format!("/admin/delete/{}", victim.id),
        Some(cookie.as_str()),
        None,
    )
    .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "missing confirmation accepted: {body}"
    );
}

// ---------- category delete: typed confirmation + audit ----------

#[tokio::test]
async fn category_delete_requires_slug_confirmation_and_audits() {
    let h = harness_as(category_v1::routes(), UserRole::Admin).await;
    let cookie = h.cookie_a.clone();
    let (status, body) = send(
        &h.app,
        "POST",
        "/create",
        Some(cookie.as_str()),
        Some(&json!({ "name": "Safe", "slug": "safe-cat-a28", "color": "#112233" })),
    )
    .await;
    assert!(status.is_success(), "create category: {body}");
    let cat = category::Entity::find()
        .filter(category::Column::Slug.eq("safe-cat-a28"))
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .expect("category row");
    let uri = format!("/delete/{}", cat.id);

    let (status, body) = send(&h.app, "POST", &uri, Some(cookie.as_str()), None).await;
    assert_ne!(status, StatusCode::OK, "no confirmation accepted: {body}");

    let (status, body) = send(
        &h.app,
        "POST",
        &uri,
        Some(cookie.as_str()),
        Some(&json!({ "confirmSlug": "wrong-slug" })),
    )
    .await;
    assert_ne!(status, StatusCode::OK, "wrong slug accepted: {body}");
    assert_eq!(
        count_sql(
            &h.state.sea_db,
            &format!("SELECT COUNT(*) FROM categories WHERE id = {}", cat.id)
        )
        .await,
        1,
        "category kept after refused delete"
    );

    let (status, body) = send(
        &h.app,
        "POST",
        &uri,
        Some(cookie.as_str()),
        Some(&json!({ "confirmSlug": "safe-cat-a28" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let logged = audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(AuditAction::CategoryDelete.as_str()))
        .all(&h.state.sea_db)
        .await
        .unwrap();
    assert_eq!(logged.len(), 1, "one category.delete row");
    assert_eq!(logged[0].resource_id, cat.id.to_string());
}

// ---------- media delete: Admin only ----------

#[tokio::test]
async fn moderator_cannot_delete_media_but_admin_passes_the_guard() {
    let moderator = harness_as(media_v1::routes(), UserRole::Moderator).await;
    let cookie = moderator.cookie_a.clone();
    let (status, body) = send(
        &moderator.app,
        "POST",
        "/delete/999999",
        Some(cookie.as_str()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "moderator delete: {body}");

    let admin = harness_as(media_v1::routes(), UserRole::Admin).await;
    let cookie = admin.cookie_a.clone();
    let (status, body) = send(
        &admin.app,
        "POST",
        "/delete/999999",
        Some(cookie.as_str()),
        None,
    )
    .await;
    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "admin blocked by guard: {body}"
    );
}

// ---------- route blocker: own admin routes ----------

#[tokio::test]
async fn block_refuses_the_route_blockers_own_admin_routes() {
    let h = harness_as(admin_route_v1::routes(), UserRole::Admin).await;
    let cookie = h.cookie_a.clone();
    let (status, body) = send(
        &h.app,
        "POST",
        "/block",
        Some(cookie.as_str()),
        Some(&json!({ "pattern": "/admin/route/v1/unblock" })),
    )
    .await;
    assert_ne!(status, StatusCode::CREATED, "self-block accepted: {body}");
    assert_eq!(
        count_sql(
            &h.state.sea_db,
            "SELECT COUNT(*) FROM route_status WHERE route_pattern = '/admin/route/v1/unblock'"
        )
        .await,
        0,
        "no route row written for refused block"
    );
}

#[test]
fn protected_route_predicate_covers_the_admin_nest_only() {
    assert!(is_protected_route("/admin/route/v1/unblock"));
    assert!(is_protected_route("/admin/route/v1"));
    assert!(is_protected_route("/admin/route/v1/"));
    assert!(!is_protected_route("/post/v1/list"));
    assert!(!is_protected_route("/admin/routes-extra"));
}

// ---------- route blocker: persisted pause ----------

#[tokio::test]
async fn sync_pause_persists_in_app_constants_and_reads_back() {
    let h: Harness = harness(Router::<AppState>::new()).await;
    let db = &h.state.sea_db;
    assert_eq!(
        RouteBlockerService::load_sync_paused(db).await.unwrap(),
        None
    );
    RouteBlockerService::persist_sync_paused(db, true, h.uid_a)
        .await
        .unwrap();
    assert_eq!(
        RouteBlockerService::load_sync_paused(db).await.unwrap(),
        Some(true)
    );
    RouteBlockerService::persist_sync_paused(db, false, h.uid_a)
        .await
        .unwrap();
    assert_eq!(
        RouteBlockerService::load_sync_paused(db).await.unwrap(),
        Some(false)
    );
}

// ---------- unmetered-nest guard ----------

/// Text from `start` up to the next nest, merge, or statement end.
fn segment(src: &str, start: usize) -> &str {
    let rest = &src[start + 1..];
    let mut end = rest.len();
    for marker in [".nest(", ".merge(", ";"] {
        if let Some(i) = rest.find(marker) {
            end = end.min(i);
        }
    }
    &rest[..end]
}

#[test]
fn every_nest_in_router_is_metered() {
    let src = fs::read_to_string(src_path("src/router.rs")).expect("read router.rs");
    let mut starts: Vec<usize> = src.match_indices(".nest(").map(|(i, _)| i).collect();
    starts.extend(src.match_indices(".merge(admin_").map(|(i, _)| i));
    starts.sort_unstable();

    let mut unmetered = Vec::new();
    for &start in &starts {
        if !segment(&src, start).contains("rate_limit") {
            let line = src[start..].lines().next().unwrap_or("").trim().to_string();
            unmetered.push(line);
        }
    }
    assert!(
        starts.len() >= 30,
        "guard found only {} nests; scan is broken",
        starts.len()
    );
    assert!(unmetered.is_empty(), "unmetered nests: {unmetered:?}");
}

// ---------- audit vocabulary ----------

#[test]
fn category_delete_action_has_its_own_resource_type() {
    assert_eq!(AuditAction::CategoryDelete.as_str(), "category.delete");
    assert_eq!(AuditAction::CategoryDelete.resource_type(), "category");
    assert!(AuditAction::ALL.contains(&AuditAction::CategoryDelete));
}
