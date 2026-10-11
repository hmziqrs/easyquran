use migration::{Migrator, MigratorTrait};
use ruxlog::db::sea_models::{email_verification, forgot_password, user, user_oauth_identity};
use ruxlog::error::ErrorCode;
use ruxlog::services::oauth::{find_or_create_user_for_oauth, OAuthProvider};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, Set};

async fn database() -> DatabaseConnection {
    ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let db = Database::connect(options).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db
}

async fn account(db: &DatabaseConnection, verified: bool) -> user::Model {
    let now = chrono::Utc::now().fixed_offset();
    user::ActiveModel {
        name: Set("Account owner".into()),
        email: Set("owner@example.com".into()),
        password: Set(Some("existing-password-hash".into())),
        is_verified: Set(verified),
        role: Set(user::UserRole::User),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn matching_verified_provider_email_never_automatically_links_existing_account() {
    for verified in [false, true] {
        let db = database().await;
        let existing = account(&db, verified).await;
        let result = find_or_create_user_for_oauth(
            &db,
            OAuthProvider::Github,
            "new-provider-subject",
            Some(existing.email.clone()),
            "Provider user".into(),
            true,
        )
        .await;

        assert_eq!(result.unwrap_err().code, ErrorCode::OperationNotAllowed);
        assert!(user_oauth_identity::Entity::list_for_user(&db, existing.id)
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn explicitly_linked_identity_can_sign_in_after_provider_email_changes() {
    let db = database().await;
    let existing = account(&db, true).await;
    ruxlog::services::oauth::login::link_identity_to_user(
        &db,
        existing.id,
        OAuthProvider::Google,
        "linked-google-subject",
    )
    .await
    .unwrap();
    let signed_in = find_or_create_user_for_oauth(
        &db,
        OAuthProvider::Google,
        "linked-google-subject",
        Some("different@example.com".into()),
        "Changed profile".into(),
        false,
    )
    .await
    .unwrap();
    assert_eq!(signed_in.id, existing.id);
}

#[tokio::test]
async fn email_change_invalidates_codes_issued_to_previous_address() {
    let db = database().await;
    let existing = account(&db, true).await;
    email_verification::Entity::create(&db, existing.id, "verification-hash".into())
        .await
        .unwrap();
    forgot_password::Entity::create(
        &db,
        forgot_password::NewForgotPassword {
            user_id: existing.id,
            code_hash: "recovery-hash".into(),
        },
    )
    .await
    .unwrap();

    let updated = user::Entity::update(
        &db,
        existing.id,
        user::UpdateUser {
            name: None,
            email: Some("new-address@example.com".into()),
            updated_at: chrono::Utc::now().fixed_offset(),
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert!(!updated.is_verified);
    assert_ne!(updated.session_auth_secret, existing.session_auth_secret);
    assert!(email_verification::Entity::find_by_user_id_or_code(
        &db,
        Some(existing.id),
        Some("verification-hash".into()),
    )
    .await
    .is_err());
    assert!(
        forgot_password::Entity::find_query(&db, Some(existing.id), None, None)
            .await
            .is_err()
    );
}

async fn recovery_code(db: &DatabaseConnection, account: &user::Model) -> forgot_password::Model {
    forgot_password::Entity::create(
        db,
        forgot_password::NewForgotPassword {
            user_id: account.id,
            code_hash: "recovery-hash".into(),
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn concurrent_recovery_verification_consumes_code_once() {
    let db = database().await;
    let existing = account(&db, true).await;
    recovery_code(&db, &existing).await;
    let (first, second) = tokio::join!(
        forgot_password::Entity::verify_code(&db, &existing.email, "recovery-hash"),
        forgot_password::Entity::verify_code(&db, &existing.email, "recovery-hash"),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
}

#[tokio::test]
async fn stale_recovery_verification_cannot_consume_replacement_code() {
    let db = database().await;
    let existing = account(&db, true).await;
    let stale = recovery_code(&db, &existing).await;
    forgot_password::Entity::regenerate(&db, existing.id, "replacement-hash".into())
        .await
        .unwrap();
    assert_eq!(
        forgot_password::Entity::consume_code(&db, &stale)
            .await
            .unwrap(),
        0
    );
    forgot_password::Entity::verify_code(&db, &existing.email, "replacement-hash")
        .await
        .unwrap();
}

#[tokio::test]
async fn expired_recovery_code_cannot_issue_reset_grant() {
    let db = database().await;
    let existing = account(&db, true).await;
    let code = recovery_code(&db, &existing).await;
    let mut expired: forgot_password::ActiveModel = code.into();
    expired.updated_at = Set(chrono::Utc::now().fixed_offset()
        - forgot_password::Entity::EXPIRY_TIME
        - chrono::Duration::seconds(1));
    expired.update(&db).await.unwrap();
    assert!(
        forgot_password::Entity::verify_code(&db, &existing.email, "recovery-hash")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reset_grant_is_invalid_after_credential_rotation() {
    let db = database().await;
    let existing = account(&db, true).await;
    user::Entity::change_password(&db, existing.id, "replacement-password".into())
        .await
        .unwrap();
    let changed = user::Entity::find_by_id(existing.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(forgot_password::Entity::reset(
        &db,
        existing.id,
        "attacker-password".into(),
        &existing.session_auth_secret,
    )
    .await
    .is_err());
    let after = user::Entity::find_by_id(existing.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.password, changed.password);
}

#[tokio::test]
async fn email_verification_is_atomic_and_bound_to_current_address() {
    let db = database().await;
    let existing = account(&db, false).await;
    email_verification::Entity::create(&db, existing.id, "verification-hash".into())
        .await
        .unwrap();
    assert!(email_verification::Entity::verify_code(
        &db,
        existing.id,
        "other@example.com",
        "verification-hash",
    )
    .await
    .is_err());
    email_verification::Entity::verify_code(&db, existing.id, &existing.email, "verification-hash")
        .await
        .unwrap();
    assert!(email_verification::Entity::verify_code(
        &db,
        existing.id,
        &existing.email,
        "verification-hash",
    )
    .await
    .is_err());
    let verified = user::Entity::find_by_id(existing.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(verified.is_verified);
}

#[tokio::test]
async fn stale_resend_request_cannot_issue_code_to_previous_address() {
    let db = database().await;
    let existing = account(&db, false).await;
    assert!(email_verification::Entity::regenerate_for_email(
        &db,
        existing.id,
        "previous@example.com",
        "verification-hash".into(),
    )
    .await
    .is_err());
    assert!(
        email_verification::Entity::find_by_user_id_or_code(&db, Some(existing.id), None,)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn admin_password_update_invalidates_previous_session_binding() {
    let db = database().await;
    let existing = account(&db, true).await;
    user::Entity::admin_update(
        &db,
        "https://media.example.com",
        existing.id,
        user::AdminUpdateUser {
            name: None,
            email: None,
            avatar_id: None,
            password: Some("admin-replacement-password".into()),
            is_verified: None,
            role: None,
            updated_at: chrono::Utc::now().fixed_offset(),
        },
    )
    .await
    .unwrap();
    let updated = user::Entity::find_by_id(existing.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(updated.session_auth_secret, existing.session_auth_secret);
}

#[tokio::test]
async fn unlinking_google_clears_legacy_sign_in_binding() {
    let db = database().await;
    let existing = account(&db, true).await;
    let mut legacy: user::ActiveModel = existing.clone().into();
    legacy.google_id = Set(Some("legacy-google-subject".into()));
    legacy.update(&db).await.unwrap();
    ruxlog::services::oauth::login::link_identity_to_user(
        &db,
        existing.id,
        OAuthProvider::Google,
        "legacy-google-subject",
    )
    .await
    .unwrap();
    user_oauth_identity::Entity::delete_for_user_provider(&db, existing.id, "google")
        .await
        .unwrap();
    assert!(
        user::Entity::find_by_google_id(&db, "legacy-google-subject".into())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn legacy_google_identity_migrates_without_matching_mutable_email() {
    let db = database().await;
    let existing = account(&db, true).await;
    let mut legacy: user::ActiveModel = existing.clone().into();
    legacy.google_id = Set(Some("legacy-google-subject".into()));
    legacy.update(&db).await.unwrap();
    let signed_in = find_or_create_user_for_oauth(
        &db,
        OAuthProvider::Google,
        "legacy-google-subject",
        Some("changed-at-provider@example.com".into()),
        "Legacy owner".into(),
        true,
    )
    .await
    .unwrap();
    assert_eq!(signed_in.id, existing.id);
    assert_eq!(
        user_oauth_identity::Entity::find_by_provider(&db, "google", "legacy-google-subject",)
            .await
            .unwrap()
            .unwrap()
            .user_id,
        existing.id
    );
}

#[tokio::test]
async fn legacy_google_identity_cannot_link_to_another_account() {
    let db = database().await;
    let existing = account(&db, true).await;
    let mut legacy: user::ActiveModel = existing.clone().into();
    legacy.google_id = Set(Some("legacy-google-subject".into()));
    legacy.update(&db).await.unwrap();
    let now = chrono::Utc::now().fixed_offset();
    let other = user::ActiveModel {
        name: Set("Other owner".into()),
        email: Set("other@example.com".into()),
        password: Set(Some("other-password-hash".into())),
        is_verified: Set(true),
        role: Set(user::UserRole::User),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    assert_eq!(
        ruxlog::services::oauth::login::link_identity_to_user(
            &db,
            other.id,
            OAuthProvider::Google,
            "legacy-google-subject",
        )
        .await
        .unwrap(),
        ruxlog::services::oauth::login::LinkOutcome::AlreadyLinkedOtherUser
    );
    assert!(user_oauth_identity::Entity::list_for_user(&db, other.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn stale_recovery_request_cannot_issue_code_to_previous_address() {
    let db = database().await;
    let existing = account(&db, true).await;
    assert!(forgot_password::Entity::regenerate_for_email(
        &db,
        existing.id,
        "previous@example.com",
        "recovery-hash".into(),
    )
    .await
    .is_err());
    assert!(
        forgot_password::Entity::find_query(&db, Some(existing.id), None, None)
            .await
            .is_err()
    );
}
