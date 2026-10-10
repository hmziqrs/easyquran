//! Production script-availability gate (campaign decision: production serves the
//! Uthmani family only; IndoPak + Tajweed are refused and never advertised).
//!
//! Lives in its own test binary because APP_ENV is process-wide: this file pins
//! production for every test, while `quran_v1.rs` keeps exercising the all-four
//! non-production behavior. Separate binaries keep the two env classes from
//! racing across parallel test threads (the in-crate `TEST_ENV_MUTEX` is
//! cfg(test)-only and cannot be reached from an integration test).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};
use ruxlog::modules::quran_v1;
use ruxlog::quran::load_quran_store;
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};

// Same pattern as quran_v1.rs::ensure_dev_env, pinned to production: set once
// per process so every test thread observes the same APP_ENV.
static PROD_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_prod_env() {
    PROD_ENV.get_or_init(|| std::env::set_var("APP_ENV", "production"));
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

async fn state_with_public_url(public_url: &str) -> AppState {
    ensure_prod_env();
    let quran = Arc::new(
        load_quran_store(&quran_settings())
            .await
            .expect("quran store loads"),
    );
    let sea_db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("in-memory db");
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
            public_url: public_url.into(),
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
            translation_catalogue_entries: translation_pool.catalogue().len() as u64,
        },
        quran_scripts: Arc::new(tokio::sync::Mutex::new(None)),
        translation_pool,
        quran_sources: Arc::new(tokio::sync::Mutex::new(None)),
    }
}

/// Slimmer than quran_v1.rs's `app_over`: no client-IP/rate-limit layers, whose
/// production branch requires a CF-Connecting-IP identity this harness does not
/// carry. The quran handlers themselves are what these tests exercise.
fn app_over(state: AppState) -> axum::Router {
    let quran = quran_v1::routes().merge(quran_v1::search_route());
    axum::Router::new().nest("/quran", quran).with_state(state)
}

async fn send(app: &axum::Router, uri: &str) -> (StatusCode, Value, HeaderMap) {
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, body, headers)
}

