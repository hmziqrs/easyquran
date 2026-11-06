// Device registry over HTTP (devices.md D2): register, rename, delete, the 409 on
// a removed id, the live-device cap, per-account isolation of one client id, the
// signed-out and browser-shaped refusals, and the rate ceiling on register.
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use eq_sync_core::format_stamp;
use migration::{Migrator, MigratorTrait};
use serde_json::{json, Value};

use reading_common::{router_with_state, seed_user, send, state};
use ruxlog::db::sea_models::device;
use ruxlog::db::sea_models::user::{self};
use ruxlog::db::sea_models::user_device::{self, NewUserDevice};
use ruxlog::db::sea_models::user_session::{self, NewUserSession};
use ruxlog::modules::device_v1;
use ruxlog::services::auth::AuthBackend;
use ruxlog::state::AppState;
use sea_orm::EntityTrait;

use rux_auth::AuthSession as GenAuthSession;

const DEV_A: &str = "0190f2c4-7b1a-7c3d-9e4f-0123456789ab";

struct Ctx {
    app: axum::Router,
    state: AppState,
    cookie_a: String,
    uid_a: i32,
    cookie_b: String,
    uid_b: i32,
}

/// A real login that also writes the audit row and its tower binding, the same
/// two rows `create_bound_session` writes in production. Without the binding the
/// current session has no device to attach to.
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
        .expect("audit row");
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

async fn setup() -> Ctx {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let a = seed_user(&state, "devicea").await;
    let b = seed_user(&state, "deviceb").await;
    let cookie_a = bound_login(&state, &a).await;
    let cookie_b = bound_login(&state, &b).await;
    Ctx {
        app: router_with_state(state.clone(), device_v1::routes(&state)),
        state,
        cookie_a,
        uid_a: a.id,
        cookie_b,
        uid_b: b.id,
    }
}

fn web_registration(uid: i32, device_id: &str) -> Value {
    json!({
        "expected_user_id": uid,
        "device_id": device_id,
        "platform": "web",
        "client": "web",
        "form_factor": "desktop",
        "browser": "chrome",
    })
}

async fn register(ctx: &Ctx, cookie: &str, body: &Value) -> (StatusCode, Value) {
    send(&ctx.app, "POST", "/register", Some(cookie), Some(body)).await
}

async fn list(ctx: &Ctx, cookie: &str) -> Vec<Value> {
    let (status, body) = send(&ctx.app, "POST", "/list", Some(cookie), Some(&json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["devices"].as_array().cloned().unwrap_or_default()
}

fn stamp_now(device: &str) -> String {
    format_stamp(Utc::now().timestamp_millis() as u64, 0, device)
}

#[tokio::test]
async fn register_lists_the_device_as_current_with_no_raw_fields() {
    let ctx = setup().await;
    let (status, body) = register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_a, DEV_A)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "registered": true }));

    let devices = list(&ctx, &ctx.cookie_a).await;
    assert_eq!(devices.len(), 1);
    let row = &devices[0];
    assert_eq!(row["id"], DEV_A);
    assert_eq!(row["isCurrent"], true);
    assert_eq!(row["browser"], "chrome");
    for banned in [
        "token",
        "ip_address",
        "user_agent",
        "device",
        "screen",
        "model",
    ] {
        assert!(row.get(banned).is_none(), "{banned} must not be listed");
    }
}

#[tokio::test]
async fn register_is_idempotent_and_keeps_one_row() {
    let ctx = setup().await;
    for _ in 0..3 {
        let (status, _) = register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_a, DEV_A)).await;
        assert_eq!(status, StatusCode::OK);
    }
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
}

#[tokio::test]
async fn register_refuses_closed_enum_drift_and_unknown_fields() {
    let ctx = setup().await;
    let mut bad_client = web_registration(ctx.uid_a, DEV_A);
    bad_client["client"] = json!("electron");
    let (status, _) = register(&ctx, &ctx.cookie_a, &bad_client).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut mismatch = web_registration(ctx.uid_a, DEV_A);
    mismatch["platform"] = json!("ios");
    let (status, _) = register(&ctx, &ctx.cookie_a, &mismatch).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut ua = web_registration(ctx.uid_a, DEV_A);
    ua["user_agent"] = json!("Mozilla/5.0");
    let (status, _) = register(&ctx, &ctx.cookie_a, &ua).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rename_is_a_register_highest_stamp_wins() {
    let ctx = setup().await;
    register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_a, DEV_A)).await;
    let older = format_stamp(Utc::now().timestamp_millis() as u64 - 60_000, 0, DEV_A);
    let newer = stamp_now(DEV_A);

    let (status, body) = send(
        &ctx.app,
        "POST",
        "/rename",
        Some(&ctx.cookie_a),
        Some(&json!({ "expected_user_id": ctx.uid_a, "device_id": DEV_A, "name": "  Cafe\u{0301}  ", "name_stamp": newer })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["renamed"], true);
    assert_eq!(
        list(&ctx, &ctx.cookie_a).await[0]["displayName"],
        "Caf\u{00E9}"
    );

    let (_, stale) = send(
        &ctx.app,
        "POST",
        "/rename",
        Some(&ctx.cookie_a),
        Some(&json!({ "expected_user_id": ctx.uid_a, "device_id": DEV_A, "name": "Old", "name_stamp": older })),
    )
    .await;
    assert_eq!(stale["renamed"], false);
}

