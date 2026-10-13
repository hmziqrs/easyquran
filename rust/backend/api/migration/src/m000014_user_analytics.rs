use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Reading insights (docs/reading-model.md §4), app SQLite only — the Quran
// DBs are untouched. `reading_events` is the raw append-only stream, keyed by
// the client-minted event id so a retried batch is idempotent; raw rows are
// purged after 90 days by the ingest path. `reading_daily` (day × surah) and
// `reading_ayah_totals` (surah × ayah) are the retention-proof rollups the
// summary reads. `local_day` is client-supplied device-local YYYY-MM-DD: the
// server never derives day boundaries. CASCADE on user deletion purges it all.
const STMTS: &[&str] = &[
    "DROP TABLE IF EXISTS reading_events",
    "DROP TABLE IF EXISTS reading_daily",
    "DROP TABLE IF EXISTS reading_ayah_totals",
    "CREATE TABLE reading_events (
        user_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        session_id TEXT,
        surah INTEGER NOT NULL,
        ayah INTEGER NOT NULL,
        source_id TEXT,
        ms INTEGER NOT NULL,
        local_day TEXT NOT NULL,
        created_at TEXT NOT NULL,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX idx_reading_events_user_created ON reading_events (user_id, created_at)",
    "CREATE TABLE reading_daily (
        user_id INTEGER NOT NULL,
        local_day TEXT NOT NULL,
        surah INTEGER NOT NULL,
        ayahs_read INTEGER NOT NULL DEFAULT 0,
        ms INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (user_id, local_day, surah),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE TABLE reading_ayah_totals (
        user_id INTEGER NOT NULL,
        surah INTEGER NOT NULL,
        ayah INTEGER NOT NULL,
        reads INTEGER NOT NULL DEFAULT 0,
        ms INTEGER NOT NULL DEFAULT 0,
        last_read_at TEXT NOT NULL,
        PRIMARY KEY (user_id, surah, ayah),
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
            "DROP TABLE IF EXISTS reading_ayah_totals",
            "DROP TABLE IF EXISTS reading_daily",
            "DROP TABLE IF EXISTS reading_events",
        ] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}
