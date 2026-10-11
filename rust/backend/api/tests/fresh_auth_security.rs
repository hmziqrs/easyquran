use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use migration::{Migrator, MigratorTrait};
use ruxlog::db::sea_models::user;
use ruxlog::error::ErrorCode;
use ruxlog::modules::auth_v1::controller::confirm_password_authentication;
use ruxlog::modules::auth_v1::validator::V1ReauthenticatePayload;
use ruxlog::services::auth::{AuthBackend, AuthSession};
use ruxlog::services::{fresh_auth, session_store::SqliteSessionStore};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, Set};
use tower_sessions::Session;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const PASSWORD: &str = "correct-horse-battery-staple";

async fn database() -> DatabaseConnection {
    std::env::set_var("APP_ENV", "development");
    ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let db = Database::connect(options).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db
}

async fn account(db: &DatabaseConnection, enrolled: bool) -> user::Model {
    let now = chrono::Utc::now().fixed_offset();
    let hash = ruxlog::services::password_work::run(|| password_auth::generate_hash(PASSWORD))
        .await
        .unwrap();
    user::ActiveModel {
        name: Set("Account owner".into()),
        email: Set("owner@example.com".into()),
        password: Set(Some(hash)),
        is_verified: Set(true),
        role: Set(user::UserRole::User),
        two_fa_enabled: Set(enrolled),
        two_fa_secret: Set(Some("JBSWY3DPEHPK3PXP".into())),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

async fn authenticated(db: &DatabaseConnection, owner: &user::Model) -> AuthSession {
    let store = Arc::new(SqliteSessionStore::new(db.clone()).await);
    let backend = AuthBackend::new(db, store.clone(), Arc::new(Mutex::new(HashSet::new())));
    let session = Session::new(None, store, None);
    let mut auth = AuthSession::new(backend, session).await;
    auth.login(owner).await.unwrap();
    auth.session().save().await.unwrap();
    auth
}

fn payload(code: Option<String>) -> V1ReauthenticatePayload {
    V1ReauthenticatePayload {
        password: PASSWORD.into(),
        code,
    }
}

fn current_code(owner: &user::Model) -> String {
    ruxlog::utils::twofa::generate_totp_code_now(&owner.two_fa_secret_plain().unwrap().unwrap(), 6)
        .unwrap()
}

#[tokio::test]
async fn idle_session_cannot_mint_fresh_auth_without_credentials() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let mut auth = authenticated(&db, &owner).await;
    auth.touch().await.unwrap();
    assert_eq!(
        fresh_auth::consume(&auth, &db).await.unwrap_err().code,
        ErrorCode::OperationNotAllowed
    );
    let mut wrong = payload(None);
    wrong.password = "wrong-password-with-valid-length".into();
    assert!(confirm_password_authentication(
        &db,
        &rux_request_gate::InMemoryStore::default(),
        &auth,
        wrong
    )
    .await
    .is_err());
    assert!(!fresh_auth::available(&auth, &db).await.unwrap());
}

#[tokio::test]
async fn password_reconfirmation_issues_single_use_cookie_bound_grant() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let auth = authenticated(&db, &owner).await;
    let other_cookie = authenticated(&db, &owner).await;
    confirm_password_authentication(
        &db,
        &rux_request_gate::InMemoryStore::default(),
        &auth,
        payload(None),
    )
    .await
    .unwrap();
    assert!(fresh_auth::consume(&other_cookie, &db).await.is_err());
    assert!(fresh_auth::available(&auth, &db).await.unwrap());
    let (first, second) = tokio::join!(
        fresh_auth::consume(&auth, &db),
        fresh_auth::consume(&auth, &db)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(!fresh_auth::available(&auth, &db).await.unwrap());
}

#[tokio::test]
async fn credential_factor_changes_and_cookie_rotation_invalidate_fresh_grant() {
    let _guard = TEST_LOCK.lock().await;
    for change in ["credential", "factor", "enabled", "password", "cookie"] {
        let db = database().await;
        let owner = account(&db, false).await;
        let auth = authenticated(&db, &owner).await;
        fresh_auth::issue(&auth, &owner).await.unwrap();
        if change == "cookie" {
            auth.session().cycle_id().await.unwrap();
            auth.session().save().await.unwrap();
        } else {
            let mut changed: user::ActiveModel = owner.into();
            match change {
                "credential" => changed.session_auth_secret = Set("rotated-credential".into()),
                "factor" => changed.two_fa_secret = Set(Some("KRUGS4ZANFZSAYJA".into())),
                "enabled" => changed.two_fa_enabled = Set(true),
                "password" => changed.password = Set(Some("new-password-hash".into())),
                _ => unreachable!(),
            }
            changed.update(&db).await.unwrap();
        }
        assert!(
            fresh_auth::consume(&auth, &db).await.is_err(),
            "{change} must invalidate proof"
        );
    }
}

#[tokio::test]
async fn enrolled_account_requires_both_password_and_fresh_local_factor() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, true).await;
    let auth = authenticated(&db, &owner).await;
    let gate = rux_request_gate::InMemoryStore::default();
    assert!(
        confirm_password_authentication(&db, &gate, &auth, payload(None))
            .await
            .is_err()
    );
    assert!(!fresh_auth::available(&auth, &db).await.unwrap());
    let code = current_code(&owner);
    confirm_password_authentication(&db, &gate, &auth, payload(Some(code.clone())))
        .await
        .unwrap();
    fresh_auth::consume(&auth, &db).await.unwrap();
    assert!(
        confirm_password_authentication(&db, &gate, &auth, payload(Some(code)))
            .await
            .is_err()
    );
    assert!(!fresh_auth::available(&auth, &db).await.unwrap());
}

