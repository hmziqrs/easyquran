// W8 integration tests. Two concerns live here:
//
// 1. W8D-001 (medium): controller/handler-level tests that drive each OAuth
//    provider's CALLBACK handler (google/apple/facebook/github) through a real
//    axum router and pin the W8d privacy invariant on the redirect the handler
//    actually emits:
//      (a) cancellation query (?error=... | ?error_reason=...) -> the handler
//          short-circuits to the opaque failure redirect `ec=cancel`, with NO
//          code/state/error_description in the Location.
//      (b) success-shape query (?code=...&state=...) -> the handler enters the
//          exchange path (state consumption fails opaquely without a stored
//          CSRF state), still opaque, still no payload leak.
//
//    These are hosted in this integration file (not each provider's #[cfg(test)]
//    module) because the callback handlers take `AuthSession`, which is an
//    axum extractor populated by the session/auth middleware stack and cannot be
//    constructed by hand in a unit test.
//
//    NOTE on the success 307: a true success-path 307 to `/auth/{p}/success`
//    requires a completed upstream token exchange. The oauth2 crate exchanges
//    via `async_http_client` against provider URLs hardcoded in the client
//    builder (not env-redirectable), and Google/Apple verify the id_token
//    signature against a hardcoded JWKS URL. Stubbing that without changing
//    production handlers is impractical, so the success-shape case below pins
//    the entry into the exchange path + the opaque-failure privacy invariant;
//    the success redirect TARGET itself (`/auth/{p}/success`) is already pinned
//    by redirect.rs `success_path_is_provider_relative`.
//
//    NOTE on status: axum 0.8 `Redirect::temporary` emits 307 TEMPORARY_REDIRECT,
//    not 302; tests assert 307 to match production behavior.
//
// 2. W8B-TEST-001 (low): end-to-end integration test that a session-rotating
//    handler emits `X-EQ-Session-Rotated: 1` on its 2xx response. Drives
//    `POST /auth/v1/log_in` through a real router (seeded user + CSRF bootstrap
//    + client-ip resolution), since login_with_metadata cycles the session id
//    (anti session-fixation) and the controller emits the header so the web
//    client refreshes its CSRF token.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    middleware,
    routing::post,
    Extension, Router,
};
use tower::ServiceExt;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};
use ruxlog::middlewares::{client_ip::resolve_client_ip, static_csrf::csrf_guard};
use ruxlog::modules::google_auth_v1;
use ruxlog::modules::{apple_auth_v1, auth_v1, csrf_v1, facebook_auth_v1, github_auth_v1};
use ruxlog::quran::load_quran_store;
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};

use migration::{Migrator, MigratorTrait};
use ruxlog::db::sea_models::user::{self, NewUser, UserRole};
use ruxlog::db::sea_models::user_ban;
use ruxlog::error::ErrorCode;
use ruxlog::services::auth::{AuthBackend, AuthSession};
use ruxlog::services::oauth::{self, OAuthProvider};
use ruxlog::utils::code_hash::hash_code;
use sea_orm::ActiveModelTrait;

// ruxlog is built without cfg(test) when linked into an integration binary, so
// is_production() cannot use cfg(test) to default an unset env. Pin the env to
// development once for the whole process (matches the other integration tests).
static DEV_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_dev_env() {
    DEV_ENV.get_or_init(|| {
        std::env::set_var("RUST_ENV", "development");
        // csrf_guard derives its signing key from COOKIE_KEY; the integration
        // binary is not cfg(test), so there is no fallback key.
        std::env::set_var("COOKIE_KEY", "test-cookie-key-not-for-production-use-32+");
        // build_failure_redirect -> build_allowed_success_redirect needs an
        // allow-listed origin; FRONTEND_URL is the dev default.
        std::env::set_var("FRONTEND_URL", "http://test.local");
    });
}

fn quran_settings() -> QuranSettings {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran");
    QuranSettings {
        uthmani_path: format!("{base}/arabic/quran-uthmani.sqlite"),
        simple_clean_path: format!("{base}/arabic/quran-simple-clean.sqlite"),
        metadata_xml_path: format!("{base}/quran-data.xml"),
        translations_dir: format!("{base}/translations"),
        max_resident_translations: 8,
        max_resident_bytes: 48 * 1024 * 1024,
        translation_idle_ttl_secs: 1800,
    }
}

