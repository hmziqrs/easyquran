// Native client contract over HTTP (devices.md D6): the x-eq-session transport,
// a full native flow with no Origin and no cookie jar, browser-shaped refusals,
// the CORS allow list, idle lifetimes (180 days native, 14 web), and revocation.
//
// The app is the real private router with the same layer order as main.rs:
// transport outside the session layer, the expiry guard inside it.
mod reading_common;

use std::str::FromStr;

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Method, Request, StatusCode},
    middleware, Extension, Router,
};
use chrono::Utc;
use eq_sync_core::format_stamp;
use migration::{Migrator, MigratorTrait};
use serde_json::{json, Value};
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_sessions::cookie::{Cookie, CookieJar, Key};
use tower_sessions::session::{Id, Record};
use tower_sessions::{SessionManagerLayer, SessionStore};

use aws_lc_rs::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use base64::prelude::*;
use ruxlog::db::sea_models::user::{self, NewUser, UserRole};
use ruxlog::middlewares::{client_ip::resolve_client_ip, cors, native_session, static_csrf};
use ruxlog::modules::auth_v1;
use ruxlog::modules::device_v1::keys::proof_message;
use ruxlog::router;
use ruxlog::state::AppState;
use ruxlog::utils::code_hash::hash_code;

const PASSWORD: &str = "native-client-test-password";
const RN_DEVICE: &str = "0190f2c4-7b1a-7c3d-9e4f-0123456789ab";
const ISO: &str = "2026-11-01T00:00:00.000Z";
const SESSION: &str = "x-eq-session";

/// The app as main.rs builds it, minus the observability and security-header
/// layers, which no contract here depends on.
fn native_app(state: AppState) -> Router {
    let cookie_key = Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    let private_cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(cors::private_request_headers())
        .allow_origin(AllowOrigin::list(
            state.allowed_origins.header_values().to_vec(),
        ))
        .allow_credentials(true);
    // WEB_AUTH_ENABLED is off in tests, so the auth nest is mounted here, as the
    // rotation test does.
    router::router(state.clone())
        .nest("/auth/v1", auth_v1::routes())
        .layer(middleware::from_fn(resolve_client_ip))
        .layer(Extension(axum_client_ip::ClientIpSource::CfConnectingIp))
        .layer(middleware::from_fn(cors::origin_guard))
        .layer(Extension(state.clone()))
        .layer(middleware::from_fn(static_csrf::csrf_guard))
        .layer(middleware::from_fn(native_session::keep_native_alive))
        .layer(session_layer)
        .layer(middleware::from_fn(native_session::transport))
        .layer(private_cors)
        .with_state(state)
}

// csrf_guard derives its key from COOKIE_KEY; this test binary is not cfg(test).
static COOKIE_KEY_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();

async fn state_and_app() -> (AppState, Router) {
    COOKIE_KEY_ENV.get_or_init(|| {
        std::env::set_var("COOKIE_KEY", "test-cookie-key-not-for-production-use-32+");
    });
    let state = reading_common::state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let app = native_app(state.clone());
    (state, app)
}

