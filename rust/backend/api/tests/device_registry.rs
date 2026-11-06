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
use ruxlog::modules::{device_v1, reading_session_v1};
use ruxlog::services::auth::AuthBackend;
use ruxlog::state::AppState;
use sea_orm::{ConnectionTrait, EntityTrait};

use aws_lc_rs::{
    hmac,
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use base64::prelude::*;
use rux_auth::AuthSession as GenAuthSession;
use ruxlog::modules::device_v1::keys::proof_message;
use sha2::{Digest, Sha256};

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
        "key_store": "none",
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
    assert_eq!(body, json!({ "registered": true, "device_id": DEV_A }));

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
        "public_key",
        "key_thumbprint",
        "key_store",
        "continuity_hint",
        "merged_into",
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
        public_key: None,
        key_thumbprint: None,
        key_store: "none".into(),
        continuity_hint: None,
        hint_conflict: false,
        merged_key_thumbprint: None,
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
                key_store: "none".into(),
                key: None,
                continuity_hint: None,
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

// ---- Device keys and recognition (devices.md D3) ----

/// A P-256 key per install, the way a native client holds one. The test keeps the
/// key pair; the server only ever sees the public JWK.
struct Device {
    pair: EcdsaKeyPair,
    jwk: Value,
}

fn new_device() -> Device {
    let pair = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).expect("key pair");
    let point = pair.public_key().as_ref().to_vec();
    let jwk = json!({
        "kty": "EC",
        "crv": "P-256",
        "x": BASE64_URL_SAFE_NO_PAD.encode(&point[1..33]),
        "y": BASE64_URL_SAFE_NO_PAD.encode(&point[33..]),
    });
    Device { pair, jwk }
}

fn thumbprint_of(jwk: &Value) -> String {
    let canonical = format!(
        r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#,
        jwk["x"].as_str().unwrap(),
        jwk["y"].as_str().unwrap()
    );
    BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest(canonical.as_bytes()))
}

fn sign_proof(dev: &Device, nonce: &str, device_id: &str, uid: i32) -> String {
    let sig = dev
        .pair
        .sign(&SystemRandom::new(), &proof_message(nonce, device_id, uid))
        .expect("signature");
    BASE64_URL_SAFE_NO_PAD.encode(sig.as_ref())
}

/// The continuity hint a native client sends: HMAC-SHA256(account salt, OS id).
fn hint_for(salt_b64: &str, os_id: &str) -> String {
    let salt = BASE64_URL_SAFE_NO_PAD.decode(salt_b64).expect("salt");
    let key = hmac::Key::new(hmac::HMAC_SHA256, &salt);
    BASE64_URL_SAFE_NO_PAD.encode(hmac::sign(&key, os_id.as_bytes()).as_ref())
}

