use ruxlog::db::sea_models::user::{self, AdminCreateUser, UserRole};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseBackend, Statement};

const DEFAULT_EMAIL: &str = "laurie40@yahoo.com";
const DEFAULT_PASSWORD: &str = "laurie40@yahoo.com";

fn db_url() -> Option<String> {
    match std::env::var("DATABASE_URL") {
        Ok(url) if url.is_empty() => None,
        Ok(url) => Some(url),
        Err(_) => Some("sqlite:./data/easyquran.db?mode=rwc".to_string()),
    }
}

#[tokio::test]
async fn seed_test_user() {
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
