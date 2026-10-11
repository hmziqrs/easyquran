use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use migration::{Migrator, MigratorTrait};
use ruxlog::db::sea_models::{user, user_session};
use ruxlog::services::auth::{AuthBackend, AuthSession};
use ruxlog::services::oauth::login::finish_oauth_login_with_db;
use ruxlog::services::oauth::OAuthProvider;
use ruxlog::services::pending_login;
use ruxlog::services::session_store::SqliteSessionStore;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    Set,
};
use tower_sessions::Session;

async fn database() -> DatabaseConnection {
    std::env::set_var("APP_ENV", "development");
    ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let db = Database::connect(options).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db
}

fn totp_payload(
    token: &str,
    owner: &user::Model,
) -> ruxlog::modules::auth_v1::validator::V1LoginTotpPayload {
    let secret = owner.two_fa_secret_plain().unwrap().unwrap();
    let code = ruxlog::utils::twofa::generate_totp_code_now(
        &secret,
        ruxlog::utils::twofa::DEFAULT_TOTP_DIGITS,
    )
    .unwrap();
    ruxlog::modules::auth_v1::validator::V1LoginTotpPayload {
        totp_token: token.into(),
        code,
    }
}

#[tokio::test]
async fn oauth_only_owner_can_complete_local_factor_once_and_get_bound_session() {
    let db = database().await;
    let owner = account(&db, true).await;
    let mut auth = anonymous(&db).await;
    let before = auth.session().id();
    finish_oauth_login_with_db(&db, &mut auth, &owner, OAuthProvider::Google)
        .await
        .unwrap();
    let body = pending_login::response(&auth).await.unwrap().unwrap();
    let token = body["totp_token"].as_str().unwrap();
    let gate = rux_request_gate::InMemoryStore::default();
    let completed = ruxlog::modules::auth_v1::controller::complete_pending_totp_login(
        &db,
        &gate,
        &mut auth,
        totp_payload(token, &owner),
    )
    .await
    .unwrap();
    assert_eq!(completed.id, owner.id);
    assert_eq!(auth.user.as_ref().unwrap().id, owner.id);
    assert_ne!(auth.session().id(), before);
    assert_eq!(user_session::Entity::find().count(&db).await.unwrap(), 1);
    assert!(
        matches!(ruxlog::services::oauth::login::callback_login_outcome(&auth, owner.id).await.unwrap(), ruxlog::services::oauth::login::CallbackOutcome::Login(id) if id == owner.id)
    );
    assert!(
        ruxlog::modules::auth_v1::controller::complete_pending_totp_login(
            &db,
            &gate,
            &mut auth,
            totp_payload(token, &owner)
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn changed_credentials_or_factor_cannot_complete_pending_login_even_with_valid_current_code()
{
    for credential_change in [true, false] {
        let db = database().await;
        let owner = account(&db, true).await;
        let mut auth = anonymous(&db).await;
        let token = pending_login::issue(&auth, &owner).await.unwrap();
        let mut changed: user::ActiveModel = owner.into();
        if credential_change {
            changed.session_auth_secret = Set("new-credential-secret".into());
        } else {
            changed.two_fa_secret = Set(Some("KRUGS4ZANFZSAYJA".into()));
        }
        let changed = changed.update(&db).await.unwrap();
        let gate = rux_request_gate::InMemoryStore::default();
        let result = ruxlog::modules::auth_v1::controller::complete_pending_totp_login(
            &db,
            &gate,
            &mut auth,
            totp_payload(&token, &changed),
        )
        .await;
        assert_eq!(
            result.unwrap_err().code,
            ruxlog::error::ErrorCode::Unauthorized
        );
        assert!(auth.user.is_none());
        assert_eq!(user_session::Entity::find().count(&db).await.unwrap(), 0);
    }
}

#[tokio::test]
async fn setup_rejects_stale_unenrolled_snapshot_after_factor_becomes_enabled() {
    let db = database().await;
    let owner = account(&db, false).await;
    let mut enabled: user::ActiveModel = owner.clone().into();
    enabled.two_fa_enabled = Set(true);
    let enabled = enabled.update(&db).await.unwrap();
    assert!(
        ruxlog::modules::auth_v1::controller::prepare_twofa_setup(&db, &owner)
            .await
            .is_err()
    );
    let current = user::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(current.two_fa_enabled);
    assert_eq!(current.two_fa_secret, enabled.two_fa_secret);
}

async fn account(db: &DatabaseConnection, enrolled: bool) -> user::Model {
    let now = chrono::Utc::now().fixed_offset();
    user::ActiveModel {
        name: Set("OAuth-only owner".into()),
        email: Set("owner@example.com".into()),
        password: Set(None),
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

async fn anonymous(db: &DatabaseConnection) -> AuthSession {
    let store = Arc::new(SqliteSessionStore::new(db.clone()).await);
    let backend = AuthBackend::new(db, store.clone(), Arc::new(Mutex::new(HashSet::new())));
    let session = Session::new(None, store, None);
    session.insert("anonymous", true).await.unwrap();
    session.save().await.unwrap();
    AuthSession::new(backend, session).await
}

#[tokio::test]
async fn enrolled_oauth_only_account_never_receives_full_session_before_local_factor() {
    let db = database().await;
    let owner = account(&db, true).await;
    for provider in [
        OAuthProvider::Google,
        OAuthProvider::Apple,
        OAuthProvider::Facebook,
        OAuthProvider::Github,
        OAuthProvider::Discord,
    ] {
        let mut auth = anonymous(&db).await;
        finish_oauth_login_with_db(&db, &mut auth, &owner, provider)
            .await
            .unwrap();
        assert!(auth.user.is_none(), "{provider} must wait for local TOTP");
        assert_eq!(user_session::Entity::find().count(&db).await.unwrap(), 0);
        let body = pending_login::response(&auth).await.unwrap().unwrap();
        assert_eq!(body["status"], "totp_required");
        assert_eq!(body["totp_token"].as_str().unwrap().len(), 64);
        assert!(matches!(
            ruxlog::services::oauth::login::callback_login_outcome(&auth, owner.id)
                .await
                .unwrap(),
            ruxlog::services::oauth::login::CallbackOutcome::TotpRequired
        ));
    }
}

#[tokio::test]
async fn pending_grant_is_cookie_bound_single_use_and_wrong_cookie_cannot_consume_it() {
    let db = database().await;
    let owner = account(&db, true).await;
    let auth = anonymous(&db).await;
    let unrelated = anonymous(&db).await;
    let token = pending_login::issue(&auth, &owner).await.unwrap();
    assert!(pending_login::response(&unrelated).await.unwrap().is_none());
    assert!(pending_login::take(&unrelated, &token)
        .await
        .unwrap()
        .is_none());
    let (first, second) = tokio::join!(
        pending_login::take(&auth, &token),
        pending_login::take(&auth, &token)
    );
    assert_eq!(
        usize::from(first.unwrap().is_some()) + usize::from(second.unwrap().is_some()),
        1
    );
    assert!(pending_login::response(&auth).await.unwrap().is_none());
}

#[tokio::test]
async fn session_rotation_and_new_first_factor_invalidate_previous_pending_grants() {
    let db = database().await;
    let owner = account(&db, true).await;
    let auth = anonymous(&db).await;
    let old_token = pending_login::issue(&auth, &owner).await.unwrap();
    let new_token = pending_login::issue(&auth, &owner).await.unwrap();
    assert!(pending_login::take(&auth, &old_token)
        .await
        .unwrap()
        .is_none());
    auth.session().cycle_id().await.unwrap();
    auth.session().save().await.unwrap();
    assert!(pending_login::take(&auth, &new_token)
        .await
        .unwrap()
        .is_none());
    assert!(pending_login::response(&auth).await.unwrap().is_none());
}

#[tokio::test]
async fn changed_credentials_or_factor_invalidate_first_factor_proof() {
    for credential_change in [true, false] {
        let db = database().await;
        let owner = account(&db, true).await;
        let auth = anonymous(&db).await;
        let token = pending_login::issue(&auth, &owner).await.unwrap();
        let grant = pending_login::take(&auth, &token).await.unwrap().unwrap();
        let mut updated: user::ActiveModel = owner.into();
        if credential_change {
            updated.session_auth_secret = Set("rotated-credential-secret".into());
        } else {
            updated.two_fa_secret = Set(Some("KRUGS4ZANFZSAYJA".into()));
        }
        let updated = updated.update(&db).await.unwrap();
        assert!(!grant.matches_user(&updated));
        let token = pending_login::issue(&auth, &updated).await.unwrap();
        let mut disabled: user::ActiveModel = updated.into();
        disabled.two_fa_enabled = Set(false);
        disabled.update(&db).await.unwrap();
        assert!(pending_login::response(&auth).await.unwrap().is_none());
        assert!(pending_login::take(&auth, &token).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn unenrolled_oauth_still_creates_bound_authenticated_session() {
    let db = database().await;
    let owner = account(&db, false).await;
    let mut auth = anonymous(&db).await;
    let before = auth.session().id();
    finish_oauth_login_with_db(&db, &mut auth, &owner, OAuthProvider::Github)
        .await
        .unwrap();
    assert_eq!(auth.user.as_ref().unwrap().id, owner.id);
    assert_ne!(auth.session().id(), before);
    assert_eq!(user_session::Entity::find().count(&db).await.unwrap(), 1);
    assert!(pending_login::response(&auth).await.unwrap().is_none());
}

#[tokio::test]
async fn setup_never_disarms_an_enrolled_factor_or_replaces_its_secret() {
    let db = database().await;
    let owner = account(&db, true).await;
    let result = ruxlog::modules::auth_v1::controller::prepare_twofa_setup(&db, &owner).await;
    assert!(
        result.is_err(),
        "enrolled setup must require supported disable flow"
    );
    let current = user::Entity::find_by_id(owner.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(current.two_fa_enabled);
    assert_eq!(current.two_fa_secret, owner.two_fa_secret);
    assert_eq!(current.two_fa_backup_codes, owner.two_fa_backup_codes);
}