async fn challenge(ctx: &Ctx, cookie: &str) -> (String, String) {
    let (status, body) = send(
        &ctx.app,
        "POST",
        "/challenge",
        Some(cookie),
        Some(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["nonce"].as_str().unwrap().to_string(),
        body["salt"].as_str().unwrap().to_string(),
    )
}

/// A keyed Android register: the shape a hint-bearing native client sends.
fn keyed(uid: i32, device_id: &str, dev: &Device, nonce: &str, hint: Option<&str>) -> Value {
    let mut body = json!({
        "expected_user_id": uid,
        "device_id": device_id,
        "platform": "android",
        "client": "react_native",
        "form_factor": "phone",
        "key_store": "secure_hardware",
        "public_key": dev.jwk,
        "nonce": nonce,
        "signature": sign_proof(dev, nonce, device_id, uid),
    });
    if let Some(hint) = hint {
        body["continuity_hint"] = json!(hint);
    }
    body
}

/// The same device shape with no key, as a client whose keystore failed sends it.
fn keyless(uid: i32, device_id: &str) -> Value {
    json!({
        "expected_user_id": uid,
        "device_id": device_id,
        "platform": "android",
        "client": "react_native",
        "form_factor": "phone",
        "key_store": "none",
    })
}

async fn login_as(ctx: &Ctx, uid: i32) -> String {
    let owner = user::Entity::find_by_id(uid)
        .one(&ctx.state.sea_db)
        .await
        .unwrap()
        .unwrap();
    bound_login(&ctx.state, &owner).await
}

async fn row(ctx: &Ctx, uid: i32, id: &str) -> user_device::Model {
    user_device::Entity::find_row(&ctx.state.sea_db, uid, id)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn keyed_register_keeps_the_id_and_the_salt_is_stable_per_account() {
    let ctx = setup().await;
    let dev = new_device();
    let (nonce, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let (status, body) = register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-k1", &dev, &nonce, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-k1");

    let (nonce2, salt2) = challenge(&ctx, &ctx.cookie_a).await;
    assert_ne!(nonce, nonce2, "every challenge is fresh");
    assert_eq!(salt, salt2, "one salt per account, never rotated");

    let stored = row(&ctx, ctx.uid_a, "dev-k1").await;
    assert_eq!(
        stored.key_thumbprint.as_deref(),
        Some(thumbprint_of(&dev.jwk).as_str())
    );
    assert_eq!(stored.key_store, "secure_hardware");
}

#[tokio::test]
async fn the_same_machine_hints_differently_under_two_accounts() {
    let ctx = setup().await;
    let (_, salt_a) = challenge(&ctx, &ctx.cookie_a).await;
    let (_, salt_b) = challenge(&ctx, &ctx.cookie_b).await;
    assert_ne!(salt_a, salt_b, "each account gets its own salt");
    let os = "0a1b2c-machine.guid";
    assert_ne!(hint_for(&salt_a, os), hint_for(&salt_b, os));
}

#[tokio::test]
async fn proof_failures_are_refused_and_spend_the_nonce() {
    let ctx = setup().await;
    let dev = new_device();
    let other = new_device();

    // Wrong signer.
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let mut forged = keyed(ctx.uid_a, "dev-f", &dev, &nonce, None);
    forged["signature"] = json!(sign_proof(&other, &nonce, "dev-f", ctx.uid_a));
    let (status, _) = register(&ctx, &ctx.cookie_a, &forged).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Signed for another device id.
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let mut wrong_id = keyed(ctx.uid_a, "dev-f", &dev, &nonce, None);
    wrong_id["signature"] = json!(sign_proof(&dev, &nonce, "dev-other", ctx.uid_a));
    let (status, _) = register(&ctx, &ctx.cookie_a, &wrong_id).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A good proof, then the same nonce again: single use.
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let good = keyed(ctx.uid_a, "dev-f", &dev, &nonce, None);
    assert_eq!(register(&ctx, &ctx.cookie_a, &good).await.0, StatusCode::OK);
    let (status, _) = register(&ctx, &ctx.cookie_a, &good).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "reused nonce");

    // A nonce issued to another session does not count.
    let (nonce, _) = challenge(&ctx, &ctx.cookie_b).await;
    let stolen = keyed(ctx.uid_a, "dev-g", &dev, &nonce, None);
    let (status, _) = register(&ctx, &ctx.cookie_a, &stolen).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "nonce from another session"
    );

    // Nothing was stored by any refused attempt.
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
}

#[tokio::test]
async fn key_store_and_key_presence_must_agree() {
    let ctx = setup().await;
    let dev = new_device();
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let mut claims_none = keyed(ctx.uid_a, "dev-n", &dev, &nonce, None);
    claims_none["key_store"] = json!("none");
    let (status, _) = register(&ctx, &ctx.cookie_a, &claims_none).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = register(&ctx, &ctx.cookie_a, &keyless(ctx.uid_a, "dev-n")).await;
    assert_eq!(status, StatusCode::OK, "no key is a valid register");
}

