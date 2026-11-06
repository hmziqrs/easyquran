// Minor flag over HTTP (A34): a self-declared under-18 account gets no device
// record, no push token, no stored reading events and no comments. The flag
// persists on the user row and is read back at every gated path.
//
// Compile-verified only in the battery build; run with the usual gates when on
// mains power.
mod reading_common;

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Method, Request, StatusCode},
    middleware, Extension, Router,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{json, Value};
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_sessions::cookie::Key;
use tower_sessions::SessionManagerLayer;

use ruxlog::db::sea_models::user::{NewUser, UserRole};
use ruxlog::db::sea_models::{device, user, user_device};
use ruxlog::middlewares::{client_ip::resolve_client_ip, cors, native_session, static_csrf};
use ruxlog::modules::auth_v1;
use ruxlog::router;
use ruxlog::state::AppState;
use ruxlog::utils::code_hash::hash_code;

const PASSWORD: &str = "minor-flag-test-password";
const DEVICE: &str = "0190f2c4-7b1a-7c3d-9e4f-0123456789ab";
const PUSH_TOKEN: &str = "minor-flag-push-token-fixture-0123456789abcdef";

fn app_for(state: AppState) -> Router {
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

static COOKIE_KEY_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();

async fn state_and_app() -> (AppState, Router) {
    COOKIE_KEY_ENV.get_or_init(|| {
        std::env::set_var("COOKIE_KEY", "test-cookie-key-not-for-production-use-32+");
    });
    let state = reading_common::state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let app = app_for(state.clone());
    (state, app)
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    cookie: Option<&str>,
    extra: &[(&str, &str)],
    body: Option<Value>,
) -> Reply {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("cf-connecting-ip", "203.0.113.9");
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

fn set_cookie(reply: &Reply) -> String {
    reply
        .headers
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// Verified account with a password, created through the real user path.
/// `is_minor` is the flag under test.
async fn seed(state: &AppState, name: &str, is_minor: bool) -> user::Model {
    let code_hash = hash_code(&state.secret_key, "dummy-verification-code");
    let created = user::Entity::create(
        &state.sea_db,
        NewUser {
            name: name.into(),
            email: format!("{name}@example.com"),
            password: PASSWORD.into(),
            role: UserRole::User,
            is_minor,
        },
        code_hash,
    )
    .await
    .expect("account created");
    user::Entity::verify(&state.sea_db, created.id)
        .await
        .expect("account verified")
}

/// Browser-shaped sign-in: CSRF pair, then log in. Returns the session cookie.
async fn sign_in(app: &Router, origin: &str, email: &str) -> (String, String) {
    let csrf = call(
        app,
        Method::POST,
        "/csrf/v1/generate",
        None,
        &[("origin", origin)],
        None,
    )
    .await;
    assert_eq!(csrf.status, StatusCode::OK, "{}", csrf.body);
    let pair = set_cookie(&csrf);
    let token = csrf.body["token"].as_str().unwrap().to_string();
    let login = call(
        app,
        Method::POST,
        "/auth/v1/log_in",
        Some(&pair),
        &[("csrf-token", &token), ("origin", origin)],
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    (set_cookie(&login), token)
}

#[tokio::test]
async fn minor_flag_persists_on_the_user_row() {
    let (state, _app) = state_and_app().await;
    let minor = seed(&state, "flagminor", true).await;
    let adult = seed(&state, "flagadult", false).await;
    assert!(minor.is_minor, "flag set at create");
    assert!(!adult.is_minor, "adult not flagged");
    let reloaded = user::Entity::find_by_id(minor.id)
        .one(&state.sea_db)
        .await
        .unwrap()
        .expect("row");
    assert!(reloaded.is_minor, "flag read back from the database");
}

#[tokio::test]
async fn minor_device_register_writes_no_device_row_and_no_push_token() {
    let (state, app) = state_and_app().await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    let minor = seed(&state, "devminor", true).await;
    let (cookie, token) = sign_in(&app, &origin, "devminor@example.com").await;

    let reply = call(
        &app,
        Method::POST,
        "/device/v1/register",
        Some(&cookie),
        &[("csrf-token", &token), ("origin", &origin)],
        Some(json!({
            "expected_user_id": minor.id,
            "device_id": DEVICE,
            "platform": "ios",
            "client": "react_native",
            "form_factor": "phone",
            "key_store": "secure_hardware",
            "token": PUSH_TOKEN,
        })),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_ne!(
        reply.body["device_id"].as_str(),
        Some(DEVICE),
        "minor gets a fresh random id, not the client id"
    );

    let rows = device::Entity::find()
        .filter(device::Column::UserId.eq(minor.id))
        .count(&state.sea_db)
        .await
        .unwrap();
    assert_eq!(rows, 0, "no device row for a minor");

    let tokens = device::Entity::find()
        .filter(device::Column::Token.eq(PUSH_TOKEN))
        .count(&state.sea_db)
        .await
        .unwrap();
    assert_eq!(tokens, 0, "no push token stored for a minor");

    let salts = user_device::Entity::find()
        .filter(user_device::Column::UserId.eq(minor.id))
        .count(&state.sea_db)
        .await
        .unwrap();
    assert_eq!(salts, 0, "no device salt written for a minor");
}

#[tokio::test]
async fn minor_device_challenge_is_refused() {
    let (state, app) = state_and_app().await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    let minor = seed(&state, "chalminor", true).await;
    let (cookie, token) = sign_in(&app, &origin, "chalminor@example.com").await;
    let reply = call(
        &app,
        Method::POST,
        "/device/v1/challenge",
        Some(&cookie),
        &[("csrf-token", &token), ("origin", &origin)],
        Some(json!({})),
    )
    .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);
    let _ = minor;
}

#[tokio::test]
async fn minor_analytics_events_store_nothing() {
    let (state, app) = state_and_app().await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    let minor = seed(&state, "anaminor", true).await;
    let (cookie, token) = sign_in(&app, &origin, "anaminor@example.com").await;
    let reply = call(
        &app,
        Method::POST,
        "/user/analytics/v1/events",
        Some(&cookie),
        &[("csrf-token", &token), ("origin", &origin)],
        Some(json!({
            "expected_user_id": minor.id,
            "events": [{
                "id": DEVICE,
                "surah": 1,
                "ayah": 1,
                "ms": 1000,
                "local_day": "2026-11-01",
                "at": "2026-11-01T00:00:00.000Z",
            }],
        })),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(
        reply.body["accepted"],
        json!(0),
        "minor batch accepted as 0"
    );
}

#[tokio::test]
async fn minor_comment_create_is_refused() {
    let (state, app) = state_and_app().await;
    let origin = state.allowed_origins.header_values()[0]
        .to_str()
        .unwrap()
        .to_string();
    seed(&state, "cmtminor", true).await;
    let (cookie, token) = sign_in(&app, &origin, "cmtminor@example.com").await;
    let reply = call(
        &app,
        Method::POST,
        "/post/comment/v1/create",
        Some(&cookie),
        &[("csrf-token", &token), ("origin", &origin)],
        Some(json!({ "post_id": 1, "content": "minor comment fixture" })),
    )
    .await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);
}