fn error_message(body: &Value) -> &str {
    body["error"]["message"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn prod_refuses_gated_scripts_and_serves_uthmani_family() {
    let state = state_with_public_url("http://localhost.invalid").await;
    let app = app_over(state);

    // Source routes: indopak/tajweed must be refused with the explicit
    // production-disabled message, not silently resolved or 404'd.
    for uri in [
        "/quran/sources/indopak/surah/2",
        "/quran/sources/tajweed/surah/2",
        "/quran/sources/indopak/range?from=1&to=7",
        "/quran/sources/tajweed/range?from=1&to=7",
    ] {
        let (status, body, _) = send(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(body["error"]["code"], "invalid_input", "{uri}");
        let message = error_message(&body);
        assert!(
            message.contains("disabled in production"),
            "{uri} must say the script is disabled in production; got: {message}"
        );
    }

    // Script query routes (range + single ayah + families + search).
    for uri in [
        "/quran/surahs/2/ayahs?script=indopak",
        "/quran/surahs/2/ayahs?script=tajweed",
        "/quran/juzs/1/ayahs?script=indopak",
        "/quran/ayahs/1/1?script=tajweed",
        "/quran/ayahs?keys=1:1&script=indopak",
        "/quran/search?q=%D8%A7%D9%84%D8%AD%D9%85%D8%AF&script=tajweed",
    ] {
        let (status, body, _) = send(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(body["error"]["code"], "invalid_input", "{uri}");
        assert!(
            error_message(&body).contains("disabled in production"),
            "{uri} must say the script is disabled in production; got: {}",
            error_message(&body)
        );
    }

    // Production must not advertise the gated ids even in validation messages.
    for uri in [
        "/quran/ayahs/1/1?script=bogus",
        "/quran/sources/bogus/surah/2",
    ] {
        let (status, body, _) = send(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        let message = error_message(&body);
        assert!(
            message.contains("uthmani, simple-clean"),
            "{uri} must list the production-available scripts; got: {message}"
        );
        assert!(
            !message.contains("indopak") && !message.contains("tajweed"),
            "{uri} must not advertise gated scripts; got: {message}"
        );
    }

    // The Uthmani family stays served exactly as before.
    let (status, body, _) = send(&app, "/quran/sources/uthmani/surah/2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["sourceId"], "uthmani");
    let (status, body, _) = send(&app, "/quran/sources/simple-clean/range?from=1&to=7").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["ayahs"].as_array().unwrap().len(), 7);
    let (status, body, _) = send(&app, "/quran/surahs/2/ayahs?script=simple-clean").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["range"]["script"], "simple-clean");
    let (status, _, _) = send(&app, "/quran/ayahs/1/1?script=uthmani").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn prod_scripts_list_excludes_gated_scripts_even_when_artifacts_verify() {
    use ruxlog::quran::{QuranStore, Script};
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    // Green HEADs for ALL four artifacts: the availability gate, not a HEAD
    // failure, must be what removes indopak + tajweed from the advertised list.
    for script in Script::ALL {
        let filename = QuranStore::artifact_filename(script);
        let cl = state.quran.artifact(script).size_bytes.to_string();
        Mock::given(method("HEAD"))
            .and(path_regex(format!(r".*/tanzil/arabic/{filename}")))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(&server)
            .await;
    }

    let app = app_over(state);
    let (status, body, headers) = send(&app, "/quran/scripts").await;
    assert_eq!(status, StatusCode::OK);
    let scripts = body["data"]["scripts"].as_array().unwrap();
    let ids: Vec<&str> = scripts.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        ["uthmani", "simple-clean"],
        "production advertises only the Uthmani family"
    );
    for row in scripts {
        let url = row["downloadUrl"].as_str().unwrap();
        assert!(
            !url.contains("indopak") && !url.contains("tajweed"),
            "gated artifact must not be advertised: {row}"
        );
    }
    assert_eq!(
        headers.get(header::CACHE_CONTROL).unwrap(),
        ruxlog::modules::quran_v1::cache::ARABIC_CACHE,
        "all advertised (production-subset) artifacts verified -> cacheable, not no-store"
    );
}

#[tokio::test]
async fn prod_sources_list_excludes_gated_scripts_even_when_artifacts_verify() {
    use ruxlog::quran::{QuranStore, Script};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let catalogue_len = state.translation_pool.catalogue().len();
    // Green HEADs for every script (gated ones included) + every translation.
    for script in Script::ALL {
        let file = state.quran.artifact(script);
        let filename = QuranStore::artifact_filename(script);
        let cl = file.size_bytes.to_string();
        Mock::given(method("HEAD"))
            .and(path(format!("/tanzil/arabic/{filename}")))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(&server)
            .await;
    }
    for e in state.translation_pool.catalogue().values() {
        let cl = e.size_bytes.to_string();
        Mock::given(method("HEAD"))
            .and(path(format!("/tanzil/translations/{}", e.path)))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(&server)
            .await;
    }

    let app = app_over(state);
    let (status, body, headers) = send(&app, "/quran/sources").await;
    assert_eq!(status, StatusCode::OK);
    let sources = body["data"]["sources"].as_array().unwrap();
    assert_eq!(
        sources.len(),
        2 + catalogue_len,
        "production sources = Uthmani family + every translation"
    );
    let ids: Vec<&str> = sources.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"uthmani") && ids.contains(&"simple-clean"));
    assert!(!ids.contains(&"indopak") && !ids.contains(&"tajweed"));
    for row in sources {
        let url = row["downloadUrl"].as_str().unwrap();
        assert!(
            !url.contains("quran-indopak") && !url.contains("quran-tajweed"),
            "gated artifact must not be advertised: {row}"
        );
    }
    assert_eq!(
        headers.get(header::CACHE_CONTROL).unwrap(),
        ruxlog::modules::quran_v1::cache::ARABIC_CACHE,
        "all advertised (production-subset) sources verified -> cacheable, not no-store"
    );
}
