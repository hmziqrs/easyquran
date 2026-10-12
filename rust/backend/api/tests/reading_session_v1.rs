// Reading-session sync rounds over HTTP: one POST /reading-session/v1/sync
// does push-then-pull (LWW/MAX mutations applied FIFO, then a full snapshot
// back as server truth) inside a single transaction. Covers auth, the
// REQUIRED expectedUserId account check, empty + full round-trips, progress
// MAX-merge (EC-05), same-session LWW convergence + stale replay skip
// (EC-21), crafted foreign session ids isolated per account (EC-52), session
// delete dropping positions/progress and re-homing scoped bookmarks with
// collision tombstones (EC-53), the 50-session quota, the future-clock clamp
// (EC-34), and the validation caps (§9.1/§9.2).
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
use ruxlog::modules::reading_session_v1;
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

// Real reading_session_v1 router + the wrapping layers production uses
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
    reading_session_v1::routes()
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
        .add(tower_sessions::cookie::Cookie::new("ruxlog.sid", session_id));
    jar.get("ruxlog.sid")
        .expect("forged session cookie present")
        .to_string()
}

async fn logged_in_app() -> (axum::Router, String, i32) {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state, "reader").await;
    let cookie = login_cookie(&state, &u).await;
    (router_with_state(state), cookie, u.id)
}

/// Two accounts on one router for the cross-account isolation scenarios.
async fn two_user_app() -> (axum::Router, String, i32, String, i32) {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let a = seed_user(&state, "readera").await;
    let b = seed_user(&state, "readerb").await;
    let cookie_a = login_cookie(&state, &a).await;
    let cookie_b = login_cookie(&state, &b).await;
    (
        router_with_state(state.clone()),
        cookie_a,
        a.id,
        cookie_b,
        b.id,
    )
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

// Client-minted session ids (dashed-hex UUID shape; the UUIDv5 account
// default passes the same check, §9.1).
const SES_DAILY: &str = "16161616-1616-4161-8161-161616161616";
const SES_STUDY: &str = "17171717-1717-4171-8171-171717171717";
const SES_RENAMED: &str = "18181818-1818-4181-8181-181818181818";

/// Distinct client-minted ids for the quota round.
fn quota_id(i: u32) -> String {
    format!("{i:08x}-0000-4000-8000-000000000000")
}
// Strictly ordered client clocks.
const T0: &str = "2026-08-26T10:00:00.000000Z";
const T1: &str = "2026-08-27T10:00:00.000000Z";
const T2: &str = "2026-08-27T11:00:00.000000Z";
const T3: &str = "2026-08-28T10:00:00.000000Z";

fn round(uid: i32, mutations: Vec<Value>) -> Value {
    json!({ "expectedUserId": uid, "mutations": mutations })
}

fn session_upsert(id: &str, name: &str, ts: &str) -> Value {
    json!({ "kind": "session.upsert", "id": id, "name": name, "updatedAt": ts })
}

fn session_upsert_full(
    id: &str,
    name: &str,
    kind: &str,
    translation_id: Value,
    archived_at: Value,
    ts: &str,
) -> Value {
    json!({
        "kind": "session.upsert",
        "id": id,
        "name": name,
        "sessionKind": kind,
        "translationId": translation_id,
        "archivedAt": archived_at,
        "updatedAt": ts,
    })
}

fn session_delete(id: &str, ts: &str) -> Value {
    json!({ "kind": "session.delete", "id": id, "updatedAt": ts })
}

fn position_upsert(session_id: &str, surah: i32, ayah: i32, ts: &str) -> Value {
    json!({
        "kind": "position.upsert",
        "sessionId": session_id,
        "surah": surah,
        "ayah": ayah,
        "localPage": 3,
        "ratio": 0.5,
        "sourceId": "en.sahih",
        "updatedAt": ts,
    })
}

fn progress_upsert(session_id: &str, surah: i32, furthest: i32, ts: &str) -> Value {
    json!({
        "kind": "progress.upsert",
        "sessionId": session_id,
        "surah": surah,
        "furthestAyah": furthest,
        "updatedAt": ts,
    })
}

fn find<'a>(items: &'a Value, id: &str) -> &'a Value {
    items
        .as_array()
        .expect("array")
        .iter()
        .find(|item| item["id"] == json!(id))
        .unwrap_or_else(|| panic!("item {id} present in {items}"))
}

