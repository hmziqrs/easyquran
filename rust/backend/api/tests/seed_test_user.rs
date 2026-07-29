//! CI test-user seeder used by `.github/workflows/e2e.yml`.
//!
//! Inserts (idempotently) the admin account the smoke scripts log in with —
//! `laurie40@yahoo.com` / `laurie40@yahoo.com` (overridable via the `EMAIL`/
//! `PASSWORD` env vars, matching the smoke scripts' defaults) — plus one
//! category and two tags. The post/comment smoke scripts only call the
//! dev-only `/admin/seed/v1/seed_categories` / `seed_tags` routes when those
//! tables are empty; the e2e API server runs under `--features full` (which
//! excludes `seed-system`, so those routes are absent), so pre-seeding a
//! category + tags here lets the smoke scripts skip those calls and proceed.
//!
//! This is a `cargo test` target only because the integration-test harness
//! already links the full `ruxlog` crate, reusing the real `User::admin_create`
//! (password hashing, `session_auth_secret` backfill, encrypted-field handling)
//! and the exact `users`/`categories`/`tags` schema without a dedicated binary.
//!
//! Gating: it acts ONLY when `SEED_TEST_USER=1` is set, so the regular
//! `cargo test --features full` step in `backend-ci.yml` (no DB reachable) skips
//! it cleanly. The `seed-system` feature is intentionally NOT required — this
//! path goes through `User::admin_create`, never the `/admin/seed/v1` routes,
//! so `release-guard` (which greps the release `ruxlog` binary for seed
//! symbols) stays satisfied.

use ruxlog::db::sea_models::user::{self, AdminCreateUser, UserRole};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseBackend, Statement};

const DEFAULT_EMAIL: &str = "laurie40@yahoo.com";
const DEFAULT_PASSWORD: &str = "laurie40@yahoo.com";

/// Resolve the SQLite database URL — same source as `sea_connect::get_db_url`:
/// prefers `DATABASE_URL`, falling back to the default
/// `sqlite:./data/easyquran.db?mode=rwc` that `try_connect` uses (the `mode=rwc`
/// creates the file on first connect). Returns `None` only if `DATABASE_URL` is
/// set but empty, so a misconfigured environment fails loudly with a clear panic
/// rather than a confusing connection error.
fn db_url() -> Option<String> {
    match std::env::var("DATABASE_URL") {
        Ok(url) if url.is_empty() => None,
        Ok(url) => Some(url),
        Err(_) => Some("sqlite:./data/easyquran.db?mode=rwc".to_string()),
    }
}

#[tokio::test]
async fn seed_test_user() {
    // Only act under the explicit CI opt-in so the normal backend-ci test
    // suite (no DB available) is unaffected.
    let enabled = std::env::var("SEED_TEST_USER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        eprintln!("seed_test_user: SEED_TEST_USER not set; skipping");
        return;
    }

    let url = db_url().expect("SEED_TEST_USER=1 but DATABASE_URL is set to an empty string");
    let conn = Database::connect(&url)
        .await
        .expect("seed_test_user: failed to connect to database");

    let email = std::env::var("EMAIL").unwrap_or_else(|_| DEFAULT_EMAIL.to_string());
    let password = std::env::var("PASSWORD").unwrap_or_else(|_| DEFAULT_PASSWORD.to_string());

    // --- User (admin, verified) ------------------------------------------------
    // Idempotent: if the account already exists (re-run), refresh its password,
    // role, and verification flag so the smoke scripts always succeed.
    let existing = user::Entity::find_by_email(&conn, email.clone())
        .await
        .expect("seed_test_user: user lookup failed");
    if let Some(model) = existing {
        let mut am: user::ActiveModel = model.into();
        am.password = sea_orm::Set(Some(password_auth::generate_hash(&password)));
        am.role = sea_orm::Set(UserRole::SuperAdmin);
        am.is_verified = sea_orm::Set(true);
        am.update(&conn)
            .await
            .expect("seed_test_user: user update failed");
    } else {
        let new_user = AdminCreateUser {
            name: email.clone(),
            email: email.clone(),
            password: password.clone(),
            role: UserRole::SuperAdmin,
            avatar_id: None,
            is_verified: Some(true),
        };
        let _ = user::Entity::admin_create(&conn, "http://127.0.0.1:8888", new_user)
            .await
            .expect("seed_test_user: admin_create failed");
    }
    println!("seed_test_user: ensured admin user '{email}' exists");

    // --- Category + tags (so smoke scripts skip the seed-only routes) ----------
    // `users` is created via the entity layer above (correct hashing); the
    // reference rows below are plain idempotent inserts — `ON CONFLICT (slug)
    // DO NOTHING` makes re-runs a no-op.
    let now = chrono::Utc::now().fixed_offset();
    let cat_values: Vec<sea_orm::Value> = vec![
        "E2E Category".into(),
        "e2e-category".into(),
        "#0d1117".into(),
        "#ffffff".into(),
        true.into(),
        now.into(),
        now.into(),
    ];
    conn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO categories \
         (name, slug, color, text_color, is_active, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (slug) DO NOTHING",
        cat_values,
    ))
    .await
    .expect("seed_test_user: category insert failed");
    println!("seed_test_user: ensured baseline category exists");

    for (name, slug) in [
        ("E2E Tag One", "e2e-tag-one"),
        ("E2E Tag Two", "e2e-tag-two"),
    ] {
        let tag_values: Vec<sea_orm::Value> = vec![
            name.into(),
            slug.into(),
            "#1f6feb".into(),
            "#ffffff".into(),
            true.into(),
            now.into(),
            now.into(),
        ];
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tags \
             (name, slug, color, text_color, is_active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (slug) DO NOTHING",
            tag_values,
        ))
        .await
        .expect("seed_test_user: tag insert failed");
    }
    println!("seed_test_user: ensured baseline tags exist");

    println!("seed_test_user: DONE");
}
