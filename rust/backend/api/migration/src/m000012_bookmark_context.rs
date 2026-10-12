use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Phase-2 bookmark context (docs/next-features.md §9.1), app SQLite only —
// the Quran DBs are untouched.
//
// `bookmarks`: `session_id` ('' = global/legacy sentinel — never a bare NULL
// in the key, SQLite counts NULLs as distinct), `source_id` (context
// metadata, never a key part), `end_surah`/`end_ayah` (backfilled to
// surah/ayah; single ayah = end equals start), `deleted_at` (tombstones).
// `idx_bookmarks_user_verse` is replaced by a PARTIAL unique index on
// (user_id, session_id, surah, ayah, end_surah, end_ayah) WHERE deleted_at IS
// NULL — a tombstone keeps protecting its key while a newer live row may take
// it. No dedupe pass: m000008 already holds one row per verse and every
// legacy row lands on session '' with end = start.
//
// `bookmark_folders`: `kind` (name-only label — §11.7 settled no goal
// metadata), `position` (manual order), `deleted_at` (tombstone; the delete
// action still detaches children explicitly — SET NULL never fires on an
// UPDATE).
// Column adds, guarded per column in up() — an out-of-band-provisioned DB or
// a re-run degrades to a no-op instead of aborting on a duplicate-column
// error (SQLite has no ADD COLUMN IF NOT EXISTS).
const ADD_COLUMNS: &[(&str, &str, &str)] = &[
    ("bookmarks", "session_id", "ALTER TABLE bookmarks ADD COLUMN session_id TEXT NOT NULL DEFAULT ''"),
    ("bookmarks", "source_id", "ALTER TABLE bookmarks ADD COLUMN source_id TEXT"),
    ("bookmarks", "end_surah", "ALTER TABLE bookmarks ADD COLUMN end_surah INTEGER NOT NULL DEFAULT 0"),
    ("bookmarks", "end_ayah", "ALTER TABLE bookmarks ADD COLUMN end_ayah INTEGER NOT NULL DEFAULT 0"),
    ("bookmarks", "deleted_at", "ALTER TABLE bookmarks ADD COLUMN deleted_at TEXT"),
    ("bookmark_folders", "kind", "ALTER TABLE bookmark_folders ADD COLUMN kind TEXT NOT NULL DEFAULT 'generic'"),
    ("bookmark_folders", "position", "ALTER TABLE bookmark_folders ADD COLUMN position INTEGER"),
    ("bookmark_folders", "deleted_at", "ALTER TABLE bookmark_folders ADD COLUMN deleted_at TEXT"),
];

const BACKFILL_RANGE_ENDS: &str =
    "UPDATE bookmarks SET end_surah = surah, end_ayah = ayah WHERE end_surah = 0 OR end_ayah = 0";

const DROP_OLD_UNIQUE: &str = "DROP INDEX IF EXISTS idx_bookmarks_user_verse";

const CREATE_SCOPED_UNIQUE: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_user_verse \
     ON bookmarks (user_id, session_id, surah, ayah, end_surah, end_ayah) \
     WHERE deleted_at IS NULL";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        for (table, column, ddl) in ADD_COLUMNS {
            if !manager.has_column(table, column).await? {
                conn.execute_unprepared(ddl).await?;
            }
        }
        // Order matters: backfill the sentinel ends BEFORE the scoped unique
        // index lands, or every legacy row would collide on (…, 0, 0).
        for stmt in [BACKFILL_RANGE_ENDS, DROP_OLD_UNIQUE, CREATE_SCOPED_UNIQUE] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Column adds and the backfill are irreversible; only the index swap
        // is undone. Re-running up() restores it.
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_bookmarks_user_verse")
            .await?;
        Ok(())
    }
}

