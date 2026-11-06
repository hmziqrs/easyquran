//! A12: recovery requests answer one way for known and unknown addresses, reset
//! grants live in the session payload (they survive a restart), and an email
//! change parks until the code on the new mailbox is proven.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use migration::{Migrator, MigratorTrait};
use ruxlog::db::sea_models::{email_change_pending, forgot_password, user};
use ruxlog::error::ErrorCode;
use ruxlog::modules::forgot_password_v1::controller::{self as recovery, reset_grant};
use ruxlog::modules::user_v1::controller::{update_profile_with_auth, EmailChangeDelivery};
use ruxlog::modules::user_v1::validator::V1UpdateProfilePayload;
use ruxlog::services::auth::{AuthBackend, AuthSession};
use ruxlog::services::mail::{router::MailRouterLimits, MailRouter};
use ruxlog::services::{fresh_auth, session_store::SqliteSessionStore};
use ruxlog::utils::code_hash::hash_code;
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, Set};
use tower_sessions::Session;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const SECRET: &[u8] = b"recovery-test-secret-key";

async fn database() -> DatabaseConnection {
    // Development runtime: the code is logged, never sent, so no SMTP is needed.
    std::env::set_var("APP_ENV", "development");
    ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let db = Database::connect(options).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db
}

async fn account(db: &DatabaseConnection) -> user::Model {
    let now = chrono::Utc::now().fixed_offset();
    user::ActiveModel {
        name: Set("Recovery owner".into()),
        email: Set("owner@example.com".into()),
        password: Set(None),
        is_verified: Set(true),
        role: Set(user::UserRole::User),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

fn mailer(db: &DatabaseConnection) -> Arc<MailRouter> {
    Arc::new(MailRouter::new(
        HashMap::new(),
        String::new(),
        Arc::new(rux_request_gate::InMemoryStore::default()),
        db.clone(),
        MailRouterLimits::default(),
        false,
    ))
}

fn backend(db: &DatabaseConnection, store: Arc<SqliteSessionStore>) -> AuthBackend {
    AuthBackend::new(db, store, Arc::new(Mutex::new(HashSet::new())))
}

async fn authenticated(db: &DatabaseConnection, owner: &user::Model) -> AuthSession {
    let store = Arc::new(SqliteSessionStore::new(db.clone()).await);
    let mut auth =
        AuthSession::new(backend(db, store.clone()), Session::new(None, store, None)).await;
    auth.login(owner).await.unwrap();
    auth.session().save().await.unwrap();
    auth
}

fn profile_email(email: &str) -> V1UpdateProfilePayload {
    V1UpdateProfilePayload {
        name: None,
        email: Some(email.to_owned()),
        password: None,
    }
}

#[tokio::test]
async fn request_is_accepted_for_known_and_unknown_addresses_alike() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db).await;

    // Known address: answered, one row written for the account.
    recovery::request_code(&db, mailer(&db), SECRET, "owner@example.com")
        .await
        .expect("known address is accepted");
    let rows = forgot_password::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].user_id, owner.id);

    // Unknown address: the same answer, and no row or mail for it.
    recovery::request_code(&db, mailer(&db), SECRET, "nobody@example.com")
        .await
        .expect("unknown address is accepted the same way");
    assert_eq!(
        forgot_password::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .len(),
        1,
        "an unknown address must not write a row"
    );

    // Second request inside the delay window: still accepted, no new row.
    recovery::request_code(&db, mailer(&db), SECRET, "owner@example.com")
        .await
        .expect("delayed known address is accepted the same way");
    assert_eq!(
        forgot_password::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn reset_grant_survives_a_restart_because_it_lives_in_the_session() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db).await;

    let store = Arc::new(SqliteSessionStore::new(db.clone()).await);
    let auth = AuthSession::new(backend(&db, store.clone()), Session::new(None, store, None)).await;
    let token = reset_grant::issue(&auth, &owner).await.unwrap();
    auth.session().save().await.unwrap();
    let session_id = auth.session().id().expect("session persisted");
    drop(auth);

    // A new store and a new session object over the same database: nothing
    // from the first process is kept in memory.
    let restarted_store = Arc::new(SqliteSessionStore::new(db.clone()).await);
    let restarted = AuthSession::new(
        backend(&db, restarted_store.clone()),
        Session::new(Some(session_id), restarted_store, None),
    )
    .await;
    let grant = reset_grant::take(&restarted)
        .await
        .unwrap()
        .expect("grant survives the restart");
    assert_eq!(grant.user_id, owner.id);
    assert!(reset_grant::token_matches(&grant, &token));
    assert!(!reset_grant::token_matches(&grant, "not-the-token"));

    // Single use: a second take finds nothing.
    assert!(reset_grant::take(&restarted).await.unwrap().is_none());
}

#[tokio::test]
async fn email_change_parks_until_the_new_mailbox_proves_the_code() {
    let _guard = TEST_LOCK.lock().await;
    let db = database().await;
    let owner = account(&db).await;
    let auth = authenticated(&db, &owner).await;

    // Typo: the change parks, the account keeps its address and session.
    fresh_auth::issue(&auth, &owner).await.unwrap();
    let outcome = update_profile_with_auth(
        &db,
        &auth,
        profile_email("ownr@example.com"),
        EmailChangeDelivery {
            secret_key: SECRET,
            mailer: mailer(&db),
        },
    )
    .await
    .unwrap()
    .expect("user exists");
    assert!(outcome.email_change_pending);
    assert_eq!(outcome.user.email, "owner@example.com");
    let parked = email_change_pending::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .expect("change parked");
    assert_eq!(parked.new_email, "ownr@example.com");

    // Recovery: the owner submits the right address; the parked typo is replaced.
    fresh_auth::issue(&auth, &owner).await.unwrap();
    update_profile_with_auth(
        &db,
        &auth,
        profile_email("owner2@example.com"),
        EmailChangeDelivery {
            secret_key: SECRET,
            mailer: mailer(&db),
        },
    )
    .await
    .unwrap();
    let replaced = email_change_pending::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .expect("change parked");
    assert_eq!(replaced.new_email, "owner2@example.com");
    assert_eq!(
        email_change_pending::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .len(),
        1,
        "one pending change per user"
    );

    // A wrong code leaves the account on its old address.
    let known_code = hash_code(SECRET, "KNOWN123");
    email_change_pending::Entity::park(
        &db,
        owner.id,
        "owner2@example.com",
        None,
        known_code.clone(),
    )
    .await
    .unwrap();
    let wrong = user::Entity::confirm_email_change(&db, owner.id, &hash_code(SECRET, "WRONG123"))
        .await
        .unwrap_err();
    assert_eq!(wrong.code, ErrorCode::InvalidInput);
    let still_old = user::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still_old.email, "owner@example.com");
    assert!(still_old.is_verified);

    // The right code switches the address and marks it verified.
    let confirmed = user::Entity::confirm_email_change(&db, owner.id, &known_code)
        .await
        .unwrap();
    assert_eq!(confirmed.email, "owner2@example.com");
    assert!(confirmed.is_verified);
    assert_ne!(confirmed.session_auth_secret, owner.session_auth_secret);
    assert!(email_change_pending::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .is_none());

    // And the code is single use: the parked row is gone.
    assert!(
        user::Entity::confirm_email_change(&db, owner.id, &known_code)
            .await
            .is_err()
    );
}
