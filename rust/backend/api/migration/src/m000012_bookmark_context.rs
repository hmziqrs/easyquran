use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Folder-based bookmarks (docs/reading-model.md §3), app SQLite only — the
// Quran DBs are untouched. Pre-launch rebuild: the m000007 tables are dropped
// and recreated (no data carried over, owner decision 2026-10-14).
//
// Every bookmark lives in a folder (`folder_id NOT NULL`). Each user has one
// default folder whose id the server derives (UUIDv5 of the user id) and
// ensures on every sync round; `is_default` is server-owned. A verse may sit
// in many folders: the live key is (user, folder, verse range), a partial
// unique index so a tombstone keeps protecting its key while a newer live row
// may take it. `user_id` leads both primary keys — ids are client-minted, so
// a key without it would let one account write into another's row.
const STMTS: &[&str] = &[
    "DROP TABLE IF EXISTS bookmarks",
    "DROP TABLE IF EXISTS bookmark_folders",
    "CREATE TABLE bookmark_folders (
        user_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        name TEXT NOT NULL,
        is_default INTEGER NOT NULL DEFAULT 0,
        position INTEGER,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE TABLE bookmarks (
        user_id INTEGER NOT NULL,
        id TEXT NOT NULL,
        folder_id TEXT NOT NULL,
        source_id TEXT,
        surah INTEGER NOT NULL,
        ayah INTEGER NOT NULL,
        end_surah INTEGER NOT NULL,
        end_ayah INTEGER NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT,
        PRIMARY KEY (user_id, id),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE,
        FOREIGN KEY (user_id, folder_id) REFERENCES bookmark_folders (user_id, id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE UNIQUE INDEX idx_bookmarks_live_key \
     ON bookmarks (user_id, folder_id, surah, ayah, end_surah, end_ayah) \
     WHERE deleted_at IS NULL",
    "CREATE INDEX idx_bookmarks_user_updated ON bookmarks (user_id, updated_at)",
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
        for stmt in ["DROP TABLE IF EXISTS bookmarks", "DROP TABLE IF EXISTS bookmark_folders"] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}
