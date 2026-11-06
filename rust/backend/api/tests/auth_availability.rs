//! Password-work lanes under load (A11).
//!
//! The lanes are process-wide, so every test here takes `SERIAL` first. Each
//! test drives the public `password_work` API the same way the auth handlers do.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use ruxlog::services::password_work::{self, PasswordWorkError, QUEUE_WAIT};

static SERIAL: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Simulated Argon2 cost for one verification.
const WORK: Duration = Duration::from_millis(20);

#[tokio::test(flavor = "multi_thread")]
async fn twenty_parallel_logins_do_not_deny_a_twenty_first() {
    let _serial = SERIAL.lock().await;
    let mut logins = Vec::new();
    for _ in 0..21 {
        logins.push(tokio::spawn(password_work::run(|| {
            std::thread::sleep(WORK)
        })));
    }
    for login in logins {
        login
            .await
            .expect("login task joins")
            .expect("a login queued behind the public lane must run, not be refused");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn saturated_public_lane_leaves_authenticated_and_dummy_lanes_free() {
    let _serial = SERIAL.lock().await;
    // Hold every default public slot well past the queue wait.
    let holders: Vec<_> = (0..2)
        .map(|_| {
            tokio::spawn(password_work::run(|| {
                std::thread::sleep(QUEUE_WAIT * 2);
            }))
        })
        .collect();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let started = Instant::now();
    assert_eq!(password_work::run_authenticated(|| 7).await.unwrap(), 7);
    assert_eq!(password_work::run_dummy(|| 8).await.unwrap(), 8);
    assert!(
        started.elapsed() < QUEUE_WAIT,
        "reserved and dummy lanes must not wait on the public lane"
    );

    // The public lane is still full: a new anonymous call waits out the queue
    // and is refused, and the saturation counter moves.
    let before = password_work::saturation_count();
    let refused = password_work::run(|| ()).await;
    assert!(matches!(refused, Err(PasswordWorkError::Busy)));
    assert!(password_work::saturation_count() > before);

    for holder in holders {
        holder
            .await
            .expect("holder joins")
            .expect("holder completes");
    }
}
