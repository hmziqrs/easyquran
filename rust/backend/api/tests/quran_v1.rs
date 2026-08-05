use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::middleware;
use serde_json::Value;
use tower::ServiceExt;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, Settings, SiteSettings,
};
use ruxlog::middlewares::{client_ip, rate_limit};
use ruxlog::modules::quran_v1;
use ruxlog::quran::load_quran_store;
use ruxlog::router::with_observability;
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, StorageState};

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
    state_with_public_url("http://localhost.invalid").await
}

async fn state_with_public_url(public_url: &str) -> AppState {
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
        quran_scripts: Arc::new(tokio::sync::Mutex::new(None)),
        translation_pool,
        quran_sources: Arc::new(tokio::sync::Mutex::new(None)),
    }
}

async fn app() -> axum::Router {
    app_over(state().await)
}

fn app_over(state: AppState) -> axum::Router {
    let ip_source = state.settings.http.ip_source.clone();
    let quran = quran_v1::routes()
        .merge(quran_v1::search_route().layer(rate_limit::rate_limit_layer(&state, 1_000_000, 60)));
    let public = with_observability(quran)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(rate_limit::rate_limit_layer_branch(
            &state, 1_000_000, 60, "quran-v1",
        ))
        .layer(middleware::from_fn(client_ip::resolve_client_ip))
        .layer(ip_source.into_extension())
        .layer(axum::Extension(state.clone()))
        .layer(quran_v1::cors::public_cors_layer());
    axum::Router::new().nest("/quran", public).with_state(state)
}

async fn full_app() -> axum::Router {
    let state = state().await;
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    let private_cors = tower_http::cors::CorsLayer::new()
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_origin(tower_http::cors::AllowOrigin::list(
            ruxlog::utils::cors::get_allowed_origins(),
        ))
        .allow_credentials(true);
    let ip_source = state.settings.http.ip_source.clone();
    let private = ruxlog::router::router(state.clone())
        .layer(middleware::from_fn(client_ip::resolve_client_ip))
        .layer(ip_source.clone().into_extension())
        .layer(axum::Extension(state.clone()))
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(middleware::from_fn(ruxlog::middlewares::cors::origin_guard))
        .layer(middleware::from_fn(
            ruxlog::middlewares::static_csrf::csrf_guard,
        ))
        .layer(session_layer)
        .layer(private_cors);
    let public = app_over_public(&state, ip_source);
    axum::Router::new()
        .merge(private)
        .nest("/quran", public)
        .with_state(state)
}

fn app_over_public(
    state: &AppState,
    ip_source: axum_client_ip::ClientIpSource,
) -> axum::Router<AppState> {
    let quran = quran_v1::routes()
        .merge(quran_v1::search_route().layer(rate_limit::rate_limit_layer(state, 1_000_000, 60)))
        .layer(middleware::from_fn(quran_v1::error::shape_routing_errors));
    with_observability(quran)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(rate_limit::rate_limit_layer_branch(
            state, 1_000_000, 60, "quran-v1",
        ))
        .layer(middleware::from_fn(client_ip::resolve_client_ip))
        .layer(ip_source.into_extension())
        .layer(axum::Extension(state.clone()))
        .layer(quran_v1::cors::public_cors_layer())
}

async fn req(method: Method, uri: &str) -> (StatusCode, Value, reqwest::header::HeaderMap) {
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 16 * 1024 * 1024).await.unwrap();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, body, headers)
}

async fn get(uri: &str) -> (StatusCode, Value, reqwest::header::HeaderMap) {
    req(Method::GET, uri).await
}

fn data(v: &Value) -> &Value {
    &v["data"]
}

#[tokio::test]
async fn surahs_list_shape_and_count() {
    let (st, body, headers) = get("/quran/surahs").await;
    assert_eq!(st, StatusCode::OK);
    let arr = data(&body).as_array().unwrap();
    assert_eq!(arr.len(), 114);
    assert!(arr[0].get("ayahCount").is_some());
    assert!(arr[0].get("ayas").is_none());
    assert_eq!(arr[0]["place"], "meccan");
    assert!(headers.contains_key(header::ETAG));
    assert_eq!(headers.get(header::VARY).unwrap(), "Accept-Encoding");
}