/// Seed a bookmark row with raw SQL (canonical LWW text timestamps) so the
/// session-delete re-home path can be observed against pre-existing rows.
async fn raw_insert_bookmark(
    db: &sea_orm::DatabaseConnection,
    rows: &[(&str, &str, i32, i32)],
) {
    for &(id, session_id, surah, ayah) in rows {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO bookmarks (id, user_id, folder_id, session_id, source_id,
                                      surah, ayah, end_surah, end_ayah, created_at, updated_at)
               VALUES (?, (SELECT id FROM users WHERE email = 'readera@example.com'),
                       NULL, ?, NULL, ?, ?, ?, ?, ?, ?)"#,
            [
                id.into(),
                session_id.into(),
                surah.into(),
                ayah.into(),
                surah.into(),
                ayah.into(),
                T1.into(),
                T1.into(),
            ],
        ))
        .await
        .expect("raw bookmark seeded");
    }
}

async fn bookmark_row(db: &sea_orm::DatabaseConnection, id: &str) -> (Option<String>, Option<String>) {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT session_id, deleted_at FROM bookmarks WHERE id = ?",
            [id.into()],
        ))
        .await
        .expect("bookmark row readable")
        .expect("bookmark row exists");
    (
        row.try_get("", "session_id").ok(),
        row.try_get("", "deleted_at").ok(),
    )
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
    let (app, cookie, uid) = logged_in_app().await;
    let (status, body) = post_sync(&app, &cookie, round(uid, vec![])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(0));
    assert_eq!(body["sessions"], json!([]));
    assert_eq!(body["positions"], json!([]));
    assert_eq!(body["progress"], json!([]));
}

#[tokio::test]
async fn expected_user_id_mismatch_is_rejected_before_reads() {
    let (app, cookie, uid) = logged_in_app().await;
    let (status, _) = post_sync(&app, &cookie, round(uid + 1, vec![])).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "account change must 403");
}

#[tokio::test]
async fn full_round_trip_applies_and_snapshots_all_three_tables() {
    let (app, cookie, uid) = logged_in_app().await;
    let (status, body) = post_sync(
        &app,
        &cookie,
        round(
            uid,
            vec![
                session_upsert_full(
                    SES_DAILY,
                    "Daily",
                    "daily",
                    json!("en.sahih"),
                    json!("2026-08-20T09:00:00.000000Z"),
                    T1,
                ),
                position_upsert(SES_DAILY, 2, 100, T1),
                progress_upsert(SES_DAILY, 2, 100, T1),
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(3));
    assert_eq!(body["skipped"], json!(0));
    let session = find(&body["sessions"], SES_DAILY);
    assert_eq!(session["name"], json!("Daily"));
    assert_eq!(session["kind"], json!("daily"));
    assert_eq!(session["translationId"], json!("en.sahih"));
    assert!(session["archivedAt"].is_string(), "archivedAt round-trips");
    let position = &body["positions"][0];
    // verseKey is DERIVED server-side from surah/ayah — never client-supplied.
    assert_eq!(position["verseKey"], json!("2:100"));
    assert_eq!(position["localPage"], json!(3));
    assert_eq!(position["ratio"], json!(0.5));
    assert_eq!(position["sourceId"], json!("en.sahih"));
    assert_eq!(body["progress"][0]["furthestAyah"], json!(100));
}

#[tokio::test]
async fn progress_never_regresses_ec05() {
    let (app, cookie, uid) = logged_in_app().await;
    post_sync(&app, &cookie, round(uid, vec![session_upsert(SES_DAILY, "Daily", T0)])).await;
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![progress_upsert(SES_DAILY, 2, 100, T1)]),
    )
    .await;
    assert_eq!(body["applied"], json!(1));
    // Stale device reports 2:50 with a NEWER clock: MAX holds 100.
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![progress_upsert(SES_DAILY, 2, 50, T2)]),
    )
    .await;
    assert_eq!(body["applied"], json!(1), "newer-timestamp push still applies");
    assert_eq!(body["progress"][0]["furthestAyah"], json!(100), "EC-05 no regress");
    // A genuinely further verse advances.
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![progress_upsert(SES_DAILY, 2, 150, T3)]),
    )
    .await;
    assert_eq!(body["progress"][0]["furthestAyah"], json!(150));
}

#[tokio::test]
async fn same_session_lww_converges_and_stale_replays_skip_ec21() {
    let (app, cookie, uid) = logged_in_app().await;
    post_sync(&app, &cookie, round(uid, vec![session_upsert(SES_RENAMED, "Old", T1)])).await;
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![session_upsert(SES_RENAMED, "New", T2)]),
    )
    .await;
    assert_eq!(body["applied"], json!(1));
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(1), "no duplication");
    assert_eq!(find(&body["sessions"], SES_RENAMED)["name"], json!("New"));
    // Older replay loses LWW — the row is untouched, not clobbered.
    let (status, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![session_upsert(SES_RENAMED, "Stale", T0)]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(1));
    assert_eq!(find(&body["sessions"], SES_RENAMED)["name"], json!("New"));
}

