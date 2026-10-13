//! Shared harness for the reading-model integration tests (bookmarks,
//! reading sessions, reading insights): a real AppState over in-memory SQLite
//! with every migration applied, seeded users, and forged session cookies.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};
use ruxlog::quran::load_quran_store;
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};

use migration::{Migrator, MigratorTrait};
use rux_auth::AuthSession as GenAuthSession;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::services::auth::AuthBackend;
use sea_orm::ActiveModelTrait;

// ruxlog is built without cfg(test) when linked into an integration test
// binary; pin the env once so is_production() takes the dev path.
static DEV_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_dev_env() {
    DEV_ENV.get_or_init(|| std::env::set_var("APP_ENV", "development"));
}

fn quran_settings() -> QuranSettings {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran");
    QuranSettings {
        uthmani_path: format!("{base}/arabic/quran-uthmani.sqlite"),
        simple_clean_path: format!("{base}/arabic/quran-simple-clean.sqlite"),
        indopak_path: format!("{base}/arabic/quran-indopak.sqlite"),
        tajweed_path: format!("{base}/arabic/quran-tajweed.sqlite"),
        metadata_xml_path: format!("{base}/quran-data.xml"),
        translations_dir: format!("{base}/translations"),
        max_resident_translations: 8,
        max_resident_bytes: 48 * 1024 * 1024,
        translation_idle_ttl_secs: 1800,
    }
}

pub async fn state() -> AppState {
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
            trusted_proxy_cidrs: Vec::new(),
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
            ban_export_token: "unused".to_string(),
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

// Real reading_session_v1 router + the wrapping layers production uses
// (SessionManagerLayer + Extension<AppState>); no CSRF layer, same as the
// bookmarks_v1 harness, so failures under test are the auth guard / handler
// alone.
pub fn router_with_state(state: AppState, routes: axum::Router<AppState>) -> axum::Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    routes
        .layer(axum::Extension(state.clone()))
        .layer(session_layer)
        .with_state(state)
}

pub async fn seed_user(state: &AppState, name: &str) -> user::Model {
    let now = chrono::Utc::now().fixed_offset();
    user::ActiveModel {
        name: sea_orm::Set(name.into()),
        email: sea_orm::Set(format!("{name}@example.com")),
        password: sea_orm::Set(None),
        role: sea_orm::Set(UserRole::User),
        is_verified: sea_orm::Set(true),
        two_fa_enabled: sea_orm::Set(false),
        created_at: sea_orm::Set(now),
        updated_at: sea_orm::Set(now),
        ..Default::default()
    }
    .insert(&state.sea_db)
    .await
    .expect("user seeded")
}

// Establish a real authenticated session in the SAME store the router uses,
// via the real AuthSession::login path, then mint the PRIVATE ruxlog.sid
// cookie the SessionManagerLayer would issue (see bookmarks_v1.rs).
pub async fn login_cookie(state: &AppState, u: &user::Model) -> String {
    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    let session = tower_sessions::Session::new(None, state.session_store.clone(), None);
    let mut auth = GenAuthSession::new(backend, session).await;
    auth.login(u).await.expect("login succeeds");
    auth.session().save().await.expect("session persisted");
    let session_id = auth
        .session()
        .id()
        .expect("session has an id after login + save")
        .to_string();
    drop(auth);

    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let mut jar = tower_sessions::cookie::CookieJar::new();
    jar.private_mut(&cookie_key)
        .add(tower_sessions::cookie::Cookie::new(
            "ruxlog.sid",
            session_id,
        ));
    jar.get("ruxlog.sid")
        .expect("forged session cookie present")
        .to_string()
}

/// One migrated state with two signed-in accounts on the given routes.
pub struct Harness {
    pub app: axum::Router,
    pub state: AppState,
    pub cookie_a: String,
    pub uid_a: i32,
    pub cookie_b: String,
    pub uid_b: i32,
}

pub async fn harness(routes: axum::Router<AppState>) -> Harness {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let a = seed_user(&state, "readera").await;
    let b = seed_user(&state, "readerb").await;
    let cookie_a = login_cookie(&state, &a).await;
    let cookie_b = login_cookie(&state, &b).await;
    Harness {
        app: router_with_state(state.clone(), routes),
        state,
        cookie_a,
        uid_a: a.id,
        cookie_b,
        uid_b: b.id,
    }
}

pub async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(b).unwrap()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
