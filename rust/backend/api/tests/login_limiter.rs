//! Failure-only login limiter (A8): successes never block, failures delay
//! credential checks without locking them, IP blocks do not extend while
//! blocked, and the admin clear lifts an identity's buckets at once.

use std::time::{Duration, Instant};

use migration::{Migrator, MigratorTrait};
use rux_request_gate::{AbuseLimiterConfig, GateDecision, InMemoryStore, RateLimitStore};
use ruxlog::db::sea_models::user;
use ruxlog::error::ErrorCode;
use ruxlog::modules::admin_bans_v1::controller::clear_identity_limits_for;
use ruxlog::services::abuse_limiter::{self, Attempt, FailureMode};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set};

// Same thresholds as the auth controller's ABUSE_LIMITER_CONFIG.
const CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 3,
    temp_block_range: 360,
    temp_block_duration: 3600,
    block_retry_limit: 5,
    block_range: 900,
    block_duration: 86400,
};

// Limiter enforcement is on in this binary: APP_ENV=production turns off the
// non-production bypass.
fn enforce() {
    std::env::set_var("APP_ENV", "production");
    // Production refuses to run without a verification-identity key (A10).
    std::env::set_var(
        "VERIFICATION_IDENTITY_KEY",
        "0123456789abcdef0123456789abcdef",
    );
}

async fn database() -> DatabaseConnection {
    enforce();
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
        name: Set("Limiter owner".into()),
        email: Set("owner@example.com".into()),
        password: Set(Some("existing-password-hash".into())),
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

async fn fail(store: &InMemoryStore, key: &str, mode: FailureMode) {
    abuse_limiter::settle(store, key, CFG, mode, Attempt::Rejected).await;
}

async fn peek(store: &InMemoryStore, key: &str, mode: FailureMode) -> GateDecision {
    store.abuse_peek(key, CFG, mode).await.unwrap()
}

#[tokio::test]
async fn three_successful_logins_never_block() {
    enforce();
    let store = InMemoryStore::default();
    let key = "login:owner@example.com";
    for _ in 0..3 {
        abuse_limiter::gate(&store, key, CFG, FailureMode::Credential)
            .await
            .unwrap();
        abuse_limiter::settle(
            &store,
            key,
            CFG,
            FailureMode::Credential,
            Attempt::Succeeded,
        )
        .await;
    }
    let started = Instant::now();
    abuse_limiter::gate(&store, key, CFG, FailureMode::Credential)
        .await
        .expect("successful logins must not block the next one");
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(
        peek(&store, key, FailureMode::Credential).await,
        GateDecision::Pass
    );
}

#[tokio::test]
async fn three_failures_delay_the_fourth_but_do_not_lock() {
    enforce();
    let store = InMemoryStore::default();
    let key = "login:owner@example.com";
    for _ in 0..3 {
        fail(&store, key, FailureMode::Credential).await;
    }
    assert_eq!(
        peek(&store, key, FailureMode::Credential).await,
        GateDecision::Delay { secs: 1 }
    );

    // The fourth attempt waits about a second and is admitted, never a 429.
    let started = Instant::now();
    abuse_limiter::gate(&store, key, CFG, FailureMode::Credential)
        .await
        .expect("credential delay must admit the attempt");
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(900), "waited {waited:?}");
    assert!(waited < Duration::from_secs(4), "waited {waited:?}");

    // A further failure doubles the delay; the bucket still admits.
    fail(&store, key, FailureMode::Credential).await;
    assert_eq!(
        peek(&store, key, FailureMode::Credential).await,
        GateDecision::Delay { secs: 2 }
    );
}

#[tokio::test]
async fn blocked_ip_retries_do_not_extend_the_block() {
    enforce();
    let store = InMemoryStore::default();
    let key = "forgot_password_verify:203.0.113.7";
    for _ in 0..3 {
        fail(&store, key, FailureMode::Ip).await;
    }
    let first = match peek(&store, key, FailureMode::Ip).await {
        GateDecision::Blocked { retry_after_secs } => retry_after_secs,
        other => panic!("expected block after 3 failures, got {other:?}"),
    };
    assert!(first > 0 && first <= 900, "retry {first} outside cap");

    for _ in 0..5 {
        let err = abuse_limiter::gate(&store, key, CFG, FailureMode::Ip)
            .await
            .expect_err("blocked identity must stay refused");
        assert_eq!(err.code, ErrorCode::TooManyAttempts);
    }
    let later = match peek(&store, key, FailureMode::Ip).await {
        GateDecision::Blocked { retry_after_secs } => retry_after_secs,
        other => panic!("block must still hold, got {other:?}"),
    };
    assert!(
        later <= first,
        "blocked retries extended the block: {first} -> {later}"
    );
}

#[tokio::test]
async fn admin_clear_lifts_identity_limits_at_once() {
    let db = database().await;
    let owner = account(&db).await;
    let store = InMemoryStore::default();

    let login = format!("login:{}", owner.email);
    let totp = format!("totp:{}", owner.id);
    let reauth = format!("reauth:{}", owner.id);
    for key in [&login, &totp, &reauth] {
        for _ in 0..3 {
            fail(&store, key, FailureMode::Credential).await;
        }
        assert!(matches!(
            peek(&store, key, FailureMode::Credential).await,
            GateDecision::Delay { .. }
        ));
    }

    let cleared = clear_identity_limits_for(&db, &store, owner.id)
        .await
        .expect("admin clear");
    assert_eq!(cleared.len(), 4);
    for key in [&login, &totp, &reauth] {
        assert_eq!(
            peek(&store, key, FailureMode::Credential).await,
            GateDecision::Pass
        );
    }
    assert!(ruxlog::services::rate_limit_store::load(&db)
        .await
        .iter()
        .all(|row| !cleared.contains(&row.key)));
}