#[tokio::test]
async fn crafted_foreign_session_id_is_isolated_ec52() {
    let (app, cookie_a, uid_a, cookie_b, uid_b) = two_user_app().await;
    // A owns session SES_DAILY.
    let (_, body) = post_sync(
        &app,
        &cookie_a,
        round(uid_a, vec![session_upsert(SES_DAILY, "A daily", T1)]),
    )
    .await;
    assert_eq!(body["applied"], json!(1));

    // B pushes position/progress naming A's session id: no live session with
    // that id exists FOR B, so both are skipped and B's snapshot stays empty.
    let (_, body) = post_sync(
        &app,
        &cookie_b,
        round(
            uid_b,
            vec![
                position_upsert(SES_DAILY, 2, 100, T2),
                progress_upsert(SES_DAILY, 2, 100, T2),
            ],
        ),
    )
    .await;
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(2));
    assert_eq!(body["positions"], json!([]));
    assert_eq!(body["progress"], json!([]));

    // B's session.upsert with the SAME id creates B's OWN (B, id) row — the
    // composite PK means it never touches A's.
    let (_, body) = post_sync(
        &app,
        &cookie_b,
        round(uid_b, vec![session_upsert(SES_DAILY, "B daily", T2)]),
    )
    .await;
    assert_eq!(body["applied"], json!(1));
    assert_eq!(find(&body["sessions"], SES_DAILY)["name"], json!("B daily"));

    // A's row is untouched by any of B's pushes.
    let (_, body) = post_sync(&app, &cookie_a, round(uid_a, vec![])).await;
    assert_eq!(find(&body["sessions"], SES_DAILY)["name"], json!("A daily"));
}

#[tokio::test]
async fn session_delete_drops_positions_progress_and_rehomes_ec53() {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state, "readera").await;
    let cookie = login_cookie(&state, &u).await;
    let app = router_with_state(state.clone());

    let (_, body) = post_sync(
        &app,
        &cookie,
        round(
            u.id,
            vec![
                session_upsert(SES_STUDY, "Study", T0),
                position_upsert(SES_STUDY, 2, 100, T1),
                progress_upsert(SES_STUDY, 2, 100, T1),
            ],
        ),
    )
    .await;
    assert_eq!(body["applied"], json!(3));

    // Live rows on the scoped key: a global 2:255 (the future collision), a
    // scoped 2:255 (collides once re-homed), and a scoped 1:1 (re-homes
    // cleanly). Seeded raw because bookmark_v1 is not mounted here.
    raw_insert_bookmark(
        &state.sea_db,
        &[
            ("99999999-9999-4999-8999-999999999999", "", 2, 255),
            ("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", SES_STUDY, 2, 255),
            ("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", SES_STUDY, 1, 1),
        ],
    )
    .await;

    let (status, body) = post_sync(
        &app,
        &cookie,
        round(u.id, vec![session_delete(SES_STUDY, T2)]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    assert_eq!(body["sessions"], json!([]), "tombstoned session leaves the snapshot");
    assert_eq!(body["positions"], json!([]), "positions dropped with the session");
    assert_eq!(body["progress"], json!([]), "progress dropped with the session");

    let (g_session, g_deleted) = bookmark_row(&state.sea_db, "99999999-9999-4999-8999-999999999999").await;
    assert_eq!(g_session.as_deref(), Some(""));
    assert!(g_deleted.is_none(), "the global row stays live");
    let (s1_session, s1_deleted) = bookmark_row(&state.sea_db, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").await;
    assert!(s1_deleted.is_some(), "colliding scoped row became a tombstone");
    assert_eq!(s1_session.as_deref(), Some(SES_STUDY), "tombstoned row keeps its scope");
    let (s2_session, s2_deleted) = bookmark_row(&state.sea_db, "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb").await;
    assert_eq!(s2_session.as_deref(), Some(""), "non-colliding scoped row re-homed to global");
    assert!(s2_deleted.is_none(), "re-homed row stays live");

    // A stale position push for the dead session is refused (live-session
    // guard), never resurrected.
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(u.id, vec![position_upsert(SES_STUDY, 2, 101, T3)]),
    )
    .await;
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["positions"], json!([]));
}

