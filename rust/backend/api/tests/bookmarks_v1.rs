// Offline-sync bookmark rounds over HTTP: one POST /bookmark/v1/sync does
// push-then-pull (LWW mutations applied FIFO, then a full snapshot back as
// server truth) inside a single transaction. Covers auth, empty rounds, replay
// idempotency, LWW upserts and deletes, verse-collapse of two-uuid collisions
// (one verse = one row), folder-delete detaching children to root,
// same-batch delete-then-reference of a dead folder, mixed-format legacy
// timestamps resolving by instant, validation caps, and cross-user isolation
// of client-minted ids.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use ruxlog::config::{
    HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings, Settings,
    SiteSettings,
};
use ruxlog::modules::bookmark_v1;
use ruxlog::quran::load_quran_store;
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};

use migration::{Migrator, MigratorTrait};
use rux_auth::AuthSession as GenAuthSession;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::services::auth::AuthBackend;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseBackend, Statement};

// ruxlog is built without cfg(test) when linked into an integration test
// binary; pin the env once so is_production() takes the dev path.
static DEV_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_dev_env() {
    DEV_ENV.get_or_init(|| std::env::set_var("RUST_ENV", "development"));
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

// Real bookmark_v1 router + the wrapping layers production uses:
// SessionManagerLayer (provides Session) + Extension<AppState> (the from_fn
// guard extracts both). No CSRF layer here — same as the admin_bans_export
// harness — so failures under test are the auth guard / handler alone.
fn router_with_state(state: AppState) -> axum::Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    bookmark_v1::routes()
        .layer(axum::Extension(state.clone()))
        .layer(session_layer)
        .with_state(state)
}

async fn seed_user(state: &AppState, name: &str) -> user::Model {
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
// cookie the SessionManagerLayer would issue (see admin_bans_export.rs).
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

async fn logged_in_app() -> (axum::Router, String) {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state, "reader").await;
    let cookie = login_cookie(&state, &u).await;
    (router_with_state(state), cookie)
}

async fn post_sync(app: &axum::Router, cookie: &str, body: Value) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sync")
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

const FOLDER_ID: &str = "11111111-1111-4111-8111-111111111111";
const BMARK_IN_FOLDER: &str = "22222222-2222-4222-8222-222222222222";
const BMARK_ROOT: &str = "33333333-3333-4333-8333-333333333333";
// Two uuids two devices minted for the SAME verse (verse-collapse tests).
const BMARK_VERSE_A: &str = "55555555-5555-4555-8555-555555555555";
const BMARK_VERSE_B: &str = "66666666-6666-4666-8666-666666666666";
const BMARK_AFTER_FOLDER_DELETE: &str = "77777777-7777-4777-8777-777777777777";
const BMARK_LEGACY_OLDER: &str = "88888888-8888-4888-8888-888888888888";
const BMARK_RFC3339: &str = "99999999-9999-4999-8999-999999999999";
const BMARK_LEGACY_NEWER: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const BMARK_STALE: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
// Strictly ordered client clocks.
const T0: &str = "2026-08-26T10:00:00.000000Z";
const T1: &str = "2026-08-27T10:00:00.000000Z";
const T2: &str = "2026-08-27T11:00:00.000000Z";

fn folder_upsert(id: &str, name: &str, ts: &str) -> Value {
    json!({ "kind": "folder.upsert", "id": id, "name": name, "updatedAt": ts })
}

fn folder_delete(id: &str, ts: &str) -> Value {
    json!({ "kind": "folder.delete", "id": id, "updatedAt": ts })
}

fn bookmark_upsert(id: &str, folder_id: Value, surah: i32, ayah: i32, ts: &str) -> Value {
    json!({
        "kind": "bookmark.upsert",
        "id": id,
        "folderId": folder_id,
        "surah": surah,
        "ayah": ayah,
        "updatedAt": ts,
    })
}

fn bookmark_delete(id: &str, ts: &str) -> Value {
    json!({ "kind": "bookmark.delete", "id": id, "updatedAt": ts })
}

fn seeded_round() -> Vec<Value> {
    vec![
        folder_upsert(FOLDER_ID, "Tafsir", T1),
        bookmark_upsert(BMARK_IN_FOLDER, json!(FOLDER_ID), 2, 255, T1),
        bookmark_upsert(BMARK_ROOT, Value::Null, 1, 1, T1),
    ]
}