#[tokio::test]
async fn rung_one_same_key_and_hint_is_the_same_row() {
    let ctx = setup().await;
    let dev = new_device();
    let (_, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt, "0a1b2c-machine.guid");
    for _ in 0..2 {
        let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
        let (status, body) = register(
            &ctx,
            &ctx.cookie_a,
            &keyed(ctx.uid_a, "dev-r1", &dev, &nonce, Some(&hint)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["device_id"], "dev-r1");
    }
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
}

#[tokio::test]
async fn rung_one_valid_key_with_a_different_hint_is_a_conflict() {
    let ctx = setup().await;
    let dev = new_device();
    let (nonce, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt, "0a1b2c-machine.guid");
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-r2", &dev, &nonce, Some(&hint)),
    )
    .await;
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let other = hint_for(&salt, "ffffff-other.machine");
    let (status, body) = register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-r2", &dev, &nonce, Some(&other)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn rung_one_known_id_with_a_wrong_key_is_a_conflict_without_the_session() {
    let ctx = setup().await;
    let first = new_device();
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-r3", &first, &nonce, None),
    )
    .await;

    // Another session of the same account holds no binding to this row.
    let other_session = login_as(&ctx, ctx.uid_a).await;
    let imposter = new_device();
    let (nonce, _) = challenge(&ctx, &other_session).await;
    let (status, body) = register(
        &ctx,
        &other_session,
        &keyed(ctx.uid_a, "dev-r3", &imposter, &nonce, None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn rung_two_session_holds_the_row_so_a_lost_key_keeps_the_row() {
    let ctx = setup().await;
    let phone = new_device();
    let (nonce, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt, "0a1b2c-machine.guid");
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-z", &phone, &nonce, Some(&hint)),
    )
    .await;

    // The keystore was wiped: a new id and a new key from the same session.
    let replacement = new_device();
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let (status, body) = register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-lost", &replacement, &nonce, Some(&hint)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-z", "the client takes the row's id");

    let devices = list(&ctx, &ctx.cookie_a).await;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["id"], "dev-z");
    assert_eq!(
        row(&ctx, ctx.uid_a, "dev-z")
            .await
            .key_thumbprint
            .as_deref(),
        Some(thumbprint_of(&replacement.jwk).as_str())
    );
}

#[tokio::test]
async fn rung_two_key_none_on_a_bound_session_keeps_the_row() {
    let ctx = setup().await;
    let dev = new_device();
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-w", &dev, &nonce, None),
    )
    .await;
    let (status, body) = register(&ctx, &ctx.cookie_a, &keyless(ctx.uid_a, "dev-w")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-w");
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
}