#[tokio::test]
async fn backup_code_reconfirmation_is_consumed_atomically() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, true).await;
    let hashes = ruxlog::services::password_work::run(|| {
        ruxlog::utils::twofa::hash_backup_codes(&["backup-code-one".into()])
    })
    .await
    .unwrap();
    let mut changed: user::ActiveModel = owner.into();
    changed.two_fa_backup_codes = Set(Some(serde_json::json!(hashes)));
    let owner = changed.update(&db).await.unwrap();
    let first_cookie = authenticated(&db, &owner).await;
    let second_cookie = authenticated(&db, &owner).await;
    let gate = rux_request_gate::InMemoryStore::default();
    let (first, second) = tokio::join!(
        confirm_password_authentication(
            &db,
            &gate,
            &first_cookie,
            payload(Some("backup-code-one".into()))
        ),
        confirm_password_authentication(
            &db,
            &gate,
            &second_cookie,
            payload(Some("backup-code-one".into()))
        )
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let current = user::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.two_fa_backup_codes, Some(serde_json::json!([])));
}

#[tokio::test]
async fn oauth_only_account_gets_fresh_grant_after_complete_provider_authentication() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let mut changed: user::ActiveModel = owner.into();
    changed.password = Set(None);
    let owner = changed.update(&db).await.unwrap();
    let mut auth = authenticated(&db, &owner).await;
    ruxlog::services::oauth::login::finish_oauth_login_with_db(
        &db,
        &mut auth,
        &owner,
        ruxlog::services::oauth::OAuthProvider::Google,
    )
    .await
    .unwrap();
    assert!(fresh_auth::available(&auth, &db).await.unwrap());
    fresh_auth::consume(&auth, &db).await.unwrap();
}

#[tokio::test]
async fn factor_setup_continuation_is_cookie_bound_and_separate_from_security_grant() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let auth = authenticated(&db, &owner).await;
    let other = authenticated(&db, &owner).await;
    fresh_auth::issue(&auth, &owner).await.unwrap();
    assert!(fresh_auth::consume_setup(&auth, &db).await.is_err());
    fresh_auth::consume(&auth, &db).await.unwrap();
    fresh_auth::issue_setup(&auth, &owner).await.unwrap();
    assert!(fresh_auth::consume_setup(&other, &db).await.is_err());
    assert!(fresh_auth::setup_available(&auth, &db).await.unwrap());
    fresh_auth::consume_setup(&auth, &db).await.unwrap();
    assert!(fresh_auth::consume_setup(&auth, &db).await.is_err());
}

#[tokio::test]
async fn stale_completed_login_cannot_grant_fresh_auth_for_changed_credentials_or_factor() {
    let _guard = TEST_LOCK.lock().await;
    for change in ["credential", "factor", "enabled", "password"] {
        let db = database().await;
        let owner = account(&db, false).await;
        let auth = authenticated(&db, &owner).await;
        let mut changed: user::ActiveModel = owner.into();
        match change {
            "credential" => changed.session_auth_secret = Set("rotated-secret".into()),
            "factor" => changed.two_fa_secret = Set(Some("KRUGS4ZANFZSAYJA".into())),
            "enabled" => changed.two_fa_enabled = Set(true),
            "password" => changed.password = Set(Some("different-hash".into())),
            _ => unreachable!(),
        }
        let current = changed.update(&db).await.unwrap();
        assert!(
            fresh_auth::issue(&auth, &current).await.is_err(),
            "{change} must not be waved through by reloading DB"
        );
    }
}