fn find<'a>(items: &'a Value, id: &str) -> &'a Value {
    items
        .as_array()
        .expect("array")
        .iter()
        .find(|item| item["id"] == json!(id))
        .unwrap_or_else(|| panic!("item {id} present in {items}"))
}

// Seed a row with a raw LEGACY updated_at ('YYYY-MM-DD HH:MM:SS' — the shape
// CURRENT_TIMESTAMP defaults produce) so LWW must resolve by instant, not by
// comparing the TEXT lexemes against the canonical RFC3339-micros pushes.
async fn raw_insert_legacy_bookmark(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    id: &str,
    surah: i32,
    ayah: i32,
    updated_at: &str,
) {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
             VALUES ('{id}', {user_id}, {surah}, {ayah}, '{updated_at}', '{updated_at}')"
        ),
    ))
    .await
    .expect("raw legacy bookmark insert");
}

#[tokio::test]
async fn verse_collision_newer_uuid_collapses_the_older_row() {
    let (app, cookie) = logged_in_app().await;

    // Device 1 mints BMARK_VERSE_A; device 2 later mints BMARK_VERSE_B for the
    // SAME verse. The newer mutation wins and the older rival row is
    // collapsed away — the snapshot holds exactly one row for the verse.
    let (status, _) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_VERSE_A, Value::Null, 1, 1, T1)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_VERSE_B, Value::Null, 1, 1, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["applied"], json!(1));
    assert_eq!(body["skipped"], json!(0));
    assert_eq!(
        body["bookmarks"].as_array().map(Vec::len),
        Some(1),
        "one verse = one live row: {body}"
    );
    assert_eq!(
        body["bookmarks"][0]["id"],
        json!(BMARK_VERSE_B),
        "the newer uuid survives the collapse"
    );

    // Deleting the visible bookmark cannot resurrect the collapsed rival.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_delete(BMARK_VERSE_B, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    assert_eq!(
        body["bookmarks"],
        json!([]),
        "no resurrection via the collapsed rival row"
    );
}

#[tokio::test]
async fn verse_collision_stale_uuid_is_skipped_original_survives() {
    let (app, cookie) = logged_in_app().await;

    let (status, _) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_VERSE_B, Value::Null, 1, 1, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // A second uuid for the same verse with an OLDER timestamp is stale: the
    // newer row survives untouched and the push counts as skipped.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_VERSE_A, Value::Null, 1, 1, T1)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(1));
    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        body["bookmarks"][0]["id"],
        json!(BMARK_VERSE_B),
        "the original newer row survives the stale rival"
    );
}

#[tokio::test]
async fn same_batch_folder_delete_then_upsert_with_dead_folder_lands_in_root() {
    let (app, cookie) = logged_in_app().await;
    let (status, _) = post_sync(&app, &cookie, json!({ "mutations": seeded_round() })).await;
    assert_eq!(status, StatusCode::OK);

    // Same round: the folder dies first, then a bookmark references the dead
    // folderId. The tx-serialized resolve must file it under root, not trip
    // the FK — 200, applied, no partial state.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [
            folder_delete(FOLDER_ID, T2),
            bookmark_upsert(BMARK_AFTER_FOLDER_DELETE, json!(FOLDER_ID), 4, 4, T2),
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["applied"], json!(2));
    assert_eq!(body["folders"], json!([]), "folder is gone");
    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(3));
    assert_eq!(
        find(&body["bookmarks"], BMARK_AFTER_FOLDER_DELETE)["folderId"],
        Value::Null,
        "upsert referencing the dead folderId lands in root"
    );
    assert_eq!(
        find(&body["bookmarks"], BMARK_IN_FOLDER)["folderId"],
        Value::Null,
        "the folder's earlier children are detached too"
    );
}