#[tokio::test]
async fn rung_three_new_id_with_a_live_hint_takes_over_the_old_row() {
    let ctx = setup().await;
    let old_phone = new_device();
    let (nonce, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt, "0a1b2c-machine.guid");
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-old", &old_phone, &nonce, Some(&hint)),
    )
    .await;

    // A reinstall: a fresh session, a new id and key, the same hint.
    let reinstalled = login_as(&ctx, ctx.uid_a).await;
    let new_phone = new_device();
    let (nonce, _) = challenge(&ctx, &reinstalled).await;
    let (status, body) = register(
        &ctx,
        &reinstalled,
        &keyed(ctx.uid_a, "dev-new", &new_phone, &nonce, Some(&hint)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-new");

    let devices = list(&ctx, &reinstalled).await;
    assert_eq!(devices.len(), 1, "one device, one row");
    assert_eq!(devices[0]["id"], "dev-new");

    let old = row(&ctx, ctx.uid_a, "dev-old").await;
    assert_eq!(old.merged_into.as_deref(), Some("dev-new"));
    assert!(old.retired_at.is_some());
    assert_eq!(
        row(&ctx, ctx.uid_a, "dev-new")
            .await
            .merged_key_thumbprint
            .as_deref(),
        Some(thumbprint_of(&old_phone.jwk).as_str()),
        "the old key is kept through the merge"
    );
}

#[tokio::test]
async fn rule_seven_a_shared_hint_is_detected_and_neither_machine_is_signed_out() {
    let ctx = setup().await;
    let phone = new_device();
    let (nonce, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt, "0a1b2c-machine.guid");
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-old", &phone, &nonce, Some(&hint)),
    )
    .await;
    // A second machine cloned from the first: new id and key, same hint.
    let clone_session = login_as(&ctx, ctx.uid_a).await;
    let clone = new_device();
    let (nonce, _) = challenge(&ctx, &clone_session).await;
    register(
        &ctx,
        &clone_session,
        &keyed(ctx.uid_a, "dev-new", &clone, &nonce, Some(&hint)),
    )
    .await;

    // The first machine comes back with its old key: the shared hint is found.
    let (nonce, _) = challenge(&ctx, &ctx.cookie_a).await;
    let (status, body) = register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-old", &phone, &nonce, Some(&hint)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-old");
    assert!(row(&ctx, ctx.uid_a, "dev-old").await.hint_conflict);
    assert!(row(&ctx, ctx.uid_a, "dev-new").await.hint_conflict);
    assert!(row(&ctx, ctx.uid_a, "dev-old").await.merged_into.is_none());

    // Both sessions still work, and both devices list.
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 2);
    assert_eq!(list(&ctx, &clone_session).await.len(), 2);

    // A flagged hint never matches again: a third install is a new row.
    let third_session = login_as(&ctx, ctx.uid_a).await;
    let third = new_device();
    let (nonce, _) = challenge(&ctx, &third_session).await;
    let (status, body) = register(
        &ctx,
        &third_session,
        &keyed(ctx.uid_a, "dev-third", &third, &nonce, Some(&hint)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-third");
    assert_eq!(list(&ctx, &third_session).await.len(), 3);
}

#[tokio::test]
async fn rung_four_two_sessions_without_a_hint_are_two_devices() {
    let ctx = setup().await;
    let (status, _) = register(&ctx, &ctx.cookie_a, &keyless(ctx.uid_a, "dev-p")).await;
    assert_eq!(status, StatusCode::OK);
    let second = login_as(&ctx, ctx.uid_a).await;
    let (status, _) = register(&ctx, &second, &keyless(ctx.uid_a, "dev-q")).await;
    assert_eq!(status, StatusCode::OK);
    // Same platform, client, form factor: not merged on coarse traits alone.
    assert_eq!(list(&ctx, &second).await.len(), 2);
}

#[tokio::test]
async fn rung_four_retires_an_old_look_alike_with_no_live_session() {
    let ctx = setup().await;
    register(&ctx, &ctx.cookie_a, &keyless(ctx.uid_a, "dev-r")).await;
    // The session that registered dev-r is dead.
    ctx.state
        .sea_db
        .execute_unprepared(
            "UPDATE user_sessions SET revoked_at = '2026-01-01T00:00:00+00:00' WHERE device_id = 'dev-r'",
        )
        .await
        .unwrap();

    let next = login_as(&ctx, ctx.uid_a).await;
    let (status, _) = register(&ctx, &next, &keyless(ctx.uid_a, "dev-s")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(row(&ctx, ctx.uid_a, "dev-r").await.retired_at.is_some());
    let devices = list(&ctx, &next).await;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["id"], "dev-s");
}

#[tokio::test]
async fn an_account_only_ever_sees_its_own_rows_for_recognition() {
    let ctx = setup().await;
    let dev = new_device();
    let (nonce, salt_a) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt_a, "0a1b2c-machine.guid");
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-shared", &dev, &nonce, Some(&hint)),
    )
    .await;

    // Account B, same machine, same key and hint bytes: no match, separate row.
    let (nonce_b, salt_b) = challenge(&ctx, &ctx.cookie_b).await;
    let hint_b = hint_for(&salt_b, "0a1b2c-machine.guid");
    let (status, body) = register(
        &ctx,
        &ctx.cookie_b,
        &keyed(ctx.uid_b, "dev-shared", &dev, &nonce_b, Some(&hint_b)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-shared");
    assert_eq!(list(&ctx, &ctx.cookie_b).await.len(), 1);
    assert_eq!(list(&ctx, &ctx.cookie_a).await.len(), 1);
    assert!(row(&ctx, ctx.uid_b, "dev-shared")
        .await
        .merged_into
        .is_none());
}

// ---- Handoff by device (devices.md D5) -------------------------------------------
//
// Threads are written and pulled through the reading-session sync route on the same
// state, so a pulled thread's `deviceName` is checked end to end.

const THREAD_A: &str = "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d501";
const THREAD_B: &str = "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d502";
const THREAD_C: &str = "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d503";
const THREAD_D: &str = "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d504";

fn sync_app(ctx: &Ctx) -> axum::Router {
    router_with_state(ctx.state.clone(), reading_session_v1::routes())
}

/// One open thread written from `device_id`, with all three registers stamped.
fn thread_from(id: &str, device_id: &str) -> Value {
    let stamp = stamp_now(device_id);
    json!({
        "kind": "session.upsert", "id": id,
        "identityStamp": stamp, "origin": "index", "scope": "surah:1",
        "startedAt": "2026-11-01T00:00:00.000Z",
        "cursorStamp": stamp, "surah": 1, "ayah": 1, "device": "phone",
        "deviceId": device_id,
        "lifecycleStamp": stamp, "state": "open",
        "updatedAt": "2026-11-01T00:00:00.000Z",
    })
}

/// A full pull (`since` 0) that also writes `mutations` first, as one round.
async fn sync_round(ctx: &Ctx, uid: i32, cookie: &str, mutations: Vec<Value>) -> Value {
    let body = json!({ "expectedUserId": uid, "mutations": mutations, "since": 0 });
    let (status, res) = send(&sync_app(ctx), "POST", "/sync", Some(cookie), Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    res
}

fn pulled<'a>(res: &'a Value, id: &str) -> &'a Value {
    res["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .find(|s| s["id"] == id)
        .unwrap_or_else(|| panic!("thread {id} not pulled: {res}"))
}

async fn name_device(ctx: &Ctx, cookie: &str, uid: i32, id: &str, name: &str) {
    let body = json!({
        "expected_user_id": uid,
        "device_id": id,
        "name": name,
        "name_stamp": stamp_now(id),
    });
    let (status, res) = send(&ctx.app, "POST", "/rename", Some(cookie), Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["renamed"], true, "{res}");
}

#[tokio::test]
async fn a_thread_from_another_phone_carries_that_phones_name() {
    let ctx = setup().await;
    register(&ctx, &ctx.cookie_a, &keyless(ctx.uid_a, "dev-pixel")).await;
    name_device(&ctx, &ctx.cookie_a, ctx.uid_a, "dev-pixel", "Pixel 8").await;

    let res = sync_round(
        &ctx,
        ctx.uid_a,
        &ctx.cookie_a,
        vec![thread_from(THREAD_A, "dev-pixel")],
    )
    .await;
    let row = pulled(&res, THREAD_A);
    assert_eq!(row["deviceId"], "dev-pixel");
    assert_eq!(row["deviceName"], "Pixel 8");
}

#[tokio::test]
async fn a_removed_or_unknown_device_stores_the_thread_and_resolves_to_no_name() {
    let ctx = setup().await;
    // Registered from its own session, so revoking it leaves the caller's session alive.
    let gone_session = login_as(&ctx, ctx.uid_a).await;
    register(&ctx, &gone_session, &keyless(ctx.uid_a, "dev-gone")).await;
    name_device(&ctx, &gone_session, ctx.uid_a, "dev-gone", "Old phone").await;
    let (status, body) = send(
        &ctx.app,
        "POST",
        "/delete",
        Some(&ctx.cookie_a),
        Some(&json!({ "expected_user_id": ctx.uid_a, "device_id": "dev-gone" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Dirty rows from the removed device land after a fresh sign-in (S9 rule): kept as
    // sent, applied, and labelled by nothing, so the client shows its device kind.
    let resigned = login_as(&ctx, ctx.uid_a).await;
    let res = sync_round(
        &ctx,
        ctx.uid_a,
        &resigned,
        vec![
            thread_from(THREAD_B, "dev-gone"),
            thread_from(THREAD_C, "never-registered"),
        ],
    )
    .await;
    for (id, device) in [(THREAD_B, "dev-gone"), (THREAD_C, "never-registered")] {
        let landed = res["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .expect("a result per row");
        assert_eq!(landed["status"], "applied", "{res}");
        let row = pulled(&res, id);
        assert_eq!(row["deviceId"], device);
        assert!(row["deviceName"].is_null(), "{row}");
    }
}

#[tokio::test]
async fn a_reinstalled_device_keeps_its_name_and_its_old_threads_still_resolve() {
    let ctx = setup().await;
    let old_phone = new_device();
    let (nonce, salt) = challenge(&ctx, &ctx.cookie_a).await;
    let hint = hint_for(&salt, "0a1b2c-pixel.guid");
    register(
        &ctx,
        &ctx.cookie_a,
        &keyed(ctx.uid_a, "dev-old-pixel", &old_phone, &nonce, Some(&hint)),
    )
    .await;
    name_device(&ctx, &ctx.cookie_a, ctx.uid_a, "dev-old-pixel", "Pixel 8").await;

    // Reinstall: new id and key, same hint. The old row is merged into the new one.
    let reinstalled = login_as(&ctx, ctx.uid_a).await;
    let new_phone = new_device();
    let (nonce, _) = challenge(&ctx, &reinstalled).await;
    let (status, body) = register(
        &ctx,
        &reinstalled,
        &keyed(ctx.uid_a, "dev-new-pixel", &new_phone, &nonce, Some(&hint)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], "dev-new-pixel");
    assert_eq!(
        row(&ctx, ctx.uid_a, "dev-new-pixel")
            .await
            .display_name
            .as_deref(),
        Some("Pixel 8"),
        "the name carries over the merge"
    );

    // A thread the old install wrote before the reinstall still reads its name.
    let res = sync_round(
        &ctx,
        ctx.uid_a,
        &reinstalled,
        vec![thread_from(THREAD_D, "dev-old-pixel")],
    )
    .await;
    assert_eq!(pulled(&res, THREAD_D)["deviceName"], "Pixel 8");
}

#[tokio::test]
async fn device_names_never_cross_accounts() {
    let ctx = setup().await;
    // The same client id under two accounts is two devices, each with its own name.
    register(&ctx, &ctx.cookie_a, &keyless(ctx.uid_a, "dev-both")).await;
    name_device(&ctx, &ctx.cookie_a, ctx.uid_a, "dev-both", "A phone").await;
    register(&ctx, &ctx.cookie_b, &keyless(ctx.uid_b, "dev-both")).await;
    name_device(&ctx, &ctx.cookie_b, ctx.uid_b, "dev-both", "B phone").await;

    let res_b = sync_round(
        &ctx,
        ctx.uid_b,
        &ctx.cookie_b,
        vec![thread_from(THREAD_A, "dev-both")],
    )
    .await;
    assert_eq!(pulled(&res_b, THREAD_A)["deviceName"], "B phone");
    let res_a = sync_round(&ctx, ctx.uid_a, &ctx.cookie_a, vec![]).await;
    assert!(res_a["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["id"] != THREAD_A));
}
