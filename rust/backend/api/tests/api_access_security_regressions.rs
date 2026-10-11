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
use ruxlog::db::sea_models::{category, comment_flag, media, post, post_comment};
use ruxlog::modules::{bookmark_v1, post_comment_v1, post_v1};
use ruxlog::quran::load_quran_store;
use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};

use migration::{Migrator, MigratorTrait};
use rux_auth::AuthSession as GenAuthSession;
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::services::auth::AuthBackend;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};

static DEV_ENV: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_dev_env() {
    DEV_ENV.get_or_init(|| {
        std::env::set_var("APP_ENV", "development");
        ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    });
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

fn router_with_state(state: AppState) -> axum::Router {
    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let session_layer = tower_sessions::SessionManagerLayer::new((*state.session_store).clone())
        .with_secure(false)
        .with_http_only(true)
        .with_name("ruxlog.sid")
        .with_private(cookie_key);
    axum::Router::new()
        .nest("/bookmark/v1", bookmark_v1::routes())
        .nest("/post/v1", post_v1::routes())
        .nest("/post_comment/v1", post_comment_v1::routes())
        .layer(axum::Extension(state.clone()))
        .layer(session_layer)
        .with_state(state)
}

async fn seed_user(state: &AppState, name: &str, role: UserRole) -> user::Model {
    let now = chrono::Utc::now().fixed_offset();
    user::ActiveModel {
        name: sea_orm::Set(name.into()),
        email: sea_orm::Set(format!("{name}@example.com")),
        password: sea_orm::Set(None),
        role: sea_orm::Set(role),
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

async fn post_json(
    app: &axum::Router,
    path: &str,
    cookie: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn seed_category(state: &AppState) -> category::Model {
    let now = chrono::Utc::now().fixed_offset();
    category::ActiveModel {
        name: Set("Security test".into()),
        slug: Set("security-test".into()),
        color: Set("#ffffff".into()),
        text_color: Set("#000000".into()),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&state.sea_db)
    .await
    .unwrap()
}

async fn seed_post(
    state: &AppState,
    author: &user::Model,
    category: &category::Model,
    status: post::PostStatus,
) -> post::Model {
    let now = chrono::Utc::now().fixed_offset();
    post::ActiveModel {
        title: Set("Security test post".into()),
        slug: Set("security-test-post".into()),
        content: Set(json!({"time": 0, "blocks": [{"type": "paragraph", "data": {"text": "Security test content"}}], "version": "2.30.7"})),
        author_id: Set(author.id),
        category_id: Set(category.id),
        status: Set(status),
        view_count: Set(0),
        likes_count: Set(0),
        tag_ids: Set(post::TagIds(vec![])),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }.insert(&state.sea_db).await.unwrap()
}

async fn seed_media(state: &AppState, uploader: Option<i32>) -> media::Model {
    let now = chrono::Utc::now().fixed_offset();
    media::ActiveModel {
        bucket: Set(Some("test".into())),
        object_key: Set(format!(
            "private-owner-{}.png",
            uploader.unwrap_or_default()
        )),
        mime_type: Set("image/png".into()),
        uploader_id: Set(uploader),
        size: Set(1),
        is_optimized: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&state.sea_db)
    .await
    .unwrap()
}

fn create_payload(category_id: i32, media_id: Option<i32>, slug: &str) -> Value {
    json!({
        "title": "Security test post", "slug": slug,
        "content": {"time": 0, "blocks": [{"type": "paragraph", "data": {"text": "Security test content"}}], "version": "2.30.7"},
        "category_id": category_id, "featured_image_id": media_id
    })
}

async fn fixture() -> AppState {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.unwrap();
    state
}

#[tokio::test]
async fn unpublishing_post_hides_comments_from_anonymous_and_other_readers() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let reader = seed_user(&state, "reader", UserRole::User).await;
    let category = seed_category(&state).await;
    let post = seed_post(&state, &author, &category, post::PostStatus::Published).await;
    post_comment::Entity::create(
        &state.sea_db,
        post_comment::NewComment {
            post_id: post.id,
            user_id: author.id,
            content: "Private draft discussion".into(),
            likes_count: None,
        },
    )
    .await
    .unwrap();
    let reader_cookie = login_cookie(&state, &reader).await;
    let app = router_with_state(state.clone());
    let path = format!("/post_comment/v1/{}", post.id);
    let (status, body) = post_json(&app, &path, None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["content"], "Private draft discussion");
    for status in [post::PostStatus::Draft, post::PostStatus::Archived] {
        let mut active: post::ActiveModel = post::Entity::find_by_id(post.id)
            .one(&state.sea_db)
            .await
            .unwrap()
            .unwrap()
            .into();
        active.status = Set(status);
        active.update(&state.sea_db).await.unwrap();
        for cookie in [None, Some(reader_cookie.as_str())] {
            let (status, body) = post_json(&app, &path, cookie, json!({})).await;
            assert_eq!(
                status,
                StatusCode::NOT_FOUND,
                "hidden comments leaked: {body}"
            );
            assert!(!body.to_string().contains("Private draft discussion"));
        }
    }
}

#[tokio::test]
async fn unrelated_reader_cannot_create_comments_on_hidden_posts() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let reader = seed_user(&state, "reader", UserRole::User).await;
    let category = seed_category(&state).await;
    let post = seed_post(&state, &author, &category, post::PostStatus::Draft).await;
    let cookie = login_cookie(&state, &reader).await;
    let app = router_with_state(state.clone());
    let (status, body) = post_json(
        &app,
        "/post_comment/v1/create",
        Some(&cookie),
        json!({"post_id": post.id, "content": "Unauthorized comment"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "hidden post accepted comment: {body}"
    );
    assert_eq!(
        post_comment::Entity::find()
            .count(&state.sea_db)
            .await
            .unwrap(),
        0
    );
    let mut published: post::ActiveModel = post.into();
    published.status = Set(post::PostStatus::Published);
    let published = published.update(&state.sea_db).await.unwrap();
    let (status, body) = post_json(
        &app,
        "/post_comment/v1/create",
        Some(&cookie),
        json!({"post_id": published.id, "content": "Public discussion"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn hidden_post_comments_remain_accessible_to_author_and_moderator() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let moderator = seed_user(&state, "moderator", UserRole::Moderator).await;
    let category = seed_category(&state).await;
    let post = seed_post(&state, &author, &category, post::PostStatus::Archived).await;
    let app = router_with_state(state.clone());
    for user in [&author, &moderator] {
        let cookie = login_cookie(&state, user).await;
        let (status, body) = post_json(
            &app,
            "/post_comment/v1/create",
            Some(&cookie),
            json!({"post_id": post.id, "content": "Allowed discussion"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let (status, body) = post_json(
            &app,
            &format!("/post_comment/v1/{}", post.id),
            Some(&cookie),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(!body.as_array().unwrap().is_empty());
    }
}

#[tokio::test]
async fn author_cannot_create_post_with_another_users_or_unowned_media() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let owner = seed_user(&state, "owner", UserRole::Author).await;
    let category = seed_category(&state).await;
    let foreign_media = seed_media(&state, Some(owner.id)).await;
    let unowned_media = seed_media(&state, None).await;
    let cookie = login_cookie(&state, &author).await;
    let app = router_with_state(state.clone());
    for (image, slug) in [
        (&foreign_media, "foreign-image"),
        (&unowned_media, "unowned-image"),
    ] {
        let (status, body) = post_json(
            &app,
            "/post/v1/create",
            Some(&cookie),
            create_payload(category.id, Some(image.id), slug),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "foreign media exposed: {body}"
        );
        assert!(!body.to_string().contains(&image.object_key));
    }
    assert_eq!(post::Entity::find().count(&state.sea_db).await.unwrap(), 0);
}

#[tokio::test]
async fn author_cannot_update_post_to_another_users_media() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let owner = seed_user(&state, "owner", UserRole::Author).await;
    let category = seed_category(&state).await;
    let post = seed_post(&state, &author, &category, post::PostStatus::Draft).await;
    let media = seed_media(&state, Some(owner.id)).await;
    let cookie = login_cookie(&state, &author).await;
    let app = router_with_state(state.clone());
    let (status, body) = post_json(
        &app,
        &format!("/post/v1/update/{}", post.id),
        Some(&cookie),
        json!({"featured_image_id": media.id, "title": "Unauthorized replacement"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "foreign media exposed: {body}"
    );
    assert!(!body.to_string().contains(&media.object_key));
    let stored = post::Entity::find_by_id(post.id)
        .one(&state.sea_db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.featured_image_id, None);
    assert_eq!(stored.title, post.title);
}

#[tokio::test]
async fn owned_media_and_moderator_media_assignment_remain_allowed() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let moderator = seed_user(&state, "moderator", UserRole::Moderator).await;
    let category = seed_category(&state).await;
    let media = seed_media(&state, Some(author.id)).await;
    let app = router_with_state(state.clone());
    for (user, slug) in [(&author, "owned-image"), (&moderator, "moderated-image")] {
        let cookie = login_cookie(&state, user).await;
        let (status, body) = post_json(
            &app,
            "/post/v1/create",
            Some(&cookie),
            create_payload(category.id, Some(media.id), slug),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["featured_image"]["id"], media.id);
        let (status, body) = post_json(
            &app,
            &format!("/post/v1/update/{}", body["id"].as_i64().unwrap()),
            Some(&cookie),
            json!({"featured_image_id": media.id}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
}

#[tokio::test]
async fn bookmark_sync_rejects_account_mismatch_before_mutation_or_snapshot() {
    let state = fixture().await;
    let previous = seed_user(&state, "previous", UserRole::User).await;
    let current = seed_user(&state, "current", UserRole::User).await;
    let cookie = login_cookie(&state, &current).await;
    let app = router_with_state(state.clone());
    let private_folder = "11111111-1111-4111-8111-111111111111";
    let queued_folder = "22222222-2222-4222-8222-222222222222";
    let (status, _) = post_json(&app, "/bookmark/v1/sync", Some(&cookie), json!({"mutations": [{"kind": "folder.upsert", "id": private_folder, "name": "Current account private folder", "updatedAt": "2026-10-11T00:00:00Z"}]})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post_json(&app, "/bookmark/v1/sync", Some(&cookie), json!({
        "expectedUserId": previous.id,
        "mutations": [{"kind": "folder.upsert", "id": queued_folder, "name": "Previous account queue", "updatedAt": "2026-10-11T00:00:00Z"}]
    })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "account mismatch accepted: {body}"
    );
    assert!(!body.to_string().contains("Current account private folder"));
    let (status, body) = post_json(
        &app,
        "/bookmark/v1/sync",
        Some(&cookie),
        json!({"expectedUserId": current.id}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["folders"].as_array().unwrap().len(), 1);
    assert_eq!(body["folders"][0]["id"], private_folder);
    let (status, body) = post_json(
        &app,
        "/bookmark/v1/sync",
        Some(&cookie),
        json!({"expectedUserId": previous.id}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "mismatched pull returned snapshot: {body}"
    );
    assert!(!body.to_string().contains("Current account private folder"));
}

#[tokio::test]
async fn unrelated_reader_cannot_flag_comments_on_hidden_posts() {
    let state = fixture().await;
    let author = seed_user(&state, "author", UserRole::Author).await;
    let reader = seed_user(&state, "reader", UserRole::User).await;
    let category = seed_category(&state).await;
    let post = seed_post(&state, &author, &category, post::PostStatus::Draft).await;
    let comment = post_comment::Entity::create(
        &state.sea_db,
        post_comment::NewComment {
            post_id: post.id,
            user_id: author.id,
            content: "Private draft discussion".into(),
            likes_count: None,
        },
    )
    .await
    .unwrap();
    let cookie = login_cookie(&state, &reader).await;
    let app = router_with_state(state.clone());
    let (status, body) = post_json(
        &app,
        &format!("/post_comment/v1/flag/{}", comment.id),
        Some(&cookie),
        json!({"reason": "Unauthorized flag"}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "hidden comment accepted flag: {body}"
    );
    assert_eq!(
        comment_flag::Entity::find()
            .count(&state.sea_db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        post_comment::Entity::find_by_id(comment.id)
            .one(&state.sea_db)
            .await
            .unwrap()
            .unwrap()
            .flags_count,
        0
    );
    let mut published: post::ActiveModel = post.into();
    published.status = Set(post::PostStatus::Published);
    published.update(&state.sea_db).await.unwrap();
    let (status, body) = post_json(
        &app,
        &format!("/post_comment/v1/flag/{}", comment.id),
        Some(&cookie),
        json!({"reason": "Published comment report"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["flags_count"], 1);
}