#[tokio::test]
async fn mixed_format_timestamps_resolve_lww_by_instant() {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state, "legacy").await;
    let cookie = login_cookie(&state, &u).await;
    let sea_db = state.sea_db.clone();
    let app = router_with_state(state);

    // Legacy row OLDER than the incoming RFC3339 push: the verse-collapse
    // delete consumes it and the push applies.
    raw_insert_legacy_bookmark(
        &sea_db,
        u.id,
        BMARK_LEGACY_OLDER,
        7,
        7,
        "2026-01-05 12:00:00",
    )
    .await;
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [
            bookmark_upsert(BMARK_RFC3339, Value::Null, 7, 7, "2026-01-05T12:00:01.000000Z"),
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["applied"], json!(1));
    assert_eq!(
        body["bookmarks"].as_array().map(Vec::len),
        Some(1),
        "the older legacy row was collapsed away: {body}"
    );
    assert_eq!(body["bookmarks"][0]["id"], json!(BMARK_RFC3339));

    // Legacy row NEWER than the incoming push: lexically ' ' < 'T' would rank
    // the legacy text OLDER and wrongly collapse it, but by instant it is
    // newer — the push is stale and the legacy row survives.
    raw_insert_legacy_bookmark(
        &sea_db,
        u.id,
        BMARK_LEGACY_NEWER,
        8,
        8,
        "2026-01-05 12:30:00",
    )
    .await;
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [
            bookmark_upsert(BMARK_STALE, Value::Null, 8, 8, "2026-01-05T12:00:01.000000Z"),
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(1));
    assert_eq!(
        find(&body["bookmarks"], BMARK_LEGACY_NEWER)["surah"],
        json!(8),
        "the newer-instant legacy row survives the older RFC3339 push"
    );
    assert_eq!(
        body["bookmarks"].as_array().map(Vec::len),
        Some(2),
        "no third row minted for verse (8, 8)"
    );
}

#[tokio::test]
async fn unauthenticated_request_is_rejected() {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let app = router_with_state(state);
    let (status, _) = post_sync(&app, "ruxlog.sid=not-a-real-session", json!({})).await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "no session cookie must be rejected, got {status}"
    );
}

#[tokio::test]
async fn empty_sync_returns_empty_snapshot() {
    let (app, cookie) = logged_in_app().await;
    let (status, body) = post_sync(&app, &cookie, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(0));
    assert_eq!(body["folders"], json!([]));
    assert_eq!(body["bookmarks"], json!([]));
}

#[tokio::test]
async fn push_then_pull_applies_and_snapshots() {
    let (app, cookie) = logged_in_app().await;
    let (status, body) = post_sync(&app, &cookie, json!({ "mutations": seeded_round() })).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["applied"], json!(3));
    assert_eq!(body["skipped"], json!(0));

    assert_eq!(body["folders"].as_array().map(Vec::len), Some(1));
    let folder = find(&body["folders"], FOLDER_ID);
    assert_eq!(folder["name"], json!("Tafsir"));
    assert!(folder["createdAt"].is_string(), "createdAt is RFC3339");
    assert!(folder["updatedAt"].is_string(), "updatedAt is RFC3339");

    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        find(&body["bookmarks"], BMARK_IN_FOLDER)["folderId"],
        json!(FOLDER_ID),
        "foldered bookmark carries its folder id"
    );
    assert_eq!(
        find(&body["bookmarks"], BMARK_ROOT)["folderId"],
        Value::Null,
        "root bookmark serializes folderId as null"
    );
    assert_eq!(find(&body["bookmarks"], BMARK_IN_FOLDER)["surah"], json!(2));
    assert_eq!(
        find(&body["bookmarks"], BMARK_IN_FOLDER)["ayah"],
        json!(255)
    );
}

#[tokio::test]
async fn replayed_round_changes_nothing() {
    let (app, cookie) = logged_in_app().await;
    let (status, first) = post_sync(&app, &cookie, json!({ "mutations": seeded_round() })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["applied"], json!(3));

    // Same timestamps replay: strictly-newer guard makes every upsert a no-op.
    let (status, replay) = post_sync(&app, &cookie, json!({ "mutations": seeded_round() })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["applied"], json!(0), "replay applies nothing");
    assert_eq!(replay["skipped"], json!(3));
    assert_eq!(replay["folders"].as_array().map(Vec::len), Some(1));
    assert_eq!(replay["bookmarks"].as_array().map(Vec::len), Some(2));
}

