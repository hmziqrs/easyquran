//! Integration tests for the public `/quran/v1` API (§10 — HTTP section).
//!
//! Builds the real public branch (routes + observability + rate limit + client
//! IP + public CORS) over a stub `AppState` carrying a real `QuranStore`, then
//! drives it with one-shot requests. Helpers are defined inline because
//! `#[cfg(test)]`-gated lib modules are not visible to integration-test crates.

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
        expected_content_version: None,
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
        GeoRouter::new(GeoRulesConfig { default_provider: String::new(), rules: vec![] }),
    ));
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
    }
}

async fn app() -> axum::Router {
    app_over(state().await)
}

/// Build the public `/quran/v1` branch over a caller-supplied state (so a test
/// can inject e.g. a mock `public_url` for `/scripts`). Mirrors production
/// layering (§8.2): search split out with its own limit; coarse IP-only branch
/// limit; CompressionLayer present (compression-vs-ETag/conditional-GET exercised).
fn app_over(state: AppState) -> axum::Router {
    let ip_source = state.settings.http.ip_source.clone();
    let quran = quran_v1::routes().merge(
        quran_v1::search_route().layer(rate_limit::rate_limit_layer(&state, 1_000_000, 60)),
    );
    let public = with_observability(quran)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(rate_limit::rate_limit_layer_branch(&state, 1_000_000, 60, "quran-v1"))
        .layer(middleware::from_fn(client_ip::resolve_client_ip))
        .layer(ip_source.into_extension())
        .layer(axum::Extension(state.clone()))
        .layer(quran_v1::cors::public_cors_layer());
    axum::Router::new().nest("/quran/v1", public).with_state(state)
}

/// The FULL merged app (private authed branch + public /quran/v1 sibling),
/// mirroring `main` minus `RouteBlockerLayer` (which needs the app DB and is
/// not the split-regression risk). Used for Phase 1a exit #2.
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
        .layer(middleware::from_fn(ruxlog::middlewares::static_csrf::csrf_guard))
        .layer(session_layer)
        .layer(private_cors);
    let public = app_over_public(&state, ip_source);
    axum::Router::new()
        .merge(private)
        .nest("/quran/v1", public)
        .with_state(state)
}

/// Public branch builder used by [`full_app`] (shares layering with [`app_over`]).
fn app_over_public(state: &AppState, ip_source: axum_client_ip::ClientIpSource) -> axum::Router<AppState> {
    let quran = quran_v1::routes().merge(
        quran_v1::search_route().layer(rate_limit::rate_limit_layer(state, 1_000_000, 60)),
    )
    .layer(middleware::from_fn(quran_v1::error::shape_routing_errors));
    with_observability(quran)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(rate_limit::rate_limit_layer_branch(state, 1_000_000, 60, "quran-v1"))
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
    let (st, body, headers) = get("/quran/v1/surahs").await;
    assert_eq!(st, StatusCode::OK);
    let arr = data(&body).as_array().unwrap();
    assert_eq!(arr.len(), 114);
    assert!(arr[0].get("ayahCount").is_some()); // camelCase, not ayas
    assert!(arr[0].get("ayas").is_none());
    assert_eq!(arr[0]["place"], "meccan");
    assert!(body.get("contentVersion").is_some());
    assert!(headers.contains_key(header::ETAG));
    assert_eq!(headers.get(header::VARY).unwrap(), "Accept-Encoding");
}

