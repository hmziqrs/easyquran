// Integration tests for the opt-in first-party reading analytics pipeline
// (next-features Feature 6). Direct-function half: `ingest_events` applies a
// whole batch in ONE transaction (per-surah verse validation against the
// Quran meta, rollup fold, retention purge) and `summary_for_user` answers
// most-read surahs/ayahs, totals, and day counts from the retention-proof
// rollup. HTTP half (same session harness shape as bookmarks_v1 /
// reading_session_v1): the `auth_guard::verified` route_layer, the REQUIRED
// expectedUserId account check (403 before any write), the full
// POST/GET round trip over the real router, and the router.rs rate-limit
// layer shape (429 + Retry-After once the matched-path bucket is full).
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;
use validator::Validate;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::middlewares::rate_limit;
use ruxlog::modules::user_analytics_v1;
use ruxlog::modules::user_analytics_v1::controller::{ingest_events, summary_for_user};
use ruxlog::modules::user_analytics_v1::validator::V1EventsPayload;
use ruxlog::quran::{load_quran_store, QuranStore};
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};

use migration::{Migrator, MigratorTrait};
use rux_auth::AuthSession as GenAuthSession;
use ruxlog::services::auth::AuthBackend;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

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

async fn db() -> DatabaseConnection {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.expect("migrations");
    db
}

async fn quran() -> QuranStore {
    load_quran_store(&quran_settings())
        .await
        .expect("quran store loads")
}

async fn seed_user(db: &DatabaseConnection, name: &str) -> user::Model {
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
    .insert(db)
    .await
    .expect("user seeded")
}

const D1: &str = "2026-10-10";
const D2: &str = "2026-10-09";
const AT: &str = "2026-10-10T08:00:00.000000Z";

fn read(surah: i32, ayah: i32, seconds: i32, day: &str) -> Value {
    json!({ "kind": "ayah.read", "surah": surah, "ayah": ayah,
            "sourceId": "en.sahih", "seconds": seconds, "localDay": day, "at": AT })
}

fn open(surah: i32, ayah: i32, day: &str) -> Value {
    json!({ "kind": "ayah.open", "surah": surah, "ayah": ayah,
            "localDay": day, "at": AT })
}

fn payload(uid: i32, events: Vec<Value>) -> V1EventsPayload {
    serde_json::from_value(json!({ "expectedUserId": uid, "events": events }))
        .expect("payload shape is valid")
}

#[tokio::test]
async fn ingest_and_summary_round_trip() {
    let db = db().await;
    let quran = quran().await;
    let uid = seed_user(&db, "reader").await.id;

    let accepted = ingest_events(
        &db,
        &quran,
        uid,
        &payload(
            uid,
            vec![
                read(2, 255, 30, D1),
                read(2, 255, 10, D1),
                read(1, 1, 5, D2),
                open(2, 255, D1),
            ],
        ),
    )
    .await
    .expect("batch accepted");
    assert_eq!(accepted, 4);

    let s = summary_for_user(&db, uid).await.expect("summary");
    // Raw totals include opens; seconds come from reads only.
    assert_eq!(s["totals"], json!({ "events": 4, "seconds": 45 }));
    // Rollup reads: surah 2 read twice, surah 1 once; deterministic ties.
    assert_eq!(
        s["topSurahs"],
        json!([
            { "surah": 2, "reads": 2 }, { "surah": 1, "reads": 1 }
        ])
    );
    // Top ayahs count READ events; the open never inflates 2:255.
    assert_eq!(
        s["topAyahs"],
        json!([
            { "surah": 2, "ayah": 255, "reads": 2 },
            { "surah": 1, "ayah": 1, "reads": 1 }
        ])
    );
    // Day counts from the rollup, newest first.
    assert_eq!(
        s["days"],
        json!([
            { "localDay": D1, "ayahsRead": 2, "seconds": 40 },
            { "localDay": D2, "ayahsRead": 1, "seconds": 5 },
        ])
    );
}

#[tokio::test]
async fn verse_past_its_surah_count_rejects_whole_batch() {
    let db = db().await;
    let quran = quran().await;
    let uid = seed_user(&db, "reader").await.id;

    // 1:8 does not exist (Al-Fatihah has 7 ayahs) — the flat 1..=286 bound
    // alone would let it through; the per-surah check must not.
    let err = ingest_events(
        &db,
        &quran,
        uid,
        &payload(uid, vec![read(2, 255, 30, D1), read(1, 8, 5, D1)]),
    )
    .await;
    assert!(err.is_err(), "1:8 must reject the round");
    // All-or-nothing: the valid 2:255 in the same batch never landed.
    let s = summary_for_user(&db, uid).await.expect("summary");
    assert_eq!(s["totals"]["events"], json!(0));
}