async fn state() -> AppState {
    ensure_dev_env();
    let quran = Arc::new(
        load_quran_store(&quran_settings())
            .await
            .expect("quran store loads"),
    );
    let sea_db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("in-memory db");
    ruxlog::services::rate_limit_store::ensure_table(&sea_db).await;
    let gate_store = Arc::new(rux_request_gate::InMemoryStore::default());
    let session_store = Arc::new(SqliteSessionStore::new(sea_db.clone()).await);
    let revoked_sessions = Arc::new(Mutex::new(HashSet::new()));
    let mailer = Arc::new(MailRouter::new(
        HashMap::new(),
        String::new(),
        gate_store.clone(),
        sea_db.clone(),
        MailRouterLimits::default(),
        false,
    ));
    let settings = Arc::new(Settings {
        cookie_key: "test_cookie_key_padded_to_more_than_32_bytes_for_tests".into(),
        http: HttpSettings {
            host: "0.0.0.0".into(),
            port: "0".into(),
            ip_source: "ConnectInfo".parse().unwrap(),
            cookie_secure: false,
        },
        site: SiteSettings {
            url: "http://localhost".into(),
            name: "test".into(),
            consumer_site_url: "http://localhost".into(),
        },
        object_storage: ObjectStorageConfig {
            region: "auto".into(),
            account_id: "test".into(),
            bucket: "test".into(),
            access_key: "test".into(),
            secret_key: "test".into(),
            public_url: "http://localhost.invalid".into(),
            endpoint: "http://localhost.invalid".into(),
        },
        optimizer: OptimizerConfig {
            enabled: false,
            max_pixels: 1,
            keep_original: false,
            default_webp_quality: 80,
        },
        rate_limit: RateLimitSettings {
            active_ban_max: 2_000,
            ban_export_token: String::new(),
            internal_token: String::new(),
            internal_requests_per_minute: 600,
            health_requests_per_minute: 120,
        },
        quran: quran_settings(),
    });
    let storage = StorageState {
        config: settings.object_storage.clone(),
        client: aws_sdk_s3::Client::new(&aws_config::SdkConfig::builder().build()),
        optimizer: settings.optimizer.clone(),
        image_moderator: None,
    };
    let billing_router = Arc::new(BillingRouter::new(
        HashMap::new(),
        GeoRouter::new(GeoRulesConfig {
            default_provider: String::new(),
            rules: vec![],
        }),
    ));
    let translation_pool = {
        let qs = quran_settings();
        let catalogue_path = format!("{}/index.min.json", qs.translations_dir);
        let cat = ruxlog::quran::load_catalogue(&catalogue_path)
            .await
            .expect("translation catalogue loads");
        Arc::new(ruxlog::quran::TranslationPool::new(
            &cat,
            std::path::PathBuf::from(&qs.translations_dir),
            qs.max_resident_translations,
            qs.max_resident_bytes,
            std::time::Duration::from_secs(qs.translation_idle_ttl_secs),
            true,
            2,
        ))
    };

    AppState {
        sea_db,
        gate_store,
        session_store,
        revoked_sessions,
        mailer,
        settings,
        allowed_origins: ruxlog::utils::cors::build_allowed_origins(false, None, None, None)
            .expect("dev default origins parse"),
        storage,
        secret_key: b"test_secret_key".to_vec(),
        http_client: build_http_client(),
        billing_router,
        fcm: None,
        webauthn: None,
        quran,
        quran_runtime_metrics: QuranRuntimeMetrics {
            arabic_load_duration_ms: 7,
            translation_catalogue_load_duration_ms: 3,
            translation_catalogue_entries: 115,
        },
        quran_scripts: Arc::new(tokio::sync::Mutex::new(None)),
        translation_pool,
        quran_sources: Arc::new(tokio::sync::Mutex::new(None)),
    }
}

// ============================================================================
// W8D-001: OAuth callback handler tests
// ============================================================================

