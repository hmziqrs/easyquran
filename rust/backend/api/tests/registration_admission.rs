//! Registration admission (A9): creation caps shared by the password door and
//! the OAuth doors, and the first-24-hours limits for new accounts.

use chrono::{Duration as ChronoDuration, Utc};
use migration::{Migrator, MigratorTrait};
use rux_request_gate::{InMemoryStore, RateLimitStore};
use ruxlog::db::sea_models::{post_comment, user};
use ruxlog::error::ErrorCode;
use ruxlog::services::oauth::{find_or_create_user_for_oauth, OAuthProvider};
use ruxlog::services::registration_admission::{
    admit_creation, check_fresh_comment, creation_ip_key, CreationCaps, CreationSite,
};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Set,
};

const IP: &str = "203.0.113.42";

async fn database() -> DatabaseConnection {
    ruxlog::utils::field_crypto::set_key(&[0x6b; 32]).unwrap();
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let db = Database::connect(options).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    // Comment fixtures do not need a full post graph: FK checks are off for this test connection only.
    db.execute_unprepared("PRAGMA foreign_keys = OFF")
        .await
        .unwrap();
    db
}

async fn account(db: &DatabaseConnection, created_at: chrono::DateTime<Utc>) -> user::Model {
    let stamp = created_at.fixed_offset();
    user::ActiveModel {
        name: Set("Fresh reader".into()),
        email: Set(format!(
            "reader-{}@example.com",
            created_at.timestamp_nanos_opt().unwrap_or(0)
        )),
        password: Set(Some("existing-password-hash".into())),
        is_verified: Set(true),
        role: Set(user::UserRole::User),
        created_at: Set(stamp),
        updated_at: Set(stamp),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

async fn insert_comment(db: &DatabaseConnection, user_id: i32) {
    let now = Utc::now().fixed_offset();
    post_comment::ActiveModel {
        post_id: Set(1),
        user_id: Set(user_id),
        content: Set("an earlier comment".into()),
        likes_count: Set(0),
        hidden: Set(false),
        flags_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn oauth_creations_and_password_registrations_share_one_daily_ip_cap() {
    let db = database().await;
    let store = InMemoryStore::default();
    let cap = CreationCaps::configured().per_ip_daily;

    let mut created = 0u64;
    let mut refused_code = None;
    for n in 0..cap + 3 {
        let result = find_or_create_user_for_oauth(
            &db,
            OAuthProvider::Github,
            &format!("subject-{n}"),
            Some(format!("oauth-{n}@example.com")),
            "OAuth reader".into(),
            true,
            CreationSite {
                store: &store,
                ip: IP,
            },
        )
        .await;
        match result {
            Ok(_) => created += 1,
            Err(err) => refused_code = Some(err.code),
        }
    }
    assert_eq!(created, cap, "OAuth creations must stop at the per-IP cap");
    assert_eq!(refused_code, Some(ErrorCode::RateLimited));

    // The password door draws on the same counter for the same address.
    let password = admit_creation(&store, IP, CreationCaps::configured(), Utc::now()).await;
    assert_eq!(password.unwrap_err().code, ErrorCode::RateLimited);

    // A different address is not affected.
    admit_creation(
        &store,
        "198.51.100.7",
        CreationCaps::configured(),
        Utc::now(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn existing_identity_login_is_never_charged() {
    let db = database().await;
    let store = InMemoryStore::default();
    let site = CreationSite {
        store: &store,
        ip: IP,
    };
    let first = find_or_create_user_for_oauth(
        &db,
        OAuthProvider::Discord,
        "returning-subject",
        Some("returning@example.com".into()),
        "Returning".into(),
        true,
        site,
    )
    .await
    .unwrap();
    for _ in 0..5 {
        let again = find_or_create_user_for_oauth(
            &db,
            OAuthProvider::Discord,
            "returning-subject",
            Some("returning@example.com".into()),
            "Returning".into(),
            true,
            site,
        )
        .await
        .unwrap();
        assert_eq!(again.id, first.id);
    }
    // One creation so far. Probe the counter: the next increment reads 2.
    let window = std::time::Duration::from_secs(3600);
    let (count, _) = store
        .incr_expire(&creation_ip_key(IP), window)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn concurrent_burst_never_exceeds_the_daily_ceiling() {
    let store = std::sync::Arc::new(InMemoryStore::default());
    let caps = CreationCaps {
        per_ip_daily: 1_000,
        global_daily: 9,
    };
    let mut tasks = Vec::new();
    for n in 0..50 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            admit_creation(&store, &format!("192.0.2.{n}"), caps, Utc::now())
                .await
                .is_ok()
        }));
    }
    let mut admitted = 0;
    for task in tasks {
        if task.await.unwrap() {
            admitted += 1;
        }
    }
    assert_eq!(admitted, 9);
}

#[tokio::test]
async fn fresh_account_gets_one_comment_and_no_links() {
    let db = database().await;
    let now = Utc::now();
    let fresh = account(&db, now - ChronoDuration::hours(2)).await;

    // Links are refused on create and on update while the account is fresh.
    let link = check_fresh_comment(&db, &fresh, "see https://example.com", now, true)
        .await
        .unwrap_err();
    assert_eq!(link.code, ErrorCode::OperationNotAllowed);
    let link_update = check_fresh_comment(&db, &fresh, "www.example.org", now, false)
        .await
        .unwrap_err();
    assert_eq!(link_update.code, ErrorCode::OperationNotAllowed);

    // The first plain comment is allowed; a second is not.
    check_fresh_comment(&db, &fresh, "a plain thought", now, true)
        .await
        .unwrap();
    insert_comment(&db, fresh.id).await;
    let second = check_fresh_comment(&db, &fresh, "another thought", now, true)
        .await
        .unwrap_err();
    assert_eq!(second.code, ErrorCode::OperationNotAllowed);
}

#[tokio::test]
async fn established_account_is_not_limited() {
    let db = database().await;
    let now = Utc::now();
    let old = account(&db, now - ChronoDuration::hours(25)).await;
    insert_comment(&db, old.id).await;

    check_fresh_comment(&db, &old, "https://example.com is fine now", now, true)
        .await
        .unwrap();
    check_fresh_comment(
        &db,
        &old,
        "more text with a link www.example.com",
        now,
        false,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn account_turns_established_exactly_at_24_hours() {
    let db = database().await;
    let now = Utc::now();
    let edge = account(&db, now - ChronoDuration::seconds(86_400)).await;
    check_fresh_comment(&db, &edge, "https://example.com", now, false)
        .await
        .unwrap();
}

// --- Verification identity caps (A10) ---------------------------------------

async fn register(db: &DatabaseConnection, email: &str) -> Result<user::Model, ErrorCode> {
    user::Entity::create(
        db,
        user::NewUser {
            name: "Reader".into(),
            email: email.into(),
            password: "correct-horse-battery".into(),
            role: user::UserRole::User,
            is_minor: false,
            age_declared: false,
        },
        "code-hash-fixture".into(),
    )
    .await
    .map_err(|err| err.code)
}

#[tokio::test]
async fn gmail_dot_and_plus_variants_share_the_identity_caps() {
    use ruxlog::services::registration_admission::{IDENTITY_LIFETIME_CAP, IDENTITY_PENDING_CAP};
    assert_eq!((IDENTITY_PENDING_CAP, IDENTITY_LIFETIME_CAP), (3, 5));

    let db = database().await;
    let mut ids = Vec::new();
    for email in ["a.b@gmail.com", "ab@gmail.com", "a.b+1@gmail.com"] {
        ids.push(register(&db, email).await.unwrap().id);
    }

    // Three pending accounts on one mailbox: the fourth alias is refused.
    let err = register(&db, "a.b+2@gmail.com").await.unwrap_err();
    assert_eq!(err, ErrorCode::RateLimited);

    // A different mailbox is not affected.
    register(&db, "other@gmail.com").await.unwrap();

    // Verifying clears the pending count but not the lifetime count.
    for id in &ids {
        user::Entity::verify(&db, *id).await.unwrap();
    }
    register(&db, "ab+3@gmail.com").await.unwrap();
    register(&db, "a.b+4@gmail.com").await.unwrap();

    // Five accounts over the lifetime: the sixth is refused though none is pending.
    let err = register(&db, "ab+5@gmail.com").await.unwrap_err();
    assert_eq!(err, ErrorCode::RateLimited);
}

#[tokio::test]
async fn email_change_into_a_full_identity_is_refused() {
    let db = database().await;
    for email in ["a.b@gmail.com", "ab+1@gmail.com", "a.b+2@gmail.com"] {
        let created = register(&db, email).await.unwrap();
        user::Entity::verify(&db, created.id).await.unwrap();
    }
    register(&db, "ab+3@gmail.com").await.unwrap();
    register(&db, "a.b+4@gmail.com").await.unwrap();
    let mover = register(&db, "mover@gmail.com").await.unwrap();

    let err = user::Entity::update(
        &db,
        mover.id,
        user::UpdateUser {
            name: None,
            email: Some("a.b+6@gmail.com".into()),
            updated_at: Utc::now().fixed_offset(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::RateLimited);

    // The refused account keeps its old address.
    let still = user::Entity::find_by_id_with_404(&db, mover.id)
        .await
        .unwrap();
    assert_eq!(still.email, "mover@gmail.com");
}

#[tokio::test]
async fn identity_backfill_fills_only_null_rows_and_is_repeatable() {
    let db = database().await;
    let created = register(&db, "Legacy.User@gmail.com").await.unwrap();
    let expected = ruxlog::utils::email::verification_identity_hash("legacyuser@gmail.com");
    assert_eq!(created.verification_identity_hash, Some(expected.clone()));

    db.execute_unprepared("UPDATE users SET verification_identity_hash = NULL")
        .await
        .unwrap();
    assert_eq!(
        user::Entity::backfill_verification_identity(&db)
            .await
            .unwrap(),
        1
    );
    let row = user::Entity::find_by_id_with_404(&db, created.id)
        .await
        .unwrap();
    assert_eq!(row.verification_identity_hash, Some(expected));
    assert_eq!(
        user::Entity::backfill_verification_identity(&db)
            .await
            .unwrap(),
        0
    );
}