#[tokio::test]
async fn validator_caps_reject_bad_batches() {
    let uid = 1;
    let cases: Vec<(&str, Value)> = vec![
        (
            "invalid calendar day",
            json!({ "expectedUserId": uid, "events": [
            read(2, 255, 30, "2026-13-01")] }),
        ),
        (
            "seconds over the 1h cap",
            json!({ "expectedUserId": uid, "events": [
            read(2, 255, 3601, D1)] }),
        ),
        (
            "sourceId charset",
            json!({ "expectedUserId": uid, "events": [
            read(2, 255, 30, D1)] , "dummy": 0 }),
        ),
        (
            "flat surah bound",
            json!({ "expectedUserId": uid, "events": [
            read(0, 1, 30, D1)] }),
        ),
    ];
    for (label, mut body) in cases {
        if label == "sourceId charset" {
            body["events"][0]["sourceId"] = json!("EN.SAHIH");
        }
        let parsed = serde_json::from_value::<V1EventsPayload>(body.clone()).expect("shape parses");
        assert!(parsed.validate().is_err(), "{label} must fail validation");
    }
    // Unknown kinds never deserialize (closed literal set).
    let mut jumped = open(2, 255, D1);
    jumped["kind"] = json!("ayah.jump");
    assert!(
        serde_json::from_value::<V1EventsPayload>(json!({
            "expectedUserId": uid, "events": [jumped]
        }))
        .is_err(),
        "unknown kind must fail deserialization"
    );
    // Batch cap: 101 events fails; an empty batch is fine (accepted 0).
    let oversized = json!({ "expectedUserId": uid,
        "events": (0..101).map(|_| read(2, 255, 1, D1)).collect::<Vec<_>>() });
    let parsed = serde_json::from_value::<V1EventsPayload>(oversized).unwrap();
    assert!(parsed.validate().is_err(), "batch over 100 must fail");
    let empty = payload(uid, vec![]);
    assert!(empty.validate().is_ok());
}

#[tokio::test]
async fn isolation_retention_purge_and_user_delete_cascade() {
    let db = db().await;
    let quran = quran().await;
    let a = seed_user(&db, "readera").await.id;
    let b = seed_user(&db, "readerb").await.id;

    ingest_events(
        &db,
        &quran,
        a,
        &payload(a, vec![read(2, 255, 30, D1), read(2, 255, 10, D1)]),
    )
    .await
    .unwrap();
    // A stale raw event far past the 90-day window.
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"INSERT INTO reading_events (user_id, surah, ayah, kind, local_day, created_at)
           VALUES (?, 2, 200, 'ayah.read', ?, '2020-01-01T00:00:00.000000Z')"#,
        [a.into(), D1.into()],
    ))
    .await
    .unwrap();

    // The next ingest purges the stale raw row but keeps the rollup.
    ingest_events(&db, &quran, a, &payload(a, vec![read(1, 1, 5, D1)]))
        .await
        .unwrap();
    let s = summary_for_user(&db, a).await.unwrap();
    assert_eq!(
        s["totals"]["events"],
        json!(3),
        "purged raw row no longer counted"
    );
    assert_eq!(
        s["topSurahs"][0],
        json!({ "surah": 2, "reads": 2 }),
        "rollup survives purge"
    );

    // B never sees A's history.
    let s = summary_for_user(&db, b).await.unwrap();
    assert_eq!(s["totals"], json!({ "events": 0, "seconds": 0 }));
    assert_eq!(s["topSurahs"], json!([]));

    // Account deletion purges events + rollup (FK ON DELETE CASCADE).
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "DELETE FROM users WHERE id = ?",
        [a.into()],
    ))
    .await
    .unwrap();
    let events = count(
        &db,
        "SELECT COUNT(*) AS c FROM reading_events WHERE user_id = ?",
    )
    .await;
    let daily = count(
        &db,
        "SELECT COUNT(*) AS c FROM reading_daily WHERE user_id = ?",
    )
    .await;
    assert_eq!((events, daily), (0, 0), "cascade purged both tables");
}

async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        sql,
        [1.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get::<i64>("", "c")
    .unwrap()
}

