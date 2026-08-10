// W3C-001: HTTP-level guard that the read-only BAN_EXPORT_TOKEN is accepted ONLY
// by /admin/bans/export and can never authorize a mutation (DELETE /admin/bans),
// which sits behind the session admin ACL. An invalid bearer is rejected (401 +
// the admin-bans challenge) without falling through to the session path.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};
use ruxlog::modules::admin_bans_v1;
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

const EXPORT_TOKEN: &str = "test-export-secret-token-xyz";

// ruxlog is built without cfg(test) when linked into an integration test
// binary; pin the env once so is_production() takes the dev path.
static DEV_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_dev_env() {
    DEV_ENV.get_or_init(|| std::env::set_var("RUST_ENV", "development"));
}

fn quran_settings() -> QuranSettings {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran/tanzil");
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
            ban_export_token: EXPORT_TOKEN.to_string(),
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

// Real admin_bans_v1 router + the same wrapping layers production uses:
// SessionManagerLayer (provides Session) + Extension<AppState> (the from_fn
// guard extracts both). No CSRF layer here so the rejection under test is the
// auth guard alone, not CSRF.
fn router_with_state(state: AppState) -> axum::Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    admin_bans_v1::routes()
        .layer(axum::Extension(state.clone()))
        .layer(session_layer)
        .with_state(state)
}

async fn app() -> axum::Router {
    router_with_state(state().await)
}

fn is_auth_rejection(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

#[tokio::test]
async fn invalid_export_bearer_token_rejected_on_export_route() {
    let app = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/bans/export")
                .header(
                    "authorization",
                    "Bearer definitely-not-the-configured-token",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // The guard rejects an invalid bearer without falling through to the
    // session check, so a token guess cannot be probed via the session path.
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        res.headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok()),
        Some("Bearer realm=\"admin-bans\""),
        "rejection must carry the admin-bans bearer challenge"
    );
}

#[tokio::test]
async fn valid_export_token_authorizes_export_route() {
    let app = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/bans/export")
                .header("authorization", format!("Bearer {EXPORT_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a valid export token must reach the read-only export handler"
    );
}

#[tokio::test]
async fn no_credentials_rejected_on_export_route() {
    let app = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/bans/export")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        is_auth_rejection(res.status()),
        "no bearer + no admin session must be rejected, got {}",
        res.status()
    );
}

#[tokio::test]
async fn export_token_does_not_authorize_delete_mutation() {
    // The DELETE route is on the session_admin router (verified_with_role<
    // ROLE_ADMIN>); the export token guard is NEVER installed there. A request
    // bearing the valid export token but no admin session must be rejected —
    // proving the read-only token cannot mutate bans.
    let app = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/admin/bans")
                .header("authorization", format!("Bearer {EXPORT_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        is_auth_rejection(res.status()),
        "export token must NOT grant mutation access on DELETE /admin/bans; got {}",
        res.status()
    );
    assert_ne!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn export_token_does_not_authorize_list_route() {
    // GET /admin/bans (list) is also on the session_admin router, so the export
    // token must not unlock it either — only /admin/bans/export accepts it.
    let app = app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/bans")
                .header("authorization", format!("Bearer {EXPORT_TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        is_auth_rejection(res.status()),
        "export token must NOT grant list access on GET /admin/bans; got {}",
        res.status()
    );
}

#[tokio::test]
async fn admin_session_fallthrough_authorizes_export_route() {
    // Positive coverage of the human-access branch of admin_or_export_token: a
    // request carrying a valid admin SESSION cookie but NO bearer token must reach
    // the read-only export handler. The sibling tests cover the export-TOKEN path
    // (valid accepted, invalid rejected, never honored on mutation/list routes);
    // this one proves the session-ACL fallthrough succeeds for a real admin.
    let state = state().await;

    // The fallthrough session check reaches the DB (get_user + check_ban), so apply the
    // real schema and seed an admin directly through the sea-orm model (before_save
    // backfills the per-user session_auth_secret that binds the session).
    Migrator::up(&state.sea_db, None)
        .await
        .expect("migrations apply");
    let now = chrono::Utc::now().fixed_offset();
    let admin = user::ActiveModel {
        name: sea_orm::Set("export-admin".into()),
        email: sea_orm::Set("export-admin@example.com".into()),
        password: sea_orm::Set(None),
        role: sea_orm::Set(UserRole::Admin),
        is_verified: sea_orm::Set(true),
        two_fa_enabled: sea_orm::Set(false),
        created_at: sea_orm::Set(now),
        updated_at: sea_orm::Set(now),
        ..Default::default()
    }
    .insert(&state.sea_db)
    .await
    .expect("admin user seeded");

    // Establish a real authenticated session in the SAME store the router uses, via the
    // real AuthSession::login path. This populates the `rux_auth` session key and binds
    // session_auth_hash so the fallthrough branch authenticates on the next request.
    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    let session = tower_sessions::Session::new(None, state.session_store.clone(), None);
    let mut auth = GenAuthSession::new(backend, session).await;
    auth.login(&admin).await.expect("admin logs in");
    auth.session().save().await.expect("session persisted");
    let session_id = auth
        .session()
        .id()
        .expect("session has an id after login + save")
        .to_string();
    drop(auth);

    // The router reads the session id from a PRIVATE ruxlog.sid cookie. Mint that cookie
    // with the same key the SessionManagerLayer derives, so the request carries a genuine
    // signed + encrypted session id — the value is opaque to the client, exactly as in
    // production. (No route in this isolated router establishes a session on its own, so
    // we forge the cookie the layer would otherwise issue on a Set-Cookie response.)
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let mut jar = tower_sessions::cookie::CookieJar::new();
    jar.private_mut(&cookie_key)
        .add(tower_sessions::cookie::Cookie::new(
            "ruxlog.sid",
            session_id,
        ));
    let cookie_header = jar
        .get("ruxlog.sid")
        .expect("forged session cookie present")
        .to_string();

    let app = router_with_state(state);
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/admin/bans/export")
                .header("cookie", &cookie_header)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "a valid admin session (no bearer) must reach the export handler via the fallthrough branch"
    );
}