#[tokio::test]
async fn rename_refuses_bidi_overrides_and_overlong_names() {
    let ctx = setup().await;
    register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_a, DEV_A)).await;
    for name in ["evil\u{202E}name", "a\nb", &"x".repeat(41)] {
        let (status, _) = send(
            &ctx.app,
            "POST",
            "/rename",
            Some(&ctx.cookie_a),
            Some(&json!({ "expected_user_id": ctx.uid_a, "device_id": DEV_A, "name": name, "name_stamp": stamp_now(DEV_A) })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "name {name:?}");
    }
}

#[tokio::test]
async fn delete_kills_the_session_and_removes_the_push_token() {
    let ctx = setup().await;
    let mut reg = web_registration(ctx.uid_a, DEV_A);
    reg["token"] = json!("fcm-token-a");
    let (status, _) = register(&ctx, &ctx.cookie_a, &reg).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        device::Entity::list_for_user(&ctx.state.sea_db, ctx.uid_a)
            .await
            .unwrap()
            .len(),
        1
    );

    let (status, body) = send(
        &ctx.app,
        "POST",
        "/delete",
        Some(&ctx.cookie_a),
        Some(&json!({ "expected_user_id": ctx.uid_a, "device_id": DEV_A })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["revoked"], true);

    assert!(device::Entity::list_for_user(&ctx.state.sea_db, ctx.uid_a)
        .await
        .unwrap()
        .is_empty());

    // The same cookie is dead on its next request.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/list",
        Some(&ctx.cookie_a),
        Some(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_removed_id_cannot_register_again() {
    let ctx = setup().await;
    register(&ctx, &ctx.cookie_b, &web_registration(ctx.uid_b, DEV_A)).await;
    // Deleting the device also kills the session that is bound to it, so the
    // re-register goes through a second login of the same account.
    send(
        &ctx.app,
        "POST",
        "/delete",
        Some(&ctx.cookie_b),
        Some(&json!({ "expected_user_id": ctx.uid_b, "device_id": DEV_A })),
    )
    .await;
    let owner = user::Entity::find_by_id(ctx.uid_b)
        .one(&ctx.state.sea_db)
        .await
        .unwrap()
        .unwrap();
    let fresh = bound_login(&ctx.state, &owner).await;
    let (status, body) = register(&ctx, &fresh, &web_registration(ctx.uid_b, DEV_A)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn the_same_id_under_two_accounts_is_two_unrelated_rows() {
    let ctx = setup().await;
    register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_a, DEV_A)).await;
    register(&ctx, &ctx.cookie_b, &web_registration(ctx.uid_b, DEV_A)).await;

    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
    assert_eq!(list(&ctx, &ctx.cookie_b).await.len(), 1);

    // Revoking A's row leaves B's row and B's session alone.
    send(
        &ctx.app,
        "POST",
        "/delete",
        Some(&ctx.cookie_a),
        Some(&json!({ "expected_user_id": ctx.uid_a, "device_id": DEV_A })),
    )
    .await;
    assert_eq!(list(&ctx, &ctx.cookie_b).await.len(), 1);
    let rows = user_device::Entity::live_for_user(&ctx.state.sea_db, ctx.uid_b)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn latest_register_wins_for_form_factor_and_app_version() {
    let ctx = setup().await;
    let mut tablet = web_registration(ctx.uid_a, DEV_A);
    tablet["form_factor"] = json!("tablet");
    tablet["app_version"] = json!("1.0.0");
    register(&ctx, &ctx.cookie_a, &tablet).await;

    let mut desktop = web_registration(ctx.uid_a, DEV_A);
    desktop["form_factor"] = json!("desktop");
    desktop["app_version"] = json!("1.1.0");
    register(&ctx, &ctx.cookie_a, &desktop).await;

    let devices = list(&ctx, &ctx.cookie_a).await;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["formFactor"], "desktop");
    assert_eq!(devices[0]["appVersion"], "1.1.0");
}

#[tokio::test]
async fn signed_out_and_wrong_owner_requests_are_refused() {
    let ctx = setup().await;
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/register",
        None,
        Some(&web_registration(ctx.uid_a, DEV_A)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(&ctx.app, "POST", "/list", None, Some(&json!({}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_b, DEV_A)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn sign_out_token_delete_leaves_the_device_row_alone() {
    let ctx = setup().await;
    let mut reg = web_registration(ctx.uid_a, DEV_A);
    reg["token"] = json!("fcm-token-a");
    register(&ctx, &ctx.cookie_a, &reg).await;

    let (status, body) = send(
        &ctx.app,
        "POST",
        "/token/delete",
        Some(&ctx.cookie_a),
        Some(&json!({ "expected_user_id": ctx.uid_a, "token": "fcm-token-a" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["removed"], 1);
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
}

#[tokio::test]
async fn register_is_rate_limited_per_account() {
    let ctx = setup().await;
    let ceiling = device_v1::REGISTER_USER_MAX;
    for n in 0..ceiling {
        let id = format!("dev-{n:03}");
        let (status, _) = register(&ctx, &ctx.cookie_a, &web_registration(ctx.uid_a, &id)).await;
        assert_eq!(status, StatusCode::OK, "register {n}");
    }
    let (status, _) = register(
        &ctx,
        &ctx.cookie_a,
        &web_registration(ctx.uid_a, "dev-over"),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

fn dated_row(user_id: i32, id: &str, seen_secs_ago: i64) -> user_device::Model {
    let seen = (Utc::now() - chrono::Duration::seconds(seen_secs_ago)).fixed_offset();
    user_device::Model {
        user_id,
        id: id.to_string(),
        display_name: None,
        platform: "web".into(),
        client: "web".into(),
        form_factor: "desktop".into(),
        browser: Some("chrome".into()),
        os_major: None,
        app_version: None,
        name_stamp: None,
        merged_into: None,
        created_at: seen,
        last_seen_at: seen,
        retired_at: None,
        revoked_at: None,
        expires_at: 0,
        retention_class: user_device::DEVICE_RETENTION_CLASS.into(),
    }
}

#[tokio::test]
async fn cap_retires_the_oldest_idle_device_never_the_caller() {
    // Oldest row is the caller. It must survive; the next-oldest goes.
    let live: Vec<user_device::Model> = (0..21)
        .map(|n| dated_row(1, &format!("dev-{n:02}"), 1000 - n as i64))
        .collect();
    let victims = user_device::cap_victims(&live, "dev-20", user_device::LIVE_DEVICE_CAP);
    assert_eq!(victims, vec!["dev-00".to_string()]);
    assert!(!victims.iter().any(|v| v == "dev-20"));

    let at_cap = &live[..20];
    assert!(user_device::cap_victims(at_cap, "dev-19", user_device::LIVE_DEVICE_CAP).is_empty());
}

#[tokio::test]
async fn cap_retirement_keeps_twenty_live_rows_in_the_database() {
    let ctx = setup().await;
    for n in 0..21 {
        user_device::Entity::register(
            &ctx.state.sea_db,
            NewUserDevice {
                user_id: ctx.uid_a,
                id: format!("cap-{n:02}"),
                platform: "web".into(),
                client: "web".into(),
                form_factor: "desktop".into(),
                browser: Some("chrome".into()),
                os_major: None,
                app_version: None,
            },
            None,
        )
        .await
        .unwrap();
    }
    let live = user_device::Entity::live_for_user(&ctx.state.sea_db, ctx.uid_a)
        .await
        .unwrap();
    let victims = user_device::cap_victims(&live, "cap-20", user_device::LIVE_DEVICE_CAP);
    for id in &victims {
        let row = user_device::Entity::find_row(&ctx.state.sea_db, ctx.uid_a, id)
            .await
            .unwrap()
            .unwrap();
        user_device::Entity::retire(&ctx.state.sea_db, row)
            .await
            .unwrap();
    }
    let left = user_device::Entity::live_for_user(&ctx.state.sea_db, ctx.uid_a)
        .await
        .unwrap();
    assert_eq!(left.len(), user_device::LIVE_DEVICE_CAP);
    assert!(left.iter().any(|row| row.id == "cap-20"));
}