#[tokio::test]
async fn get_one_surah_and_404() {
    let (st, body, _) = get("/quran/surahs/1").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(data(&body)["ayahCount"], 7);
    assert_eq!(data(&body)["bismillah"], "first-ayah");
    assert_eq!(get("/quran/surahs/0").await.0, StatusCode::NOT_FOUND);
    assert_eq!(get("/quran/surahs/115").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn surah_ayahs_inclusive_and_ordered() {
    let (st, body, _) = get("/quran/surahs/1/ayahs").await;
    assert_eq!(st, StatusCode::OK);
    let ayahs = data(&body)["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 7);
    assert_eq!(ayahs[0]["key"], "1:1");
    assert_eq!(ayahs[0]["globalIndex"], 1);
    assert_eq!(ayahs[0]["juz"], 1);
    let (_, body, _) = get("/quran/surahs/2/ayahs?from=1&to=3").await;
    assert_eq!(data(&body)["ayahs"].as_array().unwrap().len(), 3);
    assert_eq!(
        get("/quran/surahs/2/ayahs?from=3&to=1").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn single_ayah_redirect_alias_and_400() {
    let (st, body, _) = get("/quran/ayahs/1/1").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(data(&body)["key"], "1:1");
    assert!(!data(&body)["text"].as_str().unwrap().is_empty());

    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/quran/ayahs/2:255")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        "/quran/ayahs/2/255"
    );

    assert_eq!(get("/quran/ayahs/abc").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ayahs_keys_and_global_range() {
    let (st, body, _) = get("/quran/ayahs?keys=2:255,1:1").await;
    assert_eq!(st, StatusCode::OK);
    let ayahs = data(&body)["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 2);
    assert_eq!(ayahs[0]["key"], "2:255");
    assert_eq!(ayahs[1]["key"], "1:1");

    let (_, body, _) = get("/quran/ayahs?fromGlobal=1&toGlobal=7").await;
    assert_eq!(data(&body)["ayahs"].as_array().unwrap().len(), 7);

    assert_eq!(
        get("/quran/ayahs?fromGlobal=1&toGlobal=6236").await.0,
        StatusCode::BAD_REQUEST
    );

    let (_, body, _) = get("/quran/ayahs?fromGlobal=1&toGlobal=6236&limit=300").await;
    assert_eq!(data(&body)["ayahs"].as_array().unwrap().len(), 300);
    assert_eq!(data(&body)["range"]["nextCursor"], 301);
}

#[tokio::test]
async fn script_param_default_and_validation() {
    let (_, _, h_uth) = get("/quran/ayahs/1/1").await;
    let (_, _, h_sc) = get("/quran/ayahs/1/1?script=simple-clean").await;
    assert_ne!(
        h_uth.get(header::ETAG).unwrap(),
        h_sc.get(header::ETAG).unwrap()
    );
    assert_eq!(
        get("/quran/ayahs/1/1?script=bogus").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn unknown_query_param_rejected() {
    assert_eq!(
        get("/quran/surahs/1/ayahs?bogus=1").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get("/quran/surahs/1?foo=1").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn method_not_allowed() {
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/quran/surahs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(resp.headers().contains_key(header::ALLOW));
}

#[tokio::test]
async fn conditional_get_returns_304() {
    let (_, _, headers) = get("/quran/surahs/1/ayahs").await;
    let etag = headers
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let cc = headers
        .get(header::CACHE_CONTROL)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/quran/surahs/1/ayahs")
                .header(header::IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(resp.headers().get(header::ETAG).unwrap(), etag.as_str());
    assert_eq!(
        resp.headers().get(header::CACHE_CONTROL).unwrap(),
        cc.as_str()
    );
    assert_eq!(resp.headers().get(header::VARY).unwrap(), "Accept-Encoding");
}

#[tokio::test]
async fn cors_preflight_wildcard_no_credentials() {
    let app = app().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/quran/surahs")
                .header(header::ORIGIN, "https://example.com")
                .header("access-control-request-method", "GET")
                .header(header::IF_NONE_MATCH, "x")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(resp.status().is_success());
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "*"
    );
    assert!(resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        .is_none());
    let allow_h = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
        .unwrap()
        .to_str()
        .unwrap()
        .to_lowercase();
    assert!(allow_h.contains("if-none-match"));

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/surahs")
                .header(header::ORIGIN, "https://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "*"
    );
    let expose = resp
        .headers()
        .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
        .expect("Expose-Headers on actual response")
        .to_str()
        .unwrap()
        .to_lowercase();
    assert!(expose.contains("etag"));
    assert!(expose.contains("cache-control"));
}

#[tokio::test]
async fn health_ready_endpoint() {
    let (st, body, headers) = get("/quran/health/ready").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["ready"], true);
    assert_eq!(body["verseCount"], 6236);
    assert!(
        body.get("sourceDigests").is_none(),
        "health must not expose source digests — manual audit only (docs/quran.md §2)"
    );
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
}

#[tokio::test]
async fn navigation_families_counts_and_hizb_derivation() {
    for (path, n) in [
        ("/quran/juzs", 30),
        ("/quran/pages", 604),
        ("/quran/rukus", 556),
        ("/quran/hizb-quarters", 240),
        ("/quran/manzils", 7),
        ("/quran/sajdas", 15),
    ] {
        let (st, body, _) = get(path).await;
        assert_eq!(st, StatusCode::OK, "{path}");
        assert_eq!(data(&body).as_array().unwrap().len(), n, "{path}");
    }
    let (_, b1, _) = get("/quran/hizb-quarters/1").await;
    assert_eq!(data(&b1)["hizb"], 1);
    assert_eq!(data(&b1)["quarterInHizb"], 1);
    let (_, b5, _) = get("/quran/hizb-quarters/5").await;
    assert_eq!(data(&b5)["hizb"], 2);
    assert_eq!(data(&b5)["quarterInHizb"], 1);
}

#[tokio::test]
async fn random_date_is_immutable_cache() {
    let (st, _, headers) = get("/quran/random?date=2026-07-30").await;
    assert_eq!(st, StatusCode::OK);
    assert!(headers
        .get(header::CACHE_CONTROL)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("immutable"));
    assert_eq!(
        get("/quran/random?date=2026-2-3").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get("/quran/random?date=2026-02-30").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn search_results_ordered_with_highlights_and_limits() {
    let q = "%D8%A7%D9%84%D8%AD%D9%85%D8%AF";
    let (st, body, headers) = get(&format!("/quran/search?q={q}")).await;
    assert_eq!(st, StatusCode::OK);
    let d = data(&body);
    assert!(d["total"].as_u64().unwrap() > 0, "expected matches");
    let results = d["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let mut prev: u64 = 0;
    for r in results {
        // Web decodeSearchHit rejects any hit whose kind is not "ayah" | "opener" and nulls the whole payload.
        assert_eq!(
            r["kind"], "ayah",
            "every ayah hit carries the kind discriminator"
        );
        let g = r["ayah"]["globalIndex"].as_u64().unwrap();
        assert!(g >= prev, "results not ascending by globalIndex");
        prev = g;
        assert!(
            !r["highlights"].as_array().unwrap().is_empty(),
            "each hit has at least one highlight"
        );
    }
    assert_eq!(d["limit"], 20);
    assert_eq!(d["query"], "الحمد");
    assert!(headers.contains_key(header::ETAG));

    let two = "%D9%85%D9%86";
    assert_eq!(
        get(&format!("/quran/search?q={two}")).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn error_envelope_matches_section_6_4() {
    let (st, body, _) = get("/quran/surahs/115").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(!body["error"]["message"].as_str().unwrap().is_empty());

    let (st, body, _) = get("/quran/ayahs/1/1?script=bogus").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"]["code"].is_string());
    assert!(!body["error"]["message"].as_str().unwrap().is_empty());

    let (st, body, _) = get("/quran/ayahs?fromGlobal=1&toGlobal=6236").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "range_too_large");
    assert_eq!(body["error"]["detail"]["max"], 300);
    assert_eq!(body["error"]["detail"]["requested"], 6236);

    let (st, body, _) = get("/quran/surahs?bogus=1").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"]["code"].is_string());

    let (st, body, _) = get("/quran/surahs/abc").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"]["code"].is_string());
}

#[tokio::test]
async fn cursor_pagination_page_two() {
    let (_, body, _) = get("/quran/ayahs?fromGlobal=1&toGlobal=6236&limit=300").await;
    let next = data(&body)["range"]["nextCursor"].as_u64().unwrap();
    assert_eq!(next, 301);
    let (st, body2, _) = get(&format!(
        "/quran/ayahs?fromGlobal=1&toGlobal=6236&limit=300&cursor={next}"
    ))
    .await;
    assert_eq!(st, StatusCode::OK);
    let ayahs = data(&body2)["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 300);
    assert_eq!(ayahs[0]["globalIndex"], 301, "page 2 starts at global 301");
    assert_eq!(data(&body2)["range"]["nextCursor"], 601);
    assert_eq!(
        get("/quran/ayahs?fromGlobal=1&toGlobal=7&cursor=999")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn conditional_get_on_search_and_head_method() {
    let q = "%D8%A7%D9%84%D8%AD%D9%85%D8%AF";
    let (_, _, headers) = get(&format!("/quran/search?q={q}")).await;
    let etag = headers
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .uri(format!("/quran/search?q={q}"))
                .header(header::IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(resp.headers().get(header::ETAG).unwrap(), etag.as_str());
    assert_eq!(resp.headers().get(header::VARY).unwrap(), "Accept-Encoding");

    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/quran/scripts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key(header::ETAG));
    let bytes = to_bytes(resp.into_body(), 1024).await.unwrap();
    assert!(bytes.is_empty(), "HEAD body must be empty");
}

#[tokio::test]
async fn cors_no_cookies_on_every_public_route() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let cl_u = state.quran.artifacts.uthmani.size_bytes.to_string();
    let cl_sc = state.quran.artifacts.simple_clean.size_bytes.to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-uthmani\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-simple-clean\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;
    let app = app_over(state);

    #[rustfmt::skip]
    let routes = [
        "/quran/surahs", "/quran/surahs/1", "/quran/surahs/1/ayahs?limit=300",
        "/quran/ayahs?keys=1:1", "/quran/ayahs/1:1", "/quran/ayahs/1/1",
        "/quran/juzs", "/quran/juzs/1", "/quran/juzs/1/ayahs?limit=300",
        "/quran/pages", "/quran/pages/2", "/quran/pages/2/ayahs?limit=300",
        "/quran/rukus", "/quran/rukus/1", "/quran/rukus/1/ayahs?limit=300",
        "/quran/hizb-quarters", "/quran/hizb-quarters/1", "/quran/hizb-quarters/1/ayahs?limit=300",
        "/quran/manzils", "/quran/manzils/1", "/quran/manzils/1/ayahs?limit=300",
        "/quran/sajdas", "/quran/sajdas/1",
        "/quran/scripts", "/quran/random", "/quran/health/ready",
        "/quran/sources/uthmani/surah/2", "/quran/sources/uthmani/range?from=1&to=7",
        "/quran/sources",
        "/quran/openapi.json",
        "/quran/search?q=%D8%A7%D9%84%D8%AD%D9%85%D8%AF",
    ];
    let route_count = include_str!("../src/modules/quran_v1/mod.rs")
        .matches(".route(")
        .count();
    assert_eq!(
        routes.len(),
        route_count,
        "CORS table must cover every `.route(` in mod.rs (Phase 1a exit #1)"
    );

    for path in routes {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::ORIGIN, "https://x.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        assert!(
            status.is_success() || status == StatusCode::PERMANENT_REDIRECT,
            "{path} -> {status}"
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*",
            "{path} ACAO"
        );
        assert!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .is_none(),
            "{path} must not allow credentials"
        );
        assert!(
            resp.headers().get(header::SET_COOKIE).is_none(),
            "{path} must not Set-Cookie (no private-layer leak)"
        );
    }
}

#[test]
fn modules_quran_v1_has_no_sqlite_refs() {
    let srcs: [&str; 6] = [
        include_str!("../src/modules/quran_v1/controller.rs"),
        include_str!("../src/modules/quran_v1/dto.rs"),
        include_str!("../src/modules/quran_v1/cache.rs"),
        include_str!("../src/modules/quran_v1/cors.rs"),
        include_str!("../src/modules/quran_v1/mod.rs"),
        include_str!("../src/modules/quran_v1/error.rs"),
    ];
    let banned = [
        "use sqlx",
        "sea_db",
        "rusqlite",
        "SqliteConnection",
        "DatabaseConnection",
        "SqliteConnectOptions",
    ];
    for (i, s) in srcs.iter().enumerate() {
        for b in banned {
            assert!(
                !s.contains(b),
                "modules/quran_v1 file[{i}] references {b:?} (§10)"
            );
        }
    }
}

#[tokio::test]
async fn phase_1a_auth_regression_on_merged_router() {
    std::env::set_var(
        "COOKIE_KEY",
        "test_cookie_key_padded_to_more_than_32_bytes_for_tests",
    );
    let app = full_app().await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/csrf/v1/generate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "csrf generate succeeds");
    assert!(
        resp.headers().get(header::SET_COOKIE).is_some(),
        "a session cookie must still be issued on the private branch"
    );

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header(header::ORIGIN, "https://evil.example.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        !resp.status().is_success(),
        "off-origin must be rejected by origin_guard (got {})",
        resp.status()
    );

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/surahs")
                .header(header::ORIGIN, "https://evil.example.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "*"
    );
    assert!(
        resp.headers().get(header::SET_COOKIE).is_none(),
        "public branch must not set a session cookie"
    );
}

#[tokio::test]
async fn scripts_omits_failed_head_artifacts() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let uthmani_size = state.quran.artifacts.uthmani.size_bytes;
    let sc_size = state.quran.artifacts.simple_clean.size_bytes;

    let cl_u = uthmani_size.to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-uthmani\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    let cl_sc = (sc_size + 1).to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-simple-clean\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;

    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/scripts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let scripts = body["data"]["scripts"].as_array().unwrap();
    assert_eq!(
        scripts.len(),
        1,
        "only the verified (uthmani) artifact remains"
    );
    assert_eq!(scripts[0]["id"], "uthmani");
    assert!(scripts[0]["downloadUrl"]
        .as_str()
        .unwrap()
        .ends_with(".sqlite"));
}

#[tokio::test]
async fn scripts_happy_path_advertises_both_artifacts() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let cl_u = state.quran.artifacts.uthmani.size_bytes.to_string();
    let cl_sc = state.quran.artifacts.simple_clean.size_bytes.to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-uthmani\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-simple-clean\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;

    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/scripts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let scripts = body["data"]["scripts"].as_array().unwrap();
    assert_eq!(
        scripts.len(),
        2,
        "happy path advertises exactly both artifacts"
    );
    let ids: Vec<&str> = scripts.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"uthmani"), "uthmani advertised");
    assert!(ids.contains(&"simple-clean"), "simple-clean advertised");
    for s in scripts {
        let id = s["id"].as_str().unwrap();
        let filename = match id {
            "uthmani" => "quran-uthmani.sqlite",
            "simple-clean" => "quran-simple-clean.sqlite",
            other => panic!("unexpected script id: {other}"),
        };
        assert_eq!(
            s["downloadUrl"].as_str().unwrap(),
            format!("{}/tanzil/arabic/{filename}", server.uri()),
            "downloadUrl must match publisher R2 key tanzil/arabic/<file>.sqlite (upload-sqlite.ts PREFIX)"
        );
    }
    assert_eq!(
        cc.as_deref(),
        Some(ruxlog::modules::quran_v1::cache::ARABIC_CACHE)
    );
}

#[tokio::test]
async fn scripts_endpoint_carries_no_sha256() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let cl_u = state.quran.artifacts.uthmani.size_bytes.to_string();
    let cl_sc = state.quran.artifacts.simple_clean.size_bytes.to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-uthmani\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/tanzil/arabic/quran-simple-clean\.sqlite"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;
    let app = app_over(state);
    let resp = app
        .oneshot(Request::builder().uri("/quran/scripts").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let scripts = body["data"]["scripts"].as_array().unwrap();
    assert!(!scripts.is_empty());
    for row in scripts {
        assert!(
            row.get("sha256").is_none(),
            "scripts artifact must not carry sha256 — a source's identity is its id, never a hash \
             (docs/quran.md §2); got {row}"
        );
    }
}

#[tokio::test]
async fn range_out_of_bounds_is_400_not_clamped() {
    assert_eq!(
        get("/quran/surahs/2/ayahs?from=1&to=999").await.0,
        StatusCode::BAD_REQUEST,
        "to beyond the unit length must be rejected"
    );
    assert_eq!(
        get("/quran/surahs/2/ayahs?from=300&to=310").await.0,
        StatusCode::BAD_REQUEST,
        "from beyond the unit length must be rejected"
    );
    assert_eq!(
        get("/quran/juzs/1/ayahs?from=1&to=9999").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get("/quran/surahs/2/ayahs?from=1&to=3").await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn api_response_text_equals_store_source() {
    use ruxlog::quran::Script;

    let state = state().await;
    let app = app_over(state.clone());
    for (s, a, script) in [
        (1u16, 1u16, Script::Uthmani),
        (2u16, 1u16, Script::Uthmani),
        (112u16, 1u16, Script::SimpleClean),
        (55u16, 3u16, Script::SimpleClean),
    ] {
        let g = state.quran.meta().global_of(s, a).expect("valid ayah");
        let uri = format!("/quran/ayahs/{s}/{a}?script={}", script.as_str());
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{uri}");
        let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let served = body["data"]["text"].as_str().expect("text field present");
        let src = state.quran.verse(script, g).expect("store verse");
        assert_eq!(
            served.as_bytes(),
            src.as_bytes(),
            "served `text` must equal the store's verbatim source for {uri}"
        );
    }
}

#[tokio::test]
async fn search_respects_limit_offset_and_is_stable() {
    let q = "%D8%A7%D9%84%D8%AD%D9%85%D8%AF";
    let (_, b1, _) = get(&format!("/quran/search?q={q}&limit=5")).await;
    let d1 = data(&b1);
    assert_eq!(d1["limit"], 5);
    let results1 = d1["results"].as_array().unwrap();
    assert!(
        results1.len() <= 5,
        "result window must respect `limit` (got {})",
        results1.len()
    );
    let keys1: Vec<String> = results1
        .iter()
        .map(|r| r["ayah"]["key"].as_str().unwrap().to_string())
        .collect();

    let (_, b2, _) = get(&format!("/quran/search?q={q}&limit=5")).await;
    let keys2: Vec<String> = data(&b2)["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["ayah"]["key"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(keys1, keys2, "fixed query must be deterministic");

    if keys1.len() > 1 {
        let (_, bo, _) = get(&format!("/quran/search?q={q}&limit=5&offset=1")).await;
        let first_at_offset = data(&bo)["results"].as_array().unwrap()[0]["ayah"]["key"]
            .as_str()
            .unwrap();
        assert_eq!(
            first_at_offset, keys1[1],
            "offset=1 begins at the 2nd base hit"
        );
    }
}

#[tokio::test]
async fn web_compatible_read_endpoints() {
    let (st, body, _) = get("/quran/sources/uthmani/surah/2").await;
    assert_eq!(st, StatusCode::OK);
    let d = data(&body);
    assert_eq!(d["sourceId"], "uthmani");
    assert_eq!(d["script"], "uthmani");
    assert!(d["verses"].as_array().unwrap().len() > 1);
    let n = &d["normalization"];
    assert_eq!(n["surah"], 2);
    assert_eq!(n["sourceId"], "uthmani");
    assert_eq!(n["sourceProfile"], "tanzil-uthmani-581cc540");
    assert_eq!(n["packaging"], "embedded-prefix");
    assert_eq!(n["openerKind"], "header");
    assert_eq!(n["openerEndScalar"], 39);
    assert_eq!(n["bodyStartScalar"], 40);
    assert!(n["openerText"].is_string());
    let (_, bsc, _) = get("/quran/sources/simple-clean/surah/2").await;
    assert_eq!(data(&bsc)["normalization"]["bodyStartScalar"], 23);

    let (st, body, _) = get("/quran/sources/uthmani/range?from=1&to=7").await;
    assert_eq!(st, StatusCode::OK);
    let d = data(&body);
    let ayahs = d["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 7);
    assert_eq!(ayahs[0]["key"], "1:1");
    assert_eq!(ayahs[0]["globalIndex"], 1);
    assert!(ayahs[0]["text"].is_string());
    let lean = ["key", "surah", "ayah", "globalIndex", "text"];
    let obj = ayahs[0].as_object().unwrap();
    assert!(
        obj.keys().all(|k| lean.contains(&k.as_str())),
        "lean ayah must not carry extra fields"
    );
    assert_eq!(d["normalizations"].as_array().unwrap().len(), 1);
    assert_eq!(d["normalizations"][0]["surah"], 1);

    let (_, b2, _) = get("/quran/sources/uthmani/range?from=7&to=10").await;
    let norms = data(&b2)["normalizations"].as_array().unwrap();
    assert!(norms.iter().any(|n| n["surah"].as_u64() == Some(1)));
    assert!(norms.iter().any(|n| n["surah"].as_u64() == Some(2)));

    assert_eq!(
        get("/quran/sources/bogus/surah/2").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get("/quran/sources/uthmani/surah/115").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get("/quran/sources/uthmani/range?from=1&to=6236").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn translation_surah_serves_text_with_absent_packaging() {
    let (st, body, _) = get("/quran/sources/en.sahih/surah/2").await;
    assert_eq!(st, StatusCode::OK);
    let d = data(&body);
    let verses = d["verses"].as_array().expect("verses");
    assert!(!verses.is_empty(), "translation carries verses");
    // Body-only: a translation verse 1 is content, not a basmala opener.
    assert_eq!(d["normalization"]["packaging"], "absent");
    assert_eq!(d["normalization"]["openerKind"], "none");
    assert_eq!(d["normalization"]["openerEndScalar"], 0);
    assert_eq!(d["normalization"]["bodyStartScalar"], 0);
    assert_eq!(d["sourceId"], "en.sahih");
}

#[tokio::test]
async fn translation_range_serves_lean_ayahs_and_absent_normalization() {
    let (st, body, _) = get("/quran/sources/en.sahih/range?from=1&to=7").await;
    assert_eq!(st, StatusCode::OK);
    let d = data(&body);
    assert_eq!(d["ayahs"].as_array().unwrap().len(), 7);
    let norms = d["normalizations"].as_array().unwrap();
    assert_eq!(norms.len(), 1);
    assert_eq!(norms[0]["packaging"], "absent");
}

#[tokio::test]
async fn translation_source_rejects_unknown_and_traversal() {
    // Unknown id -> 400.
    assert_eq!(
        get("/quran/sources/nope/surah/2").await.0,
        StatusCode::BAD_REQUEST
    );
    // Path-traversal payloads must never resolve to a real translation or a 200
    // (the catalogue-whitelist membership check is the traversal guard).
    let trav = get("/quran/sources/..%2Fquran-uthmani/surah/1").await.0;
    assert!(
        trav.is_client_error() || trav == StatusCode::NOT_FOUND,
        "traversal not allowed: {trav}"
    );
    let trav2 = get("/quran/sources/en.sahih%2F..%2Ffoo/surah/1").await.0;
    assert!(
        trav2.is_client_error() || trav2 == StatusCode::NOT_FOUND,
        "traversal not allowed: {trav2}"
    );
}

#[tokio::test]
async fn arabic_and_translation_sources_carry_distinct_etags() {
    let (st_a, _, hdr_a) = get("/quran/sources/uthmani/surah/1").await;
    assert_eq!(st_a, StatusCode::OK);
    let (st_t, _, hdr_t) = get("/quran/sources/en.sahih/surah/1").await;
    assert_eq!(st_t, StatusCode::OK);
    let etag_a = hdr_a.get("etag").unwrap().to_str().unwrap();
    let etag_t = hdr_t.get("etag").unwrap().to_str().unwrap();
    assert_ne!(
        etag_a, etag_t,
        "translation must not share the uthmani ETag"
    );
    assert!(
        etag_t.contains("tanzil-en.sahih"),
        "translation etag keyed by its id (no digest — identity is the id): {etag_t}"
    );
}

async fn mount_green_sources_heads(state: &AppState, server: &wiremock::MockServer) -> usize {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};
    let mut n = 0usize;
    for (file, filename) in [
        (&state.quran.artifacts.uthmani, "quran-uthmani.sqlite"),
        (
            &state.quran.artifacts.simple_clean,
            "quran-simple-clean.sqlite",
        ),
    ] {
        let cl = file.size_bytes.to_string();
        Mock::given(method("HEAD"))
            .and(path(format!("/tanzil/arabic/{filename}")))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(server)
            .await;
        n += 1;
    }
    for (_id, e) in state.translation_pool.catalogue() {
        let cl = e.size_bytes.to_string();
        Mock::given(method("HEAD"))
            .and(path(format!("/tanzil/translations/{}", e.path)))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(server)
            .await;
        n += 1;
    }
    n
}

#[tokio::test]
async fn sources_lists_all_sources_with_verified_download_urls() {
    use wiremock::MockServer;
    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let total = mount_green_sources_heads(&state, &server).await;

    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/sources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let sources = body["data"]["sources"].as_array().unwrap();
    assert_eq!(sources.len(), total, "every source HEAD-verified");

    let by_id: std::collections::HashMap<&str, &serde_json::Value> = sources
        .iter()
        .map(|s| (s["id"].as_str().unwrap(), s))
        .collect();
    // Arabic entry shape + URL (guards the §1 tanzil/arabic layout).
    let u = by_id["uthmani"];
    assert_eq!(u["kind"], "arabic");
    assert_eq!(u["languageCode"], "ar");
    assert_eq!(u["direction"], "rtl");
    assert!(u["translator"].is_null(), "Arabic has no translator");
    assert!(u["downloadUrl"]
        .as_str()
        .unwrap()
        .ends_with("/tanzil/arabic/quran-uthmani.sqlite"));
    // Translation entry shape + URL (R2 layout tanzil/translations/sqlite/<id>.sqlite).
    let t = by_id["en.sahih"];
    assert_eq!(t["kind"], "translation");
    assert_eq!(t["languageCode"], "en");
    assert!(t["translator"].as_str().is_some());
    assert!(t["downloadUrl"]
        .as_str()
        .unwrap()
        .ends_with("/tanzil/translations/sqlite/en.sahih.sqlite"));
    assert_eq!(
        cc.as_deref(),
        Some(ruxlog::modules::quran_v1::cache::ARABIC_CACHE)
    );
}

#[tokio::test]
async fn sources_rows_never_carry_sha256() {
    use wiremock::MockServer;
    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let _ = mount_green_sources_heads(&state, &server).await;
    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/sources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let sources = body["data"]["sources"].as_array().unwrap();
    assert!(!sources.is_empty());
    for row in sources {
        assert!(
            row.get("sha256").is_none(),
            "sources row must not carry sha256 — a source's identity is its id, never a hash \
             (docs/quran.md §2); got {row}"
        );
    }
}

#[tokio::test]
async fn sources_partial_upstream_is_no_store_not_cached_truncation() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    // Green for Arabic + all translations EXCEPT one (wrong content-length).
    for (file, filename) in [
        (&state.quran.artifacts.uthmani, "quran-uthmani.sqlite"),
        (
            &state.quran.artifacts.simple_clean,
            "quran-simple-clean.sqlite",
        ),
    ] {
        let cl = file.size_bytes.to_string();
        Mock::given(method("HEAD"))
            .and(path(format!("/tanzil/arabic/{filename}")))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(&server)
            .await;
    }
    let mut first = true;
    for (_id, e) in state.translation_pool.catalogue() {
        let size = if first {
            first = false;
            e.size_bytes + 1
        } else {
            e.size_bytes
        };
        let cl = size.to_string();
        Mock::given(method("HEAD"))
            .and(path(format!("/tanzil/translations/{}", e.path)))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl.as_str()))
            .mount(&server)
            .await;
    }

    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/sources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let sources = body["data"]["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 116, "exactly one translation omitted");
    assert_eq!(
        cc.as_deref(),
        Some(ruxlog::modules::quran_v1::cache::NO_STORE),
        "partial upstream must be no-store, not a cached truncation"
    );
}
