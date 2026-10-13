use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Reading sessions (docs/reading-model.md §2), app SQLite only — the Quran
// DBs are untouched. A session is one continuous reading thread: its cursor
// lives on the row itself (one row per thread, last-writer-wins). Open =
// `ended_at IS NULL` and touched within 30 days; ended and stale threads are
// the archive (never deleted, a user delete is the `deleted_at` tombstone).
// `user_id` leads the primary key: ids are client-minted, so a key without it
// would let one account write into another's row. Progress (per-surah
// furthest ayah) is account-level and merged with MAX, never LWW-clobbered.
const STMTS: &[&str] = &[
    "DROP TABLE IF EXISTS reading_session_progress",
    "DROP TABLE IF EXISTS reading_session_positions",
    "DROP TABLE IF EXISTS reading_sessions",
    "CREATE TABLE reading_sessions (
        user_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        origin TEXT NOT NULL,
        origin_ref TEXT,
        scope TEXT NOT NULL,
        bound_end TEXT,
        surah INTEGER NOT NULL,
        ayah INTEGER NOT NULL,
        local_page INTEGER,
        ratio REAL,
        source_id TEXT,
        device TEXT NOT NULL,
        started_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        ended_at TEXT,
        end_reason TEXT,
        deleted_at TEXT,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX idx_reading_sessions_user_updated ON reading_sessions (user_id, updated_at)",
    "CREATE TABLE IF NOT EXISTS reading_progress (
        user_id INTEGER NOT NULL,
        surah INTEGER NOT NULL,
        furthest_ayah INTEGER NOT NULL,
        updated_at TEXT NOT NULL,
        PRIMARY KEY (user_id, surah),
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
        for stmt in ["DROP TABLE IF EXISTS reading_progress", "DROP TABLE IF EXISTS reading_sessions"] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}