#[tokio::test]
async fn session_quota_caps_at_50_live_sessions() {
    let (app, cookie, uid) = logged_in_app().await;
    let first_50: Vec<Value> = (0..50)
        .map(|i| session_upsert(&quota_id(i), "S", T1))
        .collect();
    let (_, body) = post_sync(&app, &cookie, round(uid, first_50)).await;
    assert_eq!(body["applied"], json!(50));
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(50));

    // A NEW id past the quota is skipped — never silently dropped, surfaced
    // via `skipped`.
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![session_upsert(&quota_id(99), "S", T1)]),
    )
    .await;
    assert_eq!(body["applied"], json!(0));
    assert_eq!(body["skipped"], json!(1));
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(50));

    // Updating an already-live id never grows the set — always allowed.
    let (_, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![session_upsert(&quota_id(0), "Renamed", T2)]),
    )
    .await;
    assert_eq!(body["applied"], json!(1));
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(50));
}

#[tokio::test]
async fn future_clock_clamped_to_server_now_plus_skew_ec34() {
    let (app, cookie, uid) = logged_in_app().await;
    let now = chrono::Utc::now();
    let ceiling = (now + chrono::Duration::seconds(61))
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
    let floor = (now - chrono::Duration::seconds(60))
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
    let day_ahead = (now + chrono::Duration::days(1))
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

    let (status, body) = post_sync(
        &app,
        &cookie,
        round(uid, vec![session_upsert(SES_DAILY, "Fast clock", &day_ahead)]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], json!(1));
    let stored = find(&body["sessions"], SES_DAILY)["updatedAt"]
        .as_str()
        .expect("updatedAt serialized");
    let parsed = chrono::DateTime::parse_from_rfc3339(stored).expect("updatedAt parses");
    let ceiling_ts = chrono::DateTime::parse_from_rfc3339(&ceiling).unwrap();
    let floor_ts = chrono::DateTime::parse_from_rfc3339(&floor).unwrap();
    assert!(
        parsed <= ceiling_ts,
        "a +1 day clock cannot dominate LWW: stored {stored} > {ceiling}"
    );
    assert!(parsed >= floor_ts, "clamp keeps the write, stored {stored} < {floor}");
}

#[tokio::test]
async fn validation_rejects_bad_rounds_whole() {
    let (app, cookie, uid) = logged_in_app().await;
    let long_name = "x".repeat(101);
    let cases: Vec<(String, Value)> = vec![
        (
            "missing name".into(),
            json!({ "kind": "session.upsert", "id": SES_DAILY, "updatedAt": T1 }),
        ),
        (
            "bad session kind".into(),
            json!({ "kind": "session.upsert", "id": SES_DAILY, "name": "S",
                    "sessionKind": "yolo", "updatedAt": T1 }),
        ),
        (
            "name over 100 chars".into(),
            json!({ "kind": "session.upsert", "id": SES_DAILY, "name": long_name,
                    "updatedAt": T1 }),
        ),
        (
            "ayah past its surah count (1:8)".into(),
            json!({ "kind": "position.upsert", "sessionId": SES_DAILY,
                    "surah": 1, "ayah": 8, "updatedAt": T1 }),
        ),
        (
            "furthestAyah flat out of range".into(),
            json!({ "kind": "progress.upsert", "sessionId": SES_DAILY,
                    "surah": 2, "furthestAyah": 300, "updatedAt": T1 }),
        ),
        (
            "ratio above 1".into(),
            json!({ "kind": "position.upsert", "sessionId": SES_DAILY,
                    "surah": 2, "ayah": 100, "ratio": 1.5, "updatedAt": T1 }),
        ),
        (
            "progress missing furthestAyah".into(),
            json!({ "kind": "progress.upsert", "sessionId": SES_DAILY,
                    "surah": 2, "updatedAt": T1 }),
        ),
        (
            "bad id shape".into(),
            json!({ "kind": "session.delete", "id": "not-a-uuid", "updatedAt": T1 }),
        ),
    ];
    for (label, mutation) in cases {
        let (status, _) = post_sync(&app, &cookie, round(uid, vec![mutation])).await;
        assert!(!status.is_success(), "{label} must reject the round, got {status}");
    }
    // Batch cap 200 (§9.2).
    let oversized: Vec<Value> = (0..201)
        .map(|i| session_upsert(&quota_id(i % 50), "S", T1))
        .collect();
    let (status, _) = post_sync(&app, &cookie, round(uid, oversized)).await;
    assert!(!status.is_success(), "batch over 200 must reject");
    // expectedUserId is REQUIRED (§9.2) — no legacy clients exist.
    let (status, _) = post_sync(&app, &cookie, json!({ "mutations": [] })).await;
    assert!(!status.is_success(), "missing expectedUserId must reject");
}
