use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// First-party reading analytics (next-features Feature 6), app SQLite only —
// the Quran DBs are untouched. `local_day` is the client-supplied
// device-local YYYY-MM-DD (EC-36): the server never derives day boundaries.
// `reading_daily` is the retention-proof rollup; raw `reading_events` rows are
// purged after a bounded window (90 days) by the ingest path. CASCADE on user
// deletion purges the history when the account closes.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS reading_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL,
        surah INTEGER NOT NULL,
        ayah INTEGER NOT NULL,
        source_id TEXT,
        kind TEXT NOT NULL,
        seconds INTEGER,
        local_day TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX IF NOT EXISTS idx_reading_events_user_created
        ON reading_events (user_id, created_at)",
    "CREATE TABLE IF NOT EXISTS reading_daily (
        user_id INTEGER NOT NULL,
        local_day TEXT NOT NULL,
        surah INTEGER NOT NULL,
        ayahs_read INTEGER NOT NULL DEFAULT 0,
        seconds INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (user_id, local_day, surah),
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
            "DROP TABLE IF EXISTS reading_daily",
            "DROP INDEX IF EXISTS idx_reading_events_user_created",
            "DROP TABLE IF EXISTS reading_events",
        ] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}