// Minimal router mounting all four provider callback routes + the session layer
// (AuthSession extractor needs a tower Session). No CSRF layer: the callbacks
// are GET (csrf_guard exempts safe methods). No web_auth gate: the routes are
// mounted directly so the test is independent of the boot-time WEB_AUTH_ENABLED
// OnceLock. The cancellation branch touches no DB and no upstream HTTP; the
// success-shape branch fails at CSRF-state consumption before any exchange.
fn oauth_callback_router(state: AppState) -> Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    Router::new()
        .nest("/auth/google/v1", google_auth_v1::routes())
        .nest("/auth/apple/v1", apple_auth_v1::routes())
        .nest("/auth/facebook/v1", facebook_auth_v1::routes())
        .nest("/auth/github/v1", github_auth_v1::routes())
        .layer(session_layer)
        .with_state(state)
}

// Privacy invariant shared by every redirecting callback branch: the Location
// carries only the opaque failure code, never the provider-supplied payload.
fn assert_opaque_failure(location: &str, provider: &str, expected_ec: &str) {
    let path = location.split('?').next().unwrap_or(location);
    assert!(
        path.ends_with(&format!("/auth/{provider}/failure")),
        "cancellation must redirect to the {provider} failure path, got: {location}"
    );
    // ec is the ONLY allowed query param.
    assert!(
        location.contains(&format!("ec={expected_ec}")),
        "expected ec={expected_ec} in {location}"
    );
    assert!(
        !location.contains("code="),
        "authorization code must not leak into the failure Location: {location}"
    );
    assert!(
        !location.contains("state="),
        "CSRF state must not leak into the failure Location: {location}"
    );
    assert!(
        !location.to_ascii_lowercase().contains("error"),
        "provider error params must not leak into the failure Location: {location}"
    );
    assert!(
        !location.contains("token"),
        "no token material must leak into the failure Location: {location}"
    );
}

async fn get_redirect(uri: &str) -> (StatusCode, Option<String>) {
    let app = oauth_callback_router(state().await);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    (status, location)
}

#[tokio::test]
async fn google_callback_cancellation_redirects_opaque() {
    let (status, location) = get_redirect("/auth/google/v1/callback?error=access_denied").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("cancellation emits a Location");
    assert_opaque_failure(&location, "google", "cancel");
}

#[tokio::test]
async fn google_callback_success_shape_enters_exchange_opaquely() {
    // Success-shape query reaches the handler (not short-circuited as cancel);
    // without a stored CSRF state the exchange fails, and the failure redirect
    // is still opaque. Sensitive values must never reach the Location.
    let (status, location) =
        get_redirect("/auth/google/v1/callback?code=SENSITIVE_AUTH_CODE&state=SENSITIVE_STATE")
            .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("failure emits a Location");
    let ec = location.split("ec=").nth(1).unwrap_or_default();
    assert!(
        ec == "auth" || ec == "server",
        "success-shape must enter the exchange path and fail with ec=auth|server, got: {location}"
    );
    assert_ne!(
        ec, "cancel",
        "success-shape must not be treated as cancellation"
    );
    assert!(
        !location.contains("SENSITIVE_AUTH_CODE") && !location.contains("SENSITIVE_STATE"),
        "code/state must not leak into the Location: {location}"
    );
}

#[tokio::test]
async fn apple_callback_cancellation_redirects_opaque() {
    let (status, location) = get_redirect("/auth/apple/v1/callback?error=user_cancelled").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("cancellation emits a Location");
    assert_opaque_failure(&location, "apple", "cancel");
}

#[tokio::test]
async fn apple_callback_success_shape_enters_exchange_opaquely() {
    let (status, location) =
        get_redirect("/auth/apple/v1/callback?code=SENSITIVE_AUTH_CODE&state=SENSITIVE_STATE")
            .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("failure emits a Location");
    let ec = location.split("ec=").nth(1).unwrap_or_default();
    assert!(
        ec == "auth" || ec == "server",
        "success-shape must enter the exchange path and fail with ec=auth|server, got: {location}"
    );
    assert_ne!(ec, "cancel");
    assert!(
        !location.contains("SENSITIVE_AUTH_CODE") && !location.contains("SENSITIVE_STATE"),
        "code/state must not leak into the Location: {location}"
    );
}