/// A verified account with a password, created through the real user path.
async fn seed_account(state: &AppState, name: &str) -> user::Model {
    let code_hash = hash_code(&state.secret_key, "dummy-verification-code");
    let created = user::Entity::create(
        &state.sea_db,
        NewUser {
            name: name.into(),
            email: format!("{name}@example.com"),
            password: PASSWORD.into(),
            role: UserRole::User,
            is_minor: false,
            age_declared: false,
        },
        code_hash,
    )
    .await
    .expect("account created");
    user::Entity::verify(&state.sea_db, created.id)
        .await
        .expect("account verified")
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

/// One request. `native` adds the x-eq-session header; `cookie` is a jar cookie
/// (browser shape); `extra` adds arbitrary headers.
async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    native: Option<&str>,
    cookie: Option<&str>,
    extra: &[(&str, &str)],
    body: Option<Value>,
) -> Reply {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("cf-connecting-ip", "203.0.113.7");
    if let Some(value) = native {
        req = req.header(SESSION, value);
    }
    if let Some(value) = cookie {
        req = req.header(header::COOKIE, value);
    }
    for (name, value) in extra {
        req = req.header(*name, *value);
    }
    let req = match body {
        Some(json) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    Reply {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

fn header_str(reply: &Reply, name: &str) -> String {
    reply
        .headers
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default()
}

/// The native token for a fresh anonymous session: `x-eq-session` from the reply.
async fn native_csrf(app: &Router, token_in: &str) -> (String, String) {
    let reply = call(
        app,
        Method::POST,
        "/csrf/v1/generate",
        Some(token_in),
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    (
        reply.body["token"].as_str().unwrap().to_string(),
        header_str(&reply, SESSION),
    )
}

/// Decrypt a `ruxlog.sid` value to its tower session id (the server's own key).
fn session_id(state: &AppState, value: &str) -> Id {
    let key = Key::derive_from(state.settings.cookie_key.as_bytes());
    let mut jar = CookieJar::new();
    jar.add_original(Cookie::new("ruxlog.sid", value.to_owned()));
    let plain = jar
        .private(&key)
        .get("ruxlog.sid")
        .expect("session cookie decrypts")
        .value()
        .to_owned();
    Id::from_str(&plain).expect("session id parses")
}

async fn stored(state: &AppState, value: &str) -> Record {
    state
        .session_store
        .load(&session_id(state, value))
        .await
        .expect("store reads")
        .expect("session is stored")
}

fn days_from_now(record: &Record) -> f64 {
    (record.expiry_date - time::OffsetDateTime::now_utc()).as_seconds_f64() / 86_400.0
}

/// A P-256 key pair per install; the server only sees the public JWK.
struct Key256 {
    pair: EcdsaKeyPair,
    jwk: Value,
}

fn new_key() -> Key256 {
    let pair = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).expect("key pair");
    let point = pair.public_key().as_ref().to_vec();
    let jwk = json!({
        "kty": "EC",
        "crv": "P-256",
        "x": BASE64_URL_SAFE_NO_PAD.encode(&point[1..33]),
        "y": BASE64_URL_SAFE_NO_PAD.encode(&point[33..]),
    });
    Key256 { pair, jwk }
}

fn sign(key: &Key256, nonce: &str, device_id: &str, user_id: i32) -> String {
    let sig = key
        .pair
        .sign(
            &SystemRandom::new(),
            &proof_message(nonce, device_id, user_id),
        )
        .expect("signature");
    BASE64_URL_SAFE_NO_PAD.encode(sig.as_ref())
}

/// Sign in with the password on a native session. Returns the rotated value.
async fn native_sign_in(app: &Router, native: &str, token: &str, email: &str) -> Reply {
    call(
        app,
        Method::POST,
        "/auth/v1/log_in",
        Some(native),
        None,
        &[("csrf-token", token)],
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await
}

fn native_registration(uid: i32, key: &Key256, nonce: &str) -> Value {
    json!({
        "expected_user_id": uid,
        "device_id": RN_DEVICE,
        "platform": "ios",
        "client": "react_native",
        "form_factor": "phone",
        "key_store": "secure_hardware",
        "public_key": key.jwk,
        "nonce": nonce,
        "signature": sign(key, nonce, RN_DEVICE, uid),
    })
}

fn thread(id: &str) -> Value {
    let stamp = format_stamp(Utc::now().timestamp_millis() as u64, 0, "dev-native");
    json!({
        "kind": "session.upsert", "id": id,
        "identityStamp": stamp, "origin": "index", "scope": "surah:2",
        "startedAt": ISO,
        "cursorStamp": stamp, "surah": 2, "ayah": 1, "device": "phone", "deviceId": RN_DEVICE,
        "lifecycleStamp": stamp, "state": "open",
        "updatedAt": ISO,
    })
}

/// Browser-shaped sign-in: cookie jar and Origin, no native header. Returns the
/// `ruxlog.sid` cookie pair the browser would keep.
async fn browser_sign_in(app: &Router, origin: &str, email: &str) -> String {
    let csrf = call(
        app,
        Method::POST,
        "/csrf/v1/generate",
        None,
        None,
        &[("origin", origin)],
        None,
    )
    .await;
    assert_eq!(csrf.status, StatusCode::OK, "{}", csrf.body);
    let pair = header_str(&csrf, "set-cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let token = csrf.body["token"].as_str().unwrap().to_string();
    let login = call(
        app,
        Method::POST,
        "/auth/v1/log_in",
        None,
        Some(&pair),
        &[("csrf-token", &token), ("origin", origin)],
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    header_str(&login, "set-cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// The whole native contract: CSRF, sign-in (rotation), keyed register, sync,
/// refetch CSRF after rotation, list, delete self, and a 401 afterwards.
#[tokio::test]
async fn native_client_full_flow_with_no_origin_and_no_cookie_jar() {
    let (state, app) = state_and_app().await;
    let account = seed_account(&state, "nativeflow").await;

    // Fresh install: empty header means no session yet; the reply carries one.
    let (token0, native0) = native_csrf(&app, "").await;
    assert!(!native0.is_empty(), "first CSRF reply sets x-eq-session");

    // Sign in. Login cycles the session id, so the value changes and the
    // rotation header says so.
    let login = native_sign_in(&app, &native0, &token0, "nativeflow@example.com").await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    assert_eq!(header_str(&login, "x-eq-session-rotated"), "1");
    let native1 = header_str(&login, SESSION);
    assert!(!native1.is_empty() && native1 != native0, "rotated value");
    assert!(
        login.headers.get(header::SET_COOKIE).is_none(),
        "native replies never carry Set-Cookie"
    );

    // Rotation: refetch CSRF on the new session, then register with a key proof.
    let (token1, _) = native_csrf(&app, &native1).await;
    let challenge = call(
        &app,
        Method::POST,
        "/device/v1/challenge",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(challenge.status, StatusCode::OK, "{}", challenge.body);
    let nonce = challenge.body["nonce"].as_str().unwrap().to_string();
    let key = new_key();
    let register = call(
        &app,
        Method::POST,
        "/device/v1/register",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(native_registration(account.id, &key, &nonce)),
    )
    .await;
    assert_eq!(register.status, StatusCode::OK, "{}", register.body);
    assert_eq!(register.body["device_id"], RN_DEVICE);

    // Sync a v7 thread over the native transport.
    let sync = call(
        &app,
        Method::POST,
        "/reading-session/v1/sync",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({
            "expectedUserId": account.id,
            "since": 0,
            "mutations": [thread("c1c1c1c1-c1c1-7c1c-8c1c-c1c1c1c1c1c1")],
        })),
    )
    .await;
    assert_eq!(sync.status, StatusCode::OK, "{}", sync.body);
    assert_eq!(sync.body["applied"], 1, "{}", sync.body);

    // List devices, then delete this device. Its session ends with it.
    let list = call(
        &app,
        Method::POST,
        "/device/v1/list",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    let devices = list.body["devices"].as_array().unwrap();
    assert!(devices
        .iter()
        .any(|d| d["id"] == RN_DEVICE && d["isCurrent"] == true));

    let delete = call(
        &app,
        Method::POST,
        "/device/v1/delete",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({ "expected_user_id": account.id, "device_id": RN_DEVICE })),
    )
    .await;
    assert_eq!(delete.status, StatusCode::OK, "{}", delete.body);

    let after = call(
        &app,
        Method::POST,
        "/device/v1/list",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED, "{}", after.body);
}

/// A native session with a verified key lives 180 days idle, so it survives 30
/// idle days. A web session keeps the 14-day default.
#[tokio::test]
async fn native_session_outlives_thirty_idle_days_and_web_does_not() {
    let (state, app) = state_and_app().await;
    let account = seed_account(&state, "nativeidle").await;

    let (token0, native0) = native_csrf(&app, "").await;
    let login = native_sign_in(&app, &native0, &token0, "nativeidle@example.com").await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    let native1 = header_str(&login, SESSION);
    let (token1, _) = native_csrf(&app, &native1).await;
    let challenge = call(
        &app,
        Method::POST,
        "/device/v1/challenge",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    let nonce = challenge.body["nonce"].as_str().unwrap().to_string();
    let key = new_key();
    let register = call(
        &app,
        Method::POST,
        "/device/v1/register",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(native_registration(account.id, &key, &nonce)),
    )
    .await;
    assert_eq!(register.status, StatusCode::OK, "{}", register.body);

    let native_record = stored(&state, &native1).await;
    assert!(
        days_from_now(&native_record) > 170.0,
        "native idle lifetime is 180 days, got {}",
        days_from_now(&native_record)
    );
    assert!(
        days_from_now(&native_record) > 30.0,
        "survives 30 idle days"
    );

    // A later native request does not shorten it back to 14 days.
    let list = call(
        &app,
        Method::POST,
        "/device/v1/list",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    assert!(days_from_now(&stored(&state, &native1).await) > 170.0);

    // Web: browser shape (cookie jar, Origin, no header).
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    let web_pair = browser_sign_in(&app, &origin, "nativeidle@example.com").await;
    let web_session = web_pair["ruxlog.sid=".len()..].to_string();
    let web_record = stored(&state, &web_session).await;
    assert!(
        days_from_now(&web_record) < 15.0,
        "web session keeps 14 days, got {}",
        days_from_now(&web_record)
    );
}

/// The header is refused on anything that looks like a browser, whatever its
/// value: Origin, or any Sec-Fetch-* metadata that scripts cannot set.
#[tokio::test]
async fn a_browser_cannot_use_the_native_session_header() {
    let (state, app) = state_and_app().await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();

    for extra in [
        vec![("origin", origin.as_str())],
        vec![
            ("origin", origin.as_str()),
            ("sec-fetch-site", "same-origin"),
        ],
        vec![
            ("sec-fetch-site", "same-origin"),
            ("sec-fetch-mode", "cors"),
        ],
    ] {
        for value in ["", "anything-at-all"] {
            let reply = call(
                &app,
                Method::POST,
                "/csrf/v1/generate",
                Some(value),
                None,
                &extra,
                None,
            )
            .await;
            assert_eq!(reply.status, StatusCode::FORBIDDEN, "{extra:?} {value:?}");
            assert!(
                reply.headers.get(header::SET_COOKIE).is_none(),
                "refused request mints nothing"
            );
            assert_eq!(header_str(&reply, SESSION), "");
        }
    }

    // A browser with no header is untouched: same request, Origin only, works.
    let plain = call(
        &app,
        Method::POST,
        "/csrf/v1/generate",
        None,
        None,
        &[("origin", &origin)],
        None,
    )
    .await;
    assert_eq!(plain.status, StatusCode::OK, "{}", plain.body);
}

/// The header is not in the CORS allow list, so no cross-origin preflight can
/// ask for it, and the private API never exposes it to script.
#[tokio::test]
async fn cors_never_allows_the_native_session_header() {
    let (state, app) = state_and_app().await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    assert!(!cors::private_request_headers()
        .iter()
        .any(|h| h.as_str() == SESSION));

    let preflight = call(
        &app,
        Method::OPTIONS,
        "/csrf/v1/generate",
        None,
        None,
        &[
            ("origin", &origin),
            ("access-control-request-method", "POST"),
            ("access-control-request-headers", "x-eq-session"),
        ],
        None,
    )
    .await;
    let allowed = header_str(&preflight, "access-control-allow-headers");
    assert!(
        !allowed.to_ascii_lowercase().contains(SESSION),
        "preflight allow list: {allowed:?}"
    );
    let exposed = header_str(&preflight, "access-control-expose-headers");
    assert!(!exposed.to_ascii_lowercase().contains(SESSION));
}

/// A value that could break out of the Cookie header is refused, not forwarded.
#[tokio::test]
async fn malformed_native_session_values_are_refused() {
    let (_state, app) = state_and_app().await;
    for bad in ["a;b=c", "a,b", "a\"b"] {
        let reply = call(
            &app,
            Method::POST,
            "/csrf/v1/generate",
            Some(bad),
            None,
            &[],
            None,
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{bad:?}");
    }
}

/// Native clients have no jar. A stale Cookie header is replaced by the value
/// in x-eq-session, never appended to it (devices.md D6, edge row "stale jar").
#[tokio::test]
async fn a_stale_cookie_never_overrides_the_native_session() {
    let (state, app) = state_and_app().await;
    seed_account(&state, "stalejar").await;
    seed_account(&state, "staleother").await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();

    // A browser session for a different account, as a stale jar would hold.
    let stale = browser_sign_in(&app, &origin, "staleother@example.com").await;

    // The native session belongs to "stalejar".
    let (token0, native0) = native_csrf(&app, "").await;
    let login = native_sign_in(&app, &native0, &token0, "stalejar@example.com").await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    let native1 = header_str(&login, SESSION);
    let (token1, _) = native_csrf(&app, &native1).await;

    // Register under the native session while the stale jar rides along.
    let register = call(
        &app,
        Method::POST,
        "/device/v1/register",
        Some(&native1),
        Some(&stale),
        &[("csrf-token", &token1)],
        Some(json!({
            "expected_user_id": login_user_id(&state, "stalejar").await,
            "device_id": RN_DEVICE,
            "platform": "ios",
            "client": "react_native",
            "form_factor": "phone",
            "key_store": "none",
        })),
    )
    .await;
    assert_eq!(register.status, StatusCode::OK, "{}", register.body);

    // The list is the native account's, so the row is there. The other
    // account has no such device.
    let list = call(
        &app,
        Method::POST,
        "/device/v1/list",
        Some(&native1),
        Some(&stale),
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    assert!(list.body["devices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["id"] == RN_DEVICE));

    // A stale jar alone, with the header present but empty, is not a session.
    let jar_only = call(
        &app,
        Method::POST,
        "/device/v1/list",
        Some(""),
        Some(&stale),
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(jar_only.status, StatusCode::UNAUTHORIZED);
}

async fn login_user_id(state: &AppState, name: &str) -> i32 {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    user::Entity::find()
        .filter(user::Column::Email.eq(format!("{name}@example.com")))
        .one(&state.sea_db)
        .await
        .expect("query")
        .expect("account exists")
        .id
}

/// An empty value means no session yet, and the reply carries a fresh one.
#[tokio::test]
async fn empty_native_session_means_no_session_yet() {
    let (_state, app) = state_and_app().await;
    let reply = call(
        &app,
        Method::POST,
        "/csrf/v1/generate",
        Some(""),
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(!header_str(&reply, SESSION).is_empty());
}

/// Revocation ends a native session at once: the next call is 401.
#[tokio::test]
async fn logout_ends_a_native_session_at_once() {
    let (state, app) = state_and_app().await;
    seed_account(&state, "nativelogout").await;

    let (token0, native0) = native_csrf(&app, "").await;
    let login = native_sign_in(&app, &native0, &token0, "nativelogout@example.com").await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    let native1 = header_str(&login, SESSION);
    let (token1, _) = native_csrf(&app, &native1).await;

    let out = call(
        &app,
        Method::POST,
        "/auth/v1/log_out",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        None,
    )
    .await;
    assert_eq!(out.status, StatusCode::OK, "{}", out.body);
    // Logout flushes the session: the header is empty (cleared), and the old
    // value is dead (checked below).
    assert_eq!(header_str(&out, SESSION), "", "logout clears the header");
    assert!(out.headers.get(header::SET_COOKIE).is_none());

    let after = call(
        &app,
        Method::POST,
        "/device/v1/list",
        Some(&native1),
        None,
        &[("csrf-token", &token1)],
        Some(json!({})),
    )
    .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED);
}

/// Sign-up with Turnstile on: a native app has no widget, so it registers with
/// no token; a browser with no token is still refused (devices.md D6). Only
/// register reads Turnstile, so the env set here touches no other test.
#[tokio::test]
async fn native_sign_up_skips_turnstile_and_a_browser_does_not() {
    let (state, app) = state_and_app().await;
    std::env::set_var("TURNSTILE_SECRET_KEY", "test-turnstile-secret");
    // Never reached: neither path below calls siteverify.
    std::env::set_var("TURNSTILE_VERIFY_URL", "http://127.0.0.1:9/siteverify");
    let sign_up = |email: &str| {
        json!({
            "name": "Signup",
            "email": email,
            "password": PASSWORD,
            "date_of_birth": "1990-01-01",
        })
    };

    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    let csrf = call(
        &app,
        Method::POST,
        "/csrf/v1/generate",
        None,
        None,
        &[("origin", &origin)],
        None,
    )
    .await;
    assert_eq!(csrf.status, StatusCode::OK, "{}", csrf.body);
    let pair = header_str(&csrf, "set-cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let web_token = csrf.body["token"].as_str().unwrap().to_string();
    let browser = call(
        &app,
        Method::POST,
        "/auth/v1/register",
        None,
        Some(&pair),
        &[("csrf-token", &web_token), ("origin", &origin)],
        Some(sign_up("browser.signup@gmail.com")),
    )
    .await;
    assert!(browser.status.is_client_error(), "{}", browser.status);
    assert!(
        browser
            .body
            .to_string()
            .contains("Captcha verification required"),
        "{}",
        browser.body
    );

    let (token, native) = native_csrf(&app, "").await;
    let reply = call(
        &app,
        Method::POST,
        "/auth/v1/register",
        Some(&native),
        None,
        &[("csrf-token", &token)],
        Some(sign_up("native.signup@gmail.com")),
    )
    .await;
    assert!(reply.status.is_success(), "{} {}", reply.status, reply.body);
}
