use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use std::{env, time::Duration};
use tracing::{error, info, instrument};

/// Default SQLite database location. `mode=rwc` creates the file on first
/// connect; the parent `data/` directory is ensured by [`ensure_data_dir`].
const DEFAULT_DB_URL: &str = "sqlite:./data/easyquran.db?mode=rwc";

/// Get the SQLite database URL.
///
/// Read from `DATABASE_URL` (e.g. `sqlite:./data/easyquran.db?mode=rwc`),
/// falling back to [`DEFAULT_DB_URL`]. Unlike the previous Postgres path there
/// is no TLS/`sslmode` to configure.
#[instrument]
fn get_db_url() -> Result<String, String> {
    Ok(env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB_URL.to_string()))
}

/// Extract the filesystem path from a `sqlite:` URL and create its parent
/// directory so the first connection (with `mode=rwc`) can create the file.
/// In-memory (`:memory:`) and empty paths are left untouched.
fn ensure_data_dir(db_url: &str) {
    let path = db_url
        .strip_prefix("sqlite:")
        .unwrap_or(db_url)
        .split('?')
        .next()
        .unwrap_or("");
    if path.is_empty() || path == ":memory:" {
        return;
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

fn connect_options(db_url: &str) -> ConnectOptions {
    let mut opt = ConnectOptions::new(db_url.to_string());
    // SQLite is a single-writer database; a large connection pool only increases
    // `SQLITE_BUSY` / `database is locked` contention. Keep the pool tiny.
    opt.max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        // Disable SQLx logging to prevent noisy output in TUI;
        // Axum can enable tracing separately.
        .sqlx_logging(false)
        .sqlx_logging_level(log::LevelFilter::Info);
    opt
}

/// Apply SQLite pragmas that match the previous Postgres semantics and improve
/// concurrency for the second writer (the session store) sharing this file:
/// WAL journaling, a 5s busy timeout, and foreign-key enforcement.
async fn configure_sqlite(conn: &DatabaseConnection) {
    for pragma in [
        "PRAGMA journal_mode=WAL",
        "PRAGMA busy_timeout=5000",
        "PRAGMA foreign_keys=ON",
    ] {
        let _ = conn
            .execute(Statement::from_string(DatabaseBackend::Sqlite, pragma))
            .await;
    }
}

/// Establishes a connection to the database using SeaORM
/// This function panics on errors - for non-panicking version use `try_connect()`
#[instrument]
pub async fn init_db(run_migrations: bool) -> DatabaseConnection {
    match try_connect(run_migrations).await {
        Ok(conn) => {
            info!("SeaORM database connection working");
            conn
        }
        Err(e) => {
            error!("Database initialization failed: {}", e);
            panic!("Database initialization failed: {}", e);
        }
    }
}

/// Non-panicking helper to test DB connectivity (and optionally migrations).
pub async fn try_connect(run_migrations: bool) -> Result<DatabaseConnection, String> {
    let db_url = get_db_url()?;
    ensure_data_dir(&db_url);
    let opt = connect_options(&db_url);

    let conn = Database::connect(opt)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    configure_sqlite(&conn).await;

    conn.ping()
        .await
        .map_err(|e| format!("Failed to ping database: {}", e))?;

    if run_migrations {
        info!("Starting database migrations");
        Migrator::up(&conn, None)
            .await
            .map_err(|e| format!("Failed to run migrations: {}", e))?;
        info!("Database migrations completed successfully");
    }

    info!("SeaORM database connection established");
    Ok(conn)
}

/// Backwards-compatible helper for the Axum server.
#[instrument]
pub async fn get_sea_connection() -> DatabaseConnection {
    init_db(true).await
}