#[tokio::test]
async fn facebook_callback_cancellation_redirects_opaque() {
    // Facebook Graph redirect style: ?error_reason=user_denied (no OAuth2 `error`).
    let (status, location) =
        get_redirect("/auth/facebook/v1/callback?error_reason=user_denied").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("cancellation emits a Location");
    assert_opaque_failure(&location, "facebook", "cancel");
}

#[tokio::test]
async fn facebook_callback_success_shape_enters_exchange_opaquely() {
    let (status, location) =
        get_redirect("/auth/facebook/v1/callback?code=SENSITIVE_AUTH_CODE&state=SENSITIVE_STATE")
            .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("failure emits a Location");
    let ec = location.split("ec=").nth(1).unwrap_or_default();
    assert!(
        ec == "auth" || ec == "server",
        "success-shape must enter the exchange path and fail with ec=auth|server, got: {location}"
    );
    assert_ne!(ec, "cancel");
    assert!(
        !location.contains("SENSITIVE_AUTH_CODE") && !location.contains("SENSITIVE_STATE"),
        "code/state must not leak into the Location: {location}"
    );
}

#[tokio::test]
async fn github_callback_cancellation_redirects_opaque() {
    let (status, location) = get_redirect("/auth/github/v1/callback?error=access_denied").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("cancellation emits a Location");
    assert_opaque_failure(&location, "github", "cancel");
}

#[tokio::test]
async fn github_callback_success_shape_enters_exchange_opaquely() {
    let (status, location) =
        get_redirect("/auth/github/v1/callback?code=SENSITIVE_AUTH_CODE&state=SENSITIVE_STATE")
            .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    let location = location.expect("failure emits a Location");
    let ec = location.split("ec=").nth(1).unwrap_or_default();
    assert!(
        ec == "auth" || ec == "server",
        "success-shape must enter the exchange path and fail with ec=auth|server, got: {location}"
    );
    assert_ne!(ec, "cancel");
    assert!(
        !location.contains("SENSITIVE_AUTH_CODE") && !location.contains("SENSITIVE_STATE"),
        "code/state must not leak into the Location: {location}"
    );
}

// ============================================================================
// W8B-TEST-001: session-rotation header integration test
// ============================================================================

// Real auth_v1 router + the wrapping layers production uses: SessionManagerLayer
// (Session + AuthSession), csrf_guard (POST /log_in is not CSRF-exempt), and the
// client-ip resolver (log_in's ClientIp extractor). Routes are mounted directly
// so the test is independent of the boot-time WEB_AUTH_ENABLED OnceLock.
fn login_router(state: AppState) -> Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    let state_ext = state.clone();
    Router::new()
        .nest("/auth/v1", auth_v1::routes())
        .route("/csrf/v1/generate", post(csrf_v1::controller::generate))
        .layer(middleware::from_fn(csrf_guard))
        .layer(middleware::from_fn(resolve_client_ip))
        .layer(Extension(axum_client_ip::ClientIpSource::CfConnectingIp))
        .layer(axum::Extension(state_ext))
        .layer(session_layer)
        .with_state(state)
}

// One CSRF bootstrap: POST /csrf/v1/generate materializes a tower session and
// returns the binding token. Mirrors the helper in tests/security_tests.rs.
async fn csrf_bootstrap(app: &Router) -> (String, String) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/csrf/v1/generate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "csrf generate must succeed");
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("generate sets a session cookie")
        .to_str()
        .unwrap()
        .to_string();
    // Use the full `name=value` pair verbatim (a signed/private cookie value may
    // contain '=' base64 padding; rebuilding from split parts would truncate it).
    let pair = set_cookie.split(';').next().unwrap_or(&set_cookie).trim();

    let bytes = to_bytes(res.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = json["token"].as_str().unwrap().to_string();
    (pair.to_string(), token)
}

async fn seed_user(state: &AppState, email: &str, password: &str) {
    Migrator::up(&state.sea_db, None)
        .await
        .expect("schema migrates");
    // email_verification row is created alongside the user; pass a dummy hash.
    let code_hash = hash_code(&state.secret_key, "dummy-verification-code");
    user::Entity::create(
        &state.sea_db,
        NewUser {
            name: email.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            role: UserRole::User,
        },
        code_hash,
    )
    .await
    .expect("seed user created");
}