// ---------------------------------------------------------------------------
// HTTP half — full AppState + real user_analytics_v1 router (same harness
// shape as bookmarks_v1 / reading_session_v1).
// ---------------------------------------------------------------------------

async fn http_state() -> AppState {
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

// Real user_analytics_v1 router + the wrapping layers production uses
// (SessionManagerLayer + Extension<AppState>); no CSRF layer, same as the
// bookmarks_v1 harness, so failures under test are the auth guard / handler
// alone.
fn router_with_state(state: AppState) -> axum::Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    user_analytics_v1::routes()
        .layer(axum::Extension(state.clone()))
        .layer(session_layer)
        .with_state(state)
}

// Establish a real authenticated session in the SAME store the router uses,
// via the real AuthSession::login path, then mint the PRIVATE ruxlog.sid
// cookie the SessionManagerLayer would issue (see bookmarks_v1.rs).
async fn login_cookie(state: &AppState, u: &user::Model) -> String {
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

async fn logged_in_app() -> (axum::Router, String, i32) {
    let state = http_state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state.sea_db, "reader").await;
    let cookie = login_cookie(&state, &u).await;
    (router_with_state(state), cookie, u.id)
}

async fn post_events(app: &axum::Router, cookie: &str, body: Value) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get_summary(app: &axum::Router, cookie: &str) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/summary")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn http_payload(uid: i32, events: Vec<Value>) -> Value {
    json!({ "expectedUserId": uid, "events": events })
}

#[tokio::test]
async fn unauthenticated_requests_are_rejected() {
    let state = http_state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let app = router_with_state(state);
    let (status, _) = post_events(
        &app,
        "ruxlog.sid=not-a-real-session",
        http_payload(1, vec![]),
    )
    .await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "POST /events without a session must be rejected, got {status}"
    );
    let (status, _) = get_summary(&app, "ruxlog.sid=not-a-real-session").await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "GET /summary without a session must be rejected, got {status}"
    );
}

#[tokio::test]
async fn expected_user_id_mismatch_is_rejected_before_writes() {
    let (app, cookie, uid) = logged_in_app().await;
    // A stale tab posting another account's id: 403, and nothing lands.
    let (status, _) = post_events(
        &app,
        &cookie,
        http_payload(uid + 1, vec![read(2, 255, 30, D1)]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "account change must 403");
    let (status, body) = get_summary(&app, &cookie).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["totals"], json!({ "events": 0, "seconds": 0 }));
}

#[tokio::test]
async fn events_and_summary_round_trip_over_http() {
    let (app, cookie, uid) = logged_in_app().await;

    let (status, body) = post_events(
        &app,
        &cookie,
        http_payload(
            uid,
            vec![read(2, 255, 30, D1), read(2, 255, 10, D1), open(1, 1, D2)],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["accepted"], json!(3));

    let (status, body) = get_summary(&app, &cookie).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["totals"], json!({ "events": 3, "seconds": 40 }));
    // The surah-1 event was an OPEN — raw totals count it, the rollup (and
    // therefore most-read) does not.
    assert_eq!(body["topSurahs"], json!([{ "surah": 2, "reads": 2 }]));

    // Per-surah validation over HTTP: 1:8 does not exist; the round is a 400
    // and the stored history is unchanged (all-or-nothing batch).
    let (status, _) = post_events(&app, &cookie, http_payload(uid, vec![read(1, 8, 5, D1)])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "1:8 must 400");
    let (_, body) = get_summary(&app, &cookie).await;
    assert_eq!(
        body["totals"]["events"],
        json!(3),
        "rejected batch landed nothing"
    );
}

#[tokio::test]
async fn events_are_rate_limited_per_route_bucket() {
    let state = http_state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state.sea_db, "limited").await;
    let cookie = login_cookie(&state, &u).await;
    // Same layer shape router.rs wraps the nest in — small cap so the test
    // does not need 101 round trips to cross it.
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    let app = user_analytics_v1::routes()
        .layer(rate_limit::rate_limit_layer(&state, 3, 60))
        .layer(axum::Extension(state.clone()))
        .layer(session_layer)
        .with_state(state);

    for i in 0..3 {
        let (status, body) =
            post_events(&app, &cookie, http_payload(u.id, vec![read(2, 255, i, D1)])).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "request {i} within cap, body: {body}"
        );
    }
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::to_string(&http_payload(u.id, vec![read(2, 255, 1, D1)])).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS, "cap crossed");
    assert!(
        res.headers().get("retry-after").is_some(),
        "429 carries Retry-After"
    );
}
