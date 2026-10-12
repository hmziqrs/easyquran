use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Reading-session sync tables (docs/next-features.md §9.1), app SQLite only —
// the Quran DBs are untouched. `user_id` LEADS every primary key: session ids
// are client-minted, so a key without it would let one account write into
// another's row with a crafted id (EC-52). Sessions carry tombstones
// (`deleted_at`); positions and progress have no independent identity, so a
// session delete drops their rows in the same transaction (done by the delete
// action in sea_models, not this DDL). CASCADE on user deletion purges the
// history when the account closes.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS reading_sessions (
        id TEXT NOT NULL,
        user_id INTEGER NOT NULL,
        name TEXT NOT NULL,
        kind TEXT NOT NULL DEFAULT 'free',
        translation_id TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        archived_at TEXT,
        deleted_at TEXT,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    // One row per session = the continue-reading anchor. verse_key is the
    // wire-shaped "s:a" convenience; surah/ayah are the queryable truth.
    "CREATE TABLE IF NOT EXISTS reading_session_positions (
        user_id INTEGER NOT NULL,
        session_id TEXT NOT NULL,
        verse_key TEXT NOT NULL,
        surah INTEGER NOT NULL,
        ayah INTEGER NOT NULL,
        local_page INTEGER,
        ratio REAL,
        source_id TEXT,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (user_id, session_id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    // Per-surah furthest ayah; merged with MAX, never LWW-clobbered (§9.3).
    "CREATE TABLE IF NOT EXISTS reading_session_progress (
        user_id INTEGER NOT NULL,
        session_id TEXT NOT NULL,
        surah INTEGER NOT NULL,
        furthest_ayah INTEGER NOT NULL,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (user_id, session_id, surah),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        for stmt in STMTS {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        for stmt in [
            "DROP TABLE IF EXISTS reading_session_progress",
            "DROP TABLE IF EXISTS reading_session_positions",
            "DROP TABLE IF EXISTS reading_sessions",
        ] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}