#[tokio::test]
async fn get_one_surah_and_404() {
    let (st, body, _) = get("/quran/v1/surahs/1").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(data(&body)["ayahCount"], 7);
    assert_eq!(data(&body)["bismillah"], "first-ayah");
    assert_eq!(get("/quran/v1/surahs/0").await.0, StatusCode::NOT_FOUND);
    assert_eq!(get("/quran/v1/surahs/115").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn surah_ayahs_inclusive_and_ordered() {
    let (st, body, _) = get("/quran/v1/surahs/1/ayahs").await;
    assert_eq!(st, StatusCode::OK);
    let ayahs = data(&body)["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 7);
    assert_eq!(ayahs[0]["key"], "1:1");
    assert_eq!(ayahs[0]["globalIndex"], 1);
    assert_eq!(ayahs[0]["juz"], 1);
    let (_, body, _) = get("/quran/v1/surahs/2/ayahs?from=1&to=3").await;
    assert_eq!(data(&body)["ayahs"].as_array().unwrap().len(), 3);
    // from > to → 400, never clamped (§6.1).
    assert_eq!(
        get("/quran/v1/surahs/2/ayahs?from=3&to=1").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn single_ayah_redirect_alias_and_400() {
    let (st, body, _) = get("/quran/v1/ayahs/1/1").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(data(&body)["key"], "1:1");
    assert!(!data(&body)["text"].as_str().unwrap().is_empty());

    // /ayahs/2:255 → 308 to /quran/v1/ayahs/2/255 (§6.1).
    let resp = app()
        .await
        .oneshot(Request::builder().uri("/quran/v1/ayahs/2:255").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/quran/v1/ayahs/2/255");

    // /ayahs/abc → 400.
    assert_eq!(get("/quran/v1/ayahs/abc").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ayahs_keys_and_global_range() {
    let (st, body, _) = get("/quran/v1/ayahs?keys=2:255,1:1").await;
    assert_eq!(st, StatusCode::OK);
    let ayahs = data(&body)["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 2);
    assert_eq!(ayahs[0]["key"], "2:255");
    assert_eq!(ayahs[1]["key"], "1:1");

    let (_, body, _) = get("/quran/v1/ayahs?fromGlobal=1&toGlobal=7").await;
    assert_eq!(data(&body)["ayahs"].as_array().unwrap().len(), 7);

    // whole-Qur'an range without pagination → 400 range_too_large (§6.1).
    assert_eq!(
        get("/quran/v1/ayahs?fromGlobal=1&toGlobal=6236").await.0,
        StatusCode::BAD_REQUEST
    );

    // paginated: exactly 300, nextCursor set.
    let (_, body, _) = get("/quran/v1/ayahs?fromGlobal=1&toGlobal=6236&limit=300").await;
    assert_eq!(data(&body)["ayahs"].as_array().unwrap().len(), 300);
    assert_eq!(data(&body)["range"]["nextCursor"], 301);
}

#[tokio::test]
async fn script_param_default_and_validation() {
    let (_, _, h_uth) = get("/quran/v1/ayahs/1/1").await;
    let (_, _, h_sc) = get("/quran/v1/ayahs/1/1?script=simple-clean").await;
    // two requests differing only in script have different ETags (§8.1).
    assert_ne!(h_uth.get(header::ETAG).unwrap(), h_sc.get(header::ETAG).unwrap());
    assert_eq!(get("/quran/v1/ayahs/1/1?script=bogus").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unknown_query_param_rejected() {
    assert_eq!(
        get("/quran/v1/surahs/1/ayahs?bogus=1").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(get("/quran/v1/version?foo=1").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn method_not_allowed() {
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/quran/v1/surahs")
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
    let (_, _, headers) = get("/quran/v1/surahs/1/ayahs").await;
    let etag = headers.get(header::ETAG).unwrap().to_str().unwrap().to_owned();
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
                .uri("/quran/v1/surahs/1/ayahs")
                .header(header::IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    // §10: a 304 repeats the SAME ETag + Cache-Control (not merely their presence).
    assert_eq!(resp.headers().get(header::ETAG).unwrap(), etag.as_str());
    assert_eq!(resp.headers().get(header::CACHE_CONTROL).unwrap(), cc.as_str());
    assert_eq!(resp.headers().get(header::VARY).unwrap(), "Accept-Encoding");
}

#[tokio::test]
async fn cors_preflight_wildcard_no_credentials() {
    let app = app().await;
    // Preflight: wildcard origin, If-None-Match allowed, no credentials (§8.2).
    // (Expose-Headers is NOT carried on preflight — it is an actual-response
    // directive; verified on the GET below.)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/quran/v1/surahs")
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
        resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
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

    // Actual GET with an Origin: Expose-Headers must let JS read ETag (§8.2).
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/v1/surahs")
                .header(header::ORIGIN, "https://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
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
async fn version_and_health() {
    let (st, body, _) = get("/quran/v1/version").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(data(&body)["apiVersion"], "v1");
    assert!(data(&body)["sourceDigests"]["uthmani"].is_string());

    let (st, body, headers) = get("/quran/v1/health/ready").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["ready"], true);
    assert_eq!(body["verseCount"], 6236);
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
}

#[tokio::test]
async fn navigation_families_counts_and_hizb_derivation() {
    for (path, n) in [
        ("/quran/v1/juzs", 30),
        ("/quran/v1/pages", 604),
        ("/quran/v1/rukus", 556),
        ("/quran/v1/hizb-quarters", 240),
        ("/quran/v1/manzils", 7),
        ("/quran/v1/sajdas", 15),
    ] {
        let (st, body, _) = get(path).await;
        assert_eq!(st, StatusCode::OK, "{path}");
        assert_eq!(data(&body).as_array().unwrap().len(), n, "{path}");
    }
    let (_, b1, _) = get("/quran/v1/hizb-quarters/1").await;
    assert_eq!(data(&b1)["hizb"], 1);
    assert_eq!(data(&b1)["quarterInHizb"], 1);
    let (_, b5, _) = get("/quran/v1/hizb-quarters/5").await;
    assert_eq!(data(&b5)["hizb"], 2);
    assert_eq!(data(&b5)["quarterInHizb"], 1);
}

#[tokio::test]
async fn random_date_is_immutable_cache() {
    let (st, _, headers) = get("/quran/v1/random?date=2026-07-30").await;
    assert_eq!(st, StatusCode::OK);
    assert!(headers
        .get(header::CACHE_CONTROL)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("immutable"));
    assert_eq!(get("/quran/v1/random?date=2026-2-3").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(get("/quran/v1/random?date=2026-02-30").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_results_ordered_with_highlights_and_limits() {
    // "الحمد" percent-encoded — appears in 1:2 and elsewhere.
    let q = "%D8%A7%D9%84%D8%AD%D9%85%D8%AF";
    let (st, body, headers) = get(&format!("/quran/v1/search?q={q}")).await;
    assert_eq!(st, StatusCode::OK);
    let d = data(&body);
    assert!(d["total"].as_u64().unwrap() > 0, "expected matches");
    let results = d["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let mut prev: u64 = 0;
    for r in results {
        let g = r["ayah"]["globalIndex"].as_u64().unwrap();
        assert!(g >= prev, "results not ascending by globalIndex");
        prev = g;
        assert!(
            !r["highlights"].as_array().unwrap().is_empty(),
            "each hit has at least one highlight"
        );
    }
    assert_eq!(d["limit"], 20); // default
    assert_eq!(d["query"], "الحمد");
    // search is cacheable (§8.1).
    assert!(headers.contains_key(header::ETAG));

    // 2-scalar query → 400 (§7.1). "من" = م ن.
    let two = "%D9%85%D9%86";
    assert_eq!(
        get(&format!("/quran/v1/search?q={two}")).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn error_envelope_matches_section_6_4() {
    // 404 — `{error:{code,message,detail}}`, message always present (§6.4).
    let (st, body, _) = get("/quran/v1/surahs/115").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(!body["error"]["message"].as_str().unwrap().is_empty());

    // 400 unknown script.
    let (st, body, _) = get("/quran/v1/ayahs/1/1?script=bogus").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"]["code"].is_string());
    assert!(!body["error"]["message"].as_str().unwrap().is_empty());

    // 400 range_too_large with detail {max, requested}.
    let (st, body, _) = get("/quran/v1/ayahs?fromGlobal=1&toGlobal=6236").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "range_too_large");
    assert_eq!(body["error"]["detail"]["max"], 300);
    assert_eq!(body["error"]["detail"]["requested"], 6236);

    // 400 unknown query param (QQuery maps the extractor rejection to the envelope).
    let (st, body, _) = get("/quran/v1/surahs?bogus=1").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"]["code"].is_string());

    // 400 non-numeric path (QPath rejection → envelope, not axum's stock body).
    let (st, body, _) = get("/quran/v1/surahs/abc").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(body["error"]["code"].is_string());
}

#[tokio::test]
async fn cursor_pagination_page_two() {
    let (_, body, _) = get("/quran/v1/ayahs?fromGlobal=1&toGlobal=6236&limit=300").await;
    let next = data(&body)["range"]["nextCursor"].as_u64().unwrap();
    assert_eq!(next, 301);
    let (st, body2, _) = get(&format!(
        "/quran/v1/ayahs?fromGlobal=1&toGlobal=6236&limit=300&cursor={next}"
    ))
    .await;
    assert_eq!(st, StatusCode::OK);
    let ayahs = data(&body2)["ayahs"].as_array().unwrap();
    assert_eq!(ayahs.len(), 300);
    assert_eq!(ayahs[0]["globalIndex"], 301, "page 2 starts at global 301");
    assert_eq!(data(&body2)["range"]["nextCursor"], 601);
    // cursor past the window end → 400 (no empty 200 terminal).
    assert_eq!(
        get("/quran/v1/ayahs?fromGlobal=1&toGlobal=7&cursor=999").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn conditional_get_on_search_and_head_method() {
    // /search conditional GET (distinct ETag path via respond_cached_with_etag).
    let q = "%D8%A7%D9%84%D8%AD%D9%85%D8%AF";
    let (_, _, headers) = get(&format!("/quran/v1/search?q={q}")).await;
    let etag = headers.get(header::ETAG).unwrap().to_str().unwrap().to_owned();
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .uri(format!("/quran/v1/search?q={q}"))
                .header(header::IF_NONE_MATCH, &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    // §10: the 304 repeats the same ETag captured on the 200 (search path).
    assert_eq!(resp.headers().get(header::ETAG).unwrap(), etag.as_str());
    assert_eq!(resp.headers().get(header::VARY).unwrap(), "Accept-Encoding");

    // HEAD mirrors GET headers with an empty body (§10).
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri("/quran/v1/version")
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
    // Phase 1a exit #1 (§10): table-driven over the router's ENUMERATED routes —
    // every public route has wildcard CORS, no credentials, no Set-Cookie, and a
    // Quran route added to the router without an entry here FAILS this test.
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Mock the object store so `/scripts`'s HEAD checks resolve instantly — the
    // CORS contract must cover `/scripts` too, so it stays in the table.
    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let cl_u = state.quran.artifacts.uthmani.size_bytes.to_string();
    let cl_sc = state.quran.artifacts.simple_clean.size_bytes.to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/uthmani/.*"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/simple-clean/.*"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;
    let app = app_over(state);

    // One concrete request per `.route(` in mod.rs (§6.1). Keep in sync with
    // `modules/quran_v1/mod.rs` — `routes()` + `search_route()` (+ the openapi
    // route). `/ayahs/1:1` is the 308 verseKey alias. The `…/ayahs` variants
    // carry `?limit=300` so a large unit (e.g. manzil 1 ≈ 890 ayahs) returns 200
    // instead of `range_too_large` (400) — the CORS contract is route-level.
    #[rustfmt::skip]
    let routes = [
        "/quran/v1/surahs", "/quran/v1/surahs/1", "/quran/v1/surahs/1/ayahs?limit=300",
        "/quran/v1/ayahs?keys=1:1", "/quran/v1/ayahs/1:1", "/quran/v1/ayahs/1/1",
        "/quran/v1/juzs", "/quran/v1/juzs/1", "/quran/v1/juzs/1/ayahs?limit=300",
        "/quran/v1/pages", "/quran/v1/pages/2", "/quran/v1/pages/2/ayahs?limit=300",
        "/quran/v1/rukus", "/quran/v1/rukus/1", "/quran/v1/rukus/1/ayahs?limit=300",
        "/quran/v1/hizb-quarters", "/quran/v1/hizb-quarters/1", "/quran/v1/hizb-quarters/1/ayahs?limit=300",
        "/quran/v1/manzils", "/quran/v1/manzils/1", "/quran/v1/manzils/1/ayahs?limit=300",
        "/quran/v1/sajdas", "/quran/v1/sajdas/1",
        "/quran/v1/scripts", "/quran/v1/random", "/quran/v1/version", "/quran/v1/health/ready",
        "/quran/v1/openapi.json",
        "/quran/v1/search?q=%D8%A7%D9%84%D8%AD%D9%85%D8%AF",
    ];
    // Adding a `.route(` to mod.rs without a table entry trips this assertion.
    let route_count = include_str!("../src/modules/quran_v1/mod.rs").matches(".route(").count();
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
            resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "*",
            "{path} ACAO"
        );
        assert!(
            resp.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none(),
            "{path} must not allow credentials"
        );
        assert!(
            resp.headers().get(header::SET_COOKIE).is_none(),
            "{path} must not Set-Cookie (no private-layer leak)"
        );
    }
}

/// §10 No-SQLite-after-startup (structural bullet): modules/quran_v1/ must
/// contain no reference to sea_db / sqlx / rusqlite / SQLite types.
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
            assert!(!s.contains(b), "modules/quran_v1 file[{i}] references {b:?} (§10)");
        }
    }
}

/// Phase 1a exit #2 (§10, "not optional"): the split must NOT have broken the
/// private authed branch — a session cookie is still issued, off-origin is
/// still rejected, and the public `/quran/v1` branch stays cookie-free +
/// wildcard-CORS even under an off-origin request.
#[tokio::test]
async fn phase_1a_auth_regression_on_merged_router() {
    // csrf_guard derives its signing key from the COOKIE_KEY env var directly
    // (not from AppState), so the merged-router harness must provide it.
    std::env::set_var(
        "COOKIE_KEY",
        "test_cookie_key_padded_to_more_than_32_bytes_for_tests",
    );
    let app = full_app().await;

    // (1) POST /csrf/v1/generate (CSRF-exempt) bootstraps a session → Set-Cookie.
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

    // (2) origin_guard still rejects an off-origin request on the PRIVATE branch.
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

    // (3) The PUBLIC branch has wildcard CORS + no Set-Cookie even off-origin.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/v1/surahs")
                .header(header::ORIGIN, "https://evil.example.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "*"
    );
    assert!(
        resp.headers().get(header::SET_COOKIE).is_none(),
        "public branch must not set a session cookie"
    );
}

/// §10 HTTP: `/scripts` omits any artifact whose HEAD fails (here a
/// Content-Length != sizeBytes) and exposes the verified one; Axum never serves
/// the SQLite bytes (metadata only).
#[tokio::test]
async fn scripts_omits_failed_head_artifacts() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let uthmani_size = state.quran.artifacts.uthmani.size_bytes;
    let sc_size = state.quran.artifacts.simple_clean.size_bytes;

    // uthmani: 200 + correct Content-Length → advertised.
    let cl_u = uthmani_size.to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/uthmani/.*"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    // simple-clean: 200 but WRONG Content-Length → omitted (size mismatch).
    let cl_sc = (sc_size + 1).to_string();
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/simple-clean/.*"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;

    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/v1/scripts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let scripts = body["data"]["scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), 1, "only the verified (uthmani) artifact remains");
    assert_eq!(scripts[0]["id"], "uthmani");
    // metadata only — no SQLite bytes in the body.
    assert!(scripts[0]["downloadUrl"].as_str().unwrap().ends_with(".sqlite"));
}

/// §10 HTTP: `/scripts` happy path — when BOTH artifacts HEAD-verify, the
/// response advertises EXACTLY [uthmani, simple-clean] (the omission path is
/// covered by [`scripts_omits_failed_head_artifacts`]) and is long-cached, not
/// no-store.
#[tokio::test]
async fn scripts_happy_path_advertises_both_artifacts() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let state = state_with_public_url(&server.uri()).await;
    let cl_u = state.quran.artifacts.uthmani.size_bytes.to_string();
    let cl_sc = state.quran.artifacts.simple_clean.size_bytes.to_string();
    // Both artifacts: 200 + correct identity Content-Length → advertised.
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/uthmani/.*"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_u.as_str()))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path_regex(r".*/simple-clean/.*"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-length", cl_sc.as_str()))
        .mount(&server)
        .await;

    let app = app_over(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/quran/v1/scripts")
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
    assert_eq!(scripts.len(), 2, "happy path advertises exactly both artifacts");
    let ids: Vec<&str> = scripts.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"uthmani"), "uthmani advertised");
    assert!(ids.contains(&"simple-clean"), "simple-clean advertised");
    for s in scripts {
        assert!(
            s["downloadUrl"].as_str().unwrap().ends_with(".sqlite"),
            "download URL points at the artifact, not its bytes"
        );
    }
    // Fully-verified list is long-cached (§5.1), NOT no-store (the partial path).
    assert_eq!(
        cc.as_deref(),
        Some(ruxlog::modules::quran_v1::cache::ARABIC_CACHE)
    );
}

/// §10 HTTP: ranges are validated against the unit's length — out-of-bounds
/// `from`/`to` are 400 and never silently clamped (§6.1). Surah 2 has 286 ayahs.
#[tokio::test]
async fn range_out_of_bounds_is_400_not_clamped() {
    assert_eq!(
        get("/quran/v1/surahs/2/ayahs?from=1&to=999").await.0,
        StatusCode::BAD_REQUEST,
        "to beyond the unit length must be rejected"
    );
    assert_eq!(
        get("/quran/v1/surahs/2/ayahs?from=300&to=310").await.0,
        StatusCode::BAD_REQUEST,
        "from beyond the unit length must be rejected"
    );
    // Same rule on a navigation family's `…/ayahs` route.
    assert_eq!(
        get("/quran/v1/juzs/1/ayahs?from=1&to=9999").await.0,
        StatusCode::BAD_REQUEST
    );
    // Sanity: an in-bounds range still succeeds (the check is not over-eager).
    assert_eq!(get("/quran/v1/surahs/2/ayahs?from=1&to=3").await.0, StatusCode::OK);
}

/// §10 Verbatim: the served `text` equals the store's verbatim source value
/// byte-for-byte. The lib golden-digest test pins store↔sqlite source, so this
/// transitively pins API↔source — catching any controller/DTO/serialization
/// mutation of the text at the HTTP layer.
#[tokio::test]
async fn api_response_text_equals_store_source() {
    use ruxlog::quran::Script;

    let state = state().await;
    let app = app_over(state.clone());
    // (surah, ayah, script): 1:1 basmala-as-first-ayah, 2:1 embedded basmala,
    // 112:1, and a mid-Qur'an ayah — across both scripts.
    for (s, a, script) in [
        (1u16, 1u16, Script::Uthmani),
        (2u16, 1u16, Script::Uthmani),
        (112u16, 1u16, Script::SimpleClean),
        (55u16, 3u16, Script::SimpleClean),
    ] {
        let g = state.quran.meta().global_of(s, a).expect("valid ayah");
        let uri = format!("/quran/v1/ayahs/{s}/{a}?script={}", script.as_str());
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

/// §7.1/§10 Search: the result window never exceeds `limit`; a fixed query is
/// deterministic (identical ordered keys across calls); `offset` paginates.
#[tokio::test]
async fn search_respects_limit_offset_and_is_stable() {
    let q = "%D8%A7%D9%84%D8%AD%D9%85%D8%AF"; // الحمد
    let (_, b1, _) = get(&format!("/quran/v1/search?q={q}&limit=5")).await;
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

    // Pure function of the corpus → identical ordered keys on repeat.
    let (_, b2, _) = get(&format!("/quran/v1/search?q={q}&limit=5")).await;
    let keys2: Vec<String> = data(&b2)["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["ayah"]["key"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(keys1, keys2, "fixed query must be deterministic");

    // offset=1 begins at the 2nd base hit (page consistency).
    if keys1.len() > 1 {
        let (_, bo, _) = get(&format!("/quran/v1/search?q={q}&limit=5&offset=1")).await;
        let first_at_offset =
            data(&bo)["results"].as_array().unwrap()[0]["ayah"]["key"].as_str().unwrap();
        assert_eq!(first_at_offset, keys1[1], "offset=1 begins at the 2nd base hit");
    }
}