#[tokio::test]
async fn delayed_oauth_link_authorization_rejects_changed_factor_and_replayed_state() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let auth = authenticated(&db, &owner).await;
    let unrelated = authenticated(&db, &owner).await;
    fresh_auth::authorize_link(&auth, "first-state")
        .await
        .unwrap();
    assert!(fresh_auth::consume_link(&unrelated, &db, "first-state")
        .await
        .is_err());
    let confirmed = fresh_auth::consume_link(&auth, &db, "first-state")
        .await
        .unwrap();
    assert!(fresh_auth::consume_link(&auth, &db, "first-state")
        .await
        .is_err());
    fresh_auth::authorize_link(&auth, "changed-state")
        .await
        .unwrap();
    let mut changed: user::ActiveModel = owner.into();
    changed.two_fa_enabled = Set(true);
    changed.update(&db).await.unwrap();
    assert!(
        ruxlog::services::oauth::login::link_identity_to_unchanged_user(
            &db,
            &confirmed,
            ruxlog::services::oauth::OAuthProvider::Github,
            "late-provider-identity",
        )
        .await
        .is_err()
    );
    assert!(
        ruxlog::db::sea_models::user_oauth_identity::Entity::find_by_provider(
            &db,
            "github",
            "late-provider-identity",
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(fresh_auth::consume_link(&auth, &db, "changed-state")
        .await
        .is_err());
}

#[tokio::test]
async fn stale_credentials_cannot_start_factor_setup() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let mut changed: user::ActiveModel = owner.clone().into();
    changed.session_auth_secret = Set("rotated-credentials".into());
    changed.update(&db).await.unwrap();
    assert!(
        ruxlog::modules::auth_v1::controller::prepare_twofa_setup(&db, &owner)
            .await
            .is_err()
    );
    let current = user::Entity::find_by_id_with_404(&db, owner.id)
        .await
        .unwrap();
    assert_eq!(current.two_fa_secret, owner.two_fa_secret);
    assert_eq!(current.two_fa_backup_codes, owner.two_fa_backup_codes);
    assert!(!current.two_fa_enabled);
}

#[tokio::test]
async fn http_email_change_requires_fresh_proof_but_name_edit_keeps_existing_session() {
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{Request, StatusCode};
    use axum::response::{IntoResponse, Response};
    use axum::{Json, Router};
    use tower::ServiceExt;

    async fn update(
        State(backend): State<AuthBackend>,
        auth: AuthSession,
        payload: ruxlog::extractors::ValidatedJson<
            ruxlog::modules::user_v1::validator::V1UpdateProfilePayload,
        >,
    ) -> Response {
        let result = ruxlog::modules::user_v1::controller::update_profile_with_auth(
            &backend.pool,
            &auth,
            payload.0,
        )
        .await;
        match result {
            Ok(model) => Json(model).into_response(),
            Err(error) => error.into_response(),
        }
    }

    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db, false).await;
    let auth = authenticated(&db, &owner).await;
    let key = tower_sessions::cookie::Key::derive_from(
        b"fresh-auth-http-test-cookie-key-at-least-32-bytes",
    );
    let mut jar = tower_sessions::cookie::CookieJar::new();
    jar.private_mut(&key)
        .add(tower_sessions::cookie::Cookie::new(
            "ruxlog.sid",
            auth.session().id().unwrap().to_string(),
        ));
    let cookie = jar.get("ruxlog.sid").unwrap().to_string();
    let layer = tower_sessions::SessionManagerLayer::new((*auth.backend().session_store).clone())
        .with_secure(false)
        .with_name("ruxlog.sid")
        .with_private(key);
    let app = Router::new()
        .route("/user/v1/update", axum::routing::post(update))
        .layer(layer)
        .with_state(auth.backend().clone());
    let request = |body: serde_json::Value| {
        Request::builder()
            .method("POST")
            .uri("/user/v1/update")
            .header("cookie", &cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let denied = app
        .clone()
        .oneshot(request(serde_json::json!({"email":"attacker@example.com"})))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let renamed = app
        .clone()
        .oneshot(request(serde_json::json!({"name":"New owner name"})))
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(
        user::Entity::find_by_id(owner.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .email,
        owner.email
    );
    confirm_password_authentication(
        &db,
        &rux_request_gate::InMemoryStore::default(),
        &auth,
        payload(None),
    )
    .await
    .unwrap();
    let changed = app
        .oneshot(request(
            serde_json::json!({"email":"new-owner@example.com"}),
        ))
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);
    let updated = user::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.email, "new-owner@example.com");
    assert_ne!(updated.session_auth_secret, owner.session_auth_secret);
}
