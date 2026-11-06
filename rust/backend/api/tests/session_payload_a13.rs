//! A13: challenge and grant state lives in the session payload. Covers restart survival,
//! per-session quota refusal, and legacy payloads reading as absent instead of a 500.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use migration::{Migrator, MigratorTrait};
use ruxlog::db::sea_models::user;
use ruxlog::error::ErrorCode;
use ruxlog::services::auth::{AuthBackend, AuthSession};
use ruxlog::services::session_store::SqliteSessionStore;
use ruxlog::services::{fresh_auth, pending_login};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set};
use tower_sessions::Session;

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
        email: Set("owner-a13@example.com".into()),
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

fn backend(db: &DatabaseConnection, store: Arc<SqliteSessionStore>) -> AuthBackend {
    AuthBackend::new(db, store, Arc::new(Mutex::new(HashSet::new())))
}

/// Anonymous session plus the store behind it, so a test can rebuild the same session later.
async fn anonymous(db: &DatabaseConnection) -> (AuthSession, Arc<SqliteSessionStore>) {
    let store = Arc::new(SqliteSessionStore::new(db.clone()).await);
    let auth = AuthSession::new(
        backend(db, store.clone()),
        Session::new(None, store.clone(), None),
    )
    .await;
    (auth, store)
}

async fn authenticated(db: &DatabaseConnection, owner: &user::Model) -> AuthSession {
    let (mut auth, _store) = anonymous(db).await;
    auth.login(owner).await.unwrap();
    auth.session().save().await.unwrap();
    auth
}

#[tokio::test]
async fn pending_login_survives_restart_and_stays_single_use() {
    let db = database().await;
    let owner = account(&db, true).await;
    let (auth, store) = anonymous(&db).await;
    let token = pending_login::issue(&auth, &owner).await.unwrap();
    auth.session().save().await.unwrap();

    // Post-restart view: a new AuthSession over the same store and session id.
    let restarted = AuthSession::new(
        backend(&db, store.clone()),
        Session::new(auth.session().id(), store, None),
    )
    .await;
    let grant = pending_login::take(&restarted, &token)
        .await
        .unwrap()
        .expect("pending login must survive a restart");
    assert_eq!(grant.user_id, owner.id);
    assert!(pending_login::take(&restarted, &token)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn legacy_pending_payload_reads_as_absent_not_error() {
    let db = database().await;
    let owner = account(&db, true).await;
    let (auth, _store) = anonymous(&db).await;
    // Payload written before the grant became a struct: a bare token string.
    auth.session()
        .insert("rux_auth_pending_totp", "legacy-token")
        .await
        .unwrap();
    auth.session().save().await.unwrap();

    assert!(pending_login::response(&auth).await.unwrap().is_none());
    assert!(pending_login::take(&auth, "legacy-token")
        .await
        .unwrap()
        .is_none());
    // A fresh issue still works over the legacy slot.
    let token = pending_login::issue(&auth, &owner).await.unwrap();
    assert!(pending_login::take(&auth, &token).await.unwrap().is_some());
}

#[tokio::test]
async fn legacy_fresh_auth_payload_fails_closed_not_500() {
    let db = database().await;
    let owner = account(&db, false).await;
    let auth = authenticated(&db, &owner).await;
    auth.session()
        .insert("rux_auth_fresh_grant", "legacy-token")
        .await
        .unwrap();
    auth.session().save().await.unwrap();

    assert!(!fresh_auth::available(&auth, &db).await.unwrap());
    assert!(fresh_auth::consume(&auth, &db).await.is_err());
}

#[tokio::test]
async fn link_grants_refuse_past_per_session_quota() {
    let db = database().await;
    let owner = account(&db, false).await;
    let auth = authenticated(&db, &owner).await;
    for i in 0..8 {
        fresh_auth::authorize_link(&auth, &format!("link-state-{i}"))
            .await
            .unwrap();
    }
    let refused = fresh_auth::authorize_link(&auth, "link-state-8")
        .await
        .unwrap_err();
    assert!(matches!(refused.code, ErrorCode::RateLimited));

    // Refusal evicts nothing: an earlier link grant still confirms.
    let confirmed = fresh_auth::consume_link(&auth, &db, "link-state-0")
        .await
        .unwrap();
    assert_eq!(confirmed.id, owner.id);
}