// W8B-001: login_with_metadata cycles the session id (anti session-fixation),
// so the controller emits X-EQ-Session-Rotated on the 2xx response — the web
// client refreshes its in-memory CSRF token iff this header is present. Drive
// the real POST /auth/v1/log_in handler end-to-end and assert the header.
#[tokio::test]
async fn login_emits_session_rotated_header() {
    let state = state().await;
    let email = "rotation-test@test.local";
    let password = "super-secret-password";
    seed_user(&state, email, password).await;

    let app = login_router(state);
    let (cookie, token) = csrf_bootstrap(&app).await;

    let body = serde_json::json!({
        "email": email,
        "password": password,
    });
    let body_bytes = body.to_string();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/v1/log_in")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONTENT_LENGTH, body_bytes.len())
                .header(header::COOKIE, cookie)
                .header("csrf-token", token)
                // ClientIpSource::CfConnectingIp resolves the extractor; without
                // it log_in's ClientIp rejection short-circuits the handler.
                .header("cf-connecting-ip", "203.0.113.50")
                .body(Body::from(body_bytes))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = res.status();
    let rotated = res
        .headers()
        .get("x-eq-session-rotated")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if status != StatusCode::OK {
        let body = to_bytes(res.into_body(), 8192).await.unwrap_or_default();
        panic!(
            "login must succeed (seeded user + valid CSRF); got {status}, body: {}",
            String::from_utf8_lossy(&body)
        );
    }
    assert_eq!(
        rotated.as_deref(),
        Some("1"),
        "a successful login rotates the session and must emit X-EQ-Session-Rotated: 1"
    );
}

// ============================================================================
// W8D-001: OAuth ban-guard tests
// ============================================================================

// finish_oauth_login cannot be reached through a provider callback in a test
// (the success path needs a real upstream token exchange + JWKS signature
// verification — see the NOTE at the top of this file). So the guard is driven
// directly: a real AuthSession over the in-memory DB + SqliteSessionStore,
// exactly the object the four providers hand it. This exercises the real
// fail-closed check_ban call site plus the full success path (auth.login +
// create_bound_session) for the non-banned case.

async fn seed_user_model(state: &AppState, email: &str) -> user::Model {
    seed_user(state, email, "super-secret-password").await;
    user::Entity::find_by_email(&state.sea_db, email.to_string())
        .await
        .expect("find seeded user")
        .expect("seeded user exists")
}

async fn build_auth(state: &AppState) -> AuthSession {
    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    // Lazy session — no store I/O until auth.login touches it.
    let session = tower_sessions::Session::new(None, state.session_store.clone(), None);
    AuthSession::new(backend, session).await
}

#[tokio::test]
async fn oauth_login_rejects_banned_user() {
    let state = state().await;
    let user = seed_user_model(&state, "oauth-banned@test.local").await;

    // Active ban: no expiry, not revoked -> check_ban returns is_banned().
    let now = chrono::Utc::now().fixed_offset();
    user_ban::ActiveModel {
        user_id: sea_orm::Set(user.id),
        reason: sea_orm::Set(Some("abuse".to_string())),
        banned_by: sea_orm::Set(None),
        expires_at: sea_orm::Set(None),
        created_at: sea_orm::Set(now),
        revoked_at: sea_orm::Set(None),
        revoked_by: sea_orm::Set(None),
        ..Default::default()
    }
    .insert(&state.sea_db)
    .await
    .expect("ban row inserted");

    let mut auth = build_auth(&state).await;
    let err = oauth::finish_oauth_login(&state, &mut auth, &user, OAuthProvider::Github)
        .await
        .expect_err("banned user must not obtain an OAuth session");
    assert_eq!(
        err.code,
        ErrorCode::AccountLocked,
        "banned OAuth login must surface AccountLocked"
    );
    assert!(
        err.message.contains("banned"),
        "expected the same ban message password-login returns, got: {}",
        err.message
    );
}

#[tokio::test]
async fn oauth_login_allows_non_banned_user() {
    let state = state().await;
    let user = seed_user_model(&state, "oauth-clean@test.local").await;

    let mut auth = build_auth(&state).await;
    oauth::finish_oauth_login(&state, &mut auth, &user, OAuthProvider::Github)
        .await
        .expect("non-banned user must complete OAuth login (guard must not fire)");
}