#[tokio::test]
async fn lww_upsert_skips_stale_and_applies_newer() {
    let (app, cookie) = logged_in_app().await;
    let (status, _) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 1, 1, T1)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Older timestamp: skipped, stored state untouched.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 3, 7, T0)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(1));
    let b = find(&body["bookmarks"], BMARK_ROOT);
    assert_eq!(b["surah"], json!(1), "stale upsert must not change surah");
    assert_eq!(b["ayah"], json!(1), "stale upsert must not change ayah");

    // Newer timestamp: applied.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 3, 7, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    let b = find(&body["bookmarks"], BMARK_ROOT);
    assert_eq!(b["surah"], json!(3), "newer upsert overwrites surah");
    assert_eq!(b["ayah"], json!(7), "newer upsert overwrites ayah");
}

#[tokio::test]
async fn folder_delete_detaches_children_to_root() {
    let (app, cookie) = logged_in_app().await;
    let (status, _) = post_sync(&app, &cookie, json!({ "mutations": seeded_round() })).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [folder_delete(FOLDER_ID, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    assert_eq!(
        body["folders"],
        json!([]),
        "deleted folder is gone from the snapshot"
    );
    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        find(&body["bookmarks"], BMARK_IN_FOLDER)["folderId"],
        Value::Null,
        "child bookmark survives the folder delete, filed under root"
    );
}

#[tokio::test]
async fn bookmark_delete_respects_lww() {
    let (app, cookie) = logged_in_app().await;
    let (status, _) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 1, 1, T1)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Delete older than the row: refused, bookmark survives.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_delete(BMARK_ROOT, T0)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(1));
    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(1));

    // Delete newer than the row: applied.
    let (status, body) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_delete(BMARK_ROOT, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    assert_eq!(body["bookmarks"], json!([]), "deleted bookmark is gone");
}

#[tokio::test]
async fn validation_errors_reject_the_round() {
    let (app, cookie) = logged_in_app().await;

    let (status, _) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 0, 1, T1)] }),
    )
    .await;
    assert!(
        status.is_client_error(),
        "surah 0 is out of range, got {status}"
    );

    let (status, _) = post_sync(
        &app,
        &cookie,
        json!({ "mutations": [folder_upsert(FOLDER_ID, &"x".repeat(101), T1)] }),
    )
    .await;
    assert!(
        status.is_client_error(),
        "101-char name is out of range, got {status}"
    );

    let too_many: Vec<Value> = (0..201)
        .map(|i| {
            bookmark_upsert(
                &format!("{:08x}-0000-4000-8000-{:012x}", i, i),
                Value::Null,
                1,
                1,
                T1,
            )
        })
        .collect();
    let (status, _) = post_sync(&app, &cookie, json!({ "mutations": too_many })).await;
    assert!(
        status.is_client_error(),
        "201 mutations exceed the cap, got {status}"
    );
}

#[tokio::test]
async fn cross_user_ids_are_isolated() {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let a = seed_user(&state, "alice").await;
    let b = seed_user(&state, "bob").await;
    let cookie_a = login_cookie(&state, &a).await;
    let cookie_b = login_cookie(&state, &b).await;
    let app = router_with_state(state);

    // Alice owns the id first.
    let (status, body) = post_sync(
        &app,
        &cookie_a,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 1, 1, T1)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));

    // Bob pushes the SAME id with a NEWER timestamp: the ownership guard in
    // the DO UPDATE clause must make it a no-op, not a hijack.
    let (status, body) = post_sync(
        &app,
        &cookie_b,
        json!({ "mutations": [bookmark_upsert(BMARK_ROOT, Value::Null, 9, 9, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(0), "foreign id must not be applied");
    assert_eq!(body["skipped"], json!(1));
    assert_eq!(body["bookmarks"], json!([]), "Bob's snapshot has no rows");

    // Bob's own rows sync fine and never leak into Alice's snapshot.
    let bob_id = "44444444-4444-4444-8444-444444444444";
    let (status, body) = post_sync(
        &app,
        &cookie_b,
        json!({ "mutations": [bookmark_upsert(bob_id, Value::Null, 5, 5, T2)] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(1));

    let (status, body) = post_sync(&app, &cookie_a, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["bookmarks"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        find(&body["bookmarks"], BMARK_ROOT)["surah"],
        json!(1),
        "Alice's row keeps her data despite Bob's newer foreign upsert"
    );
}
