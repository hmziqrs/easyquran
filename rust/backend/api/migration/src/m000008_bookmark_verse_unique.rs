use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// One verse, one row: bookmark identity is (user_id, surah, ayah), but the
// client-minted UUIDv4 ids let two offline devices mint TWO uuids for the same
// verse — m000007 had no uniqueness on the verse, so both rows lived side by
// side and deleting the visible one "resurrected" the bookmark via the other.
// This migration deduplicates existing verse groups (keep MAX(updated_at),
// tie-break max(rowid)) and then enforces UNIQUE (user_id, surah, ayah).
//
// Legacy CURRENT_TIMESTAMP-shaped rows ('YYYY-MM-DD HH:MM:SS') sort wrong
// against the canonical LWW wire format ('...T...Z' — ' ' < 'T' lexically), so
// both tables are normalized to the exact lww_timestamp shape FIRST; only then
// does the dedupe's MAX(updated_at) compare by instant.
//
// Normalization target = chrono to_rfc3339_opts(SecondsFormat::Micros, true):
// 'YYYY-MM-DDTHH:MM:SS.ffffffZ'. %f renders 'SS.SSS'; suffixing '000' before
// substr fixes the fraction at 6 digits. Rows already carrying the canonical
// leading '____-__-__T' shape are left untouched — and so are corrupt rows:
// strftime() returns NULL for an unparseable value, and assigning NULL to the
// NOT NULL column would abort the migration mid-flight, so the guarded
// `IS NOT NULL` predicate leaves garbage timestamps exactly as they are. The
// dedupe's ORDER BY tolerates them (TEXT comparison stays deterministic;
// corrupt text just wins or loses lexically), and the runtime LWW statements
// treat a NULL-julianday row as oldest (see the bookmark actions) — corrupt
// rows stay killable instead of blocking the migration.

const NORMALIZE_BOOKMARKS: &str = "UPDATE bookmarks SET updated_at = \
     strftime('%Y-%m-%dT%H:%M:%S', updated_at) \
     || '.' || substr(strftime('%f', updated_at) || '000', 4, 6) || 'Z' \
     WHERE updated_at NOT LIKE '____-__-__T%' \
       AND strftime('%Y-%m-%dT%H:%M:%S', updated_at) IS NOT NULL";

const NORMALIZE_BOOKMARK_FOLDERS: &str = "UPDATE bookmark_folders SET updated_at = \
     strftime('%Y-%m-%dT%H:%M:%S', updated_at) \
     || '.' || substr(strftime('%f', updated_at) || '000', 4, 6) || 'Z' \
     WHERE updated_at NOT LIKE '____-__-__T%' \
       AND strftime('%Y-%m-%dT%H:%M:%S', updated_at) IS NOT NULL";

// Correlated keeper-subquery form (no window functions): for every row, its
// verse group's keeper is the MAX(updated_at) row, ties broken by max(rowid);
// every non-keeper is deleted. Runs on any SQLite version.
const DEDUPE_BOOKMARKS_BY_VERSE: &str = "DELETE FROM bookmarks WHERE rowid NOT IN (\
     SELECT b2.rowid FROM bookmarks b2 \
     WHERE b2.rowid = (\
         SELECT b3.rowid FROM bookmarks b3 \
         WHERE b3.user_id = b2.user_id AND b3.surah = b2.surah AND b3.ayah = b2.ayah \
         ORDER BY b3.updated_at DESC, b3.rowid DESC LIMIT 1))";

const CREATE_BOOKMARKS_USER_VERSE_UNIQUE: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_user_verse \
     ON bookmarks (user_id, surah, ayah)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        // Order matters: normalize -> dedupe -> unique index. The index can
        // only be created once every existing verse group holds one row, and
        // the dedupe only ranks correctly once timestamps share one format.
        for stmt in [
            NORMALIZE_BOOKMARKS,
            NORMALIZE_BOOKMARK_FOLDERS,
            DEDUPE_BOOKMARKS_BY_VERSE,
            CREATE_BOOKMARKS_USER_VERSE_UNIQUE,
        ] {
            conn.execute_unprepared(stmt).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The dedupe and normalization are irreversible data changes; only the
        // index is droppable. Re-running up() re-creates it (IF NOT EXISTS).
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_bookmarks_user_verse")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, Value};
    use sea_orm_migration::SchemaManager;

    async fn mem_db() -> sea_orm::DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        Database::connect(opt).await.unwrap()
    }

    // Minimal fixture mirroring the m000007 table shapes; m000008 only touches
    // the columns reproduced here.
    async fn create_fixture_tables(db: &sea_orm::DatabaseConnection) {
        db.execute_unprepared(
            "CREATE TABLE bookmark_folders (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL, \
             name TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE bookmarks (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL, \
             folder_id TEXT, surah INTEGER NOT NULL, ayah INTEGER NOT NULL, \
             created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
    }

    async fn text_scalar(db: &sea_orm::DatabaseConnection, sql: &str) -> String {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap();
        let val: Option<String> = row.try_get_by_index(0).unwrap();
        val.unwrap_or_default()
    }

    async fn count_scalar(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    async fn index_exists(db: &sea_orm::DatabaseConnection, table: &str, name: &str) -> bool {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!("PRAGMA index_list('{table}')"),
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter().any(|r| {
            let n: Option<String> = r.try_get_by_index(1).ok();
            n.as_deref() == Some(name)
        })
    }

    #[tokio::test]
    async fn normalizes_legacy_timestamps_in_both_tables() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        db.execute_unprepared(
            "INSERT INTO bookmark_folders (id, user_id, name, created_at, updated_at) \
             VALUES ('f1', 1, 'Tafsir', '2026-01-05 12:00:00', '2026-01-05 12:00:00')",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
             VALUES ('b1', 1, 1, 1, '2026-01-05 12:00:00', '2026-01-05 12:34:56')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        // Exact lww_timestamp shape: RFC3339 UTC, fixed-width 6-digit micros.
        assert_eq!(
            text_scalar(
                &db,
                "SELECT updated_at FROM bookmark_folders WHERE id = 'f1'"
            )
            .await,
            "2026-01-05T12:00:00.000000Z"
        );
        assert_eq!(
            text_scalar(&db, "SELECT updated_at FROM bookmarks WHERE id = 'b1'").await,
            "2026-01-05T12:34:56.000000Z"
        );
    }

    #[tokio::test]
    async fn canonical_rows_are_left_untouched() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
             VALUES ('b1', 1, 1, 1, '2026-01-05T12:00:00Z', '2026-01-05T12:00:00.123456Z')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            text_scalar(&db, "SELECT updated_at FROM bookmarks WHERE id = 'b1'").await,
            "2026-01-05T12:00:00.123456Z",
            "already-canonical rows must not be rewritten"
        );
    }

    #[tokio::test]
    async fn dedupes_verse_groups_keeping_newest_then_max_rowid() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        // Three rows for verse (1, 9, 9): newest updated_at wins.
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) VALUES \
             ('old-a', 1, 9, 9, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'), \
             ('new',   1, 9, 9, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z'), \
             ('old-b', 1, 9, 9, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'), \
             ('other', 1, 9, 8, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            text_scalar(&db, "SELECT id FROM bookmarks WHERE surah = 9 AND ayah = 9").await,
            "new",
            "MAX(updated_at) row survives the dedupe"
        );
        assert_eq!(
            count_scalar(&db, "SELECT COUNT(*) FROM bookmarks").await,
            2,
            "other verses keep their rows"
        );
    }

    #[tokio::test]
    async fn dedupe_ties_on_updated_at_keep_max_rowid() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        // Two rows, one verse, identical updated_at: the later-inserted row
        // (max rowid) is the deterministic keeper.
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) VALUES \
             ('tie-1', 2, 1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'), \
             ('tie-2', 2, 1, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            text_scalar(&db, "SELECT id FROM bookmarks WHERE user_id = 2").await,
            "tie-2",
            "equal updated_at keeps the max-rowid row"
        );
    }

    #[tokio::test]
    async fn unique_index_rejects_a_second_row_per_verse() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert!(index_exists(&db, "bookmarks", "idx_bookmarks_user_verse").await);

        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
             VALUES ('b1', 1, 2, 255, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();
        let dup = db
            .execute_unprepared(
                "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
                 VALUES ('b2', 1, 2, 255, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .await;
        assert!(
            dup.is_err(),
            "UNIQUE(user_id, surah, ayah) must reject a second row for one verse"
        );

        // Same verse under ANOTHER user stays legal.
        let other_user = db
            .execute_unprepared(
                "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
                 VALUES ('b3', 2, 2, 255, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .await;
        assert!(other_user.is_ok(), "uniqueness is per (user_id, verse)");
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
             VALUES ('b1', 1, 1, 1, '2026-01-05 12:00:00', '2026-01-05 12:00:00')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        let first = text_scalar(&db, "SELECT updated_at FROM bookmarks WHERE id = 'b1'").await;
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            text_scalar(&db, "SELECT updated_at FROM bookmarks WHERE id = 'b1'").await,
            first,
            "second run rewrites nothing (NOT LIKE guard + IF NOT EXISTS)"
        );
        assert_eq!(count_scalar(&db, "SELECT COUNT(*) FROM bookmarks").await, 1);
    }

    #[tokio::test]
    async fn garbage_timestamp_rows_survive_the_migration() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        // Unparseable updated_at: strftime() yields NULL, so an unguarded
        // normalization would try to write NULL into the NOT NULL column and
        // abort the whole migration. The guarded UPDATE must leave the row
        // untouched instead.
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, surah, ayah, created_at, updated_at) \
             VALUES ('corrupt', 1, 6, 6, 'not-a-date', 'not-a-date'), \
             ('valid', 1, 6, 6, '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            text_scalar(&db, "SELECT updated_at FROM bookmarks WHERE id = 'corrupt'").await,
            "not-a-date",
            "unparseable rows are left untouched, not aborted on"
        );
        assert_eq!(
            count_scalar(
                &db,
                "SELECT COUNT(*) FROM bookmarks WHERE surah = 6 AND ayah = 6"
            )
            .await,
            1,
            "the dedupe still collapses the shared verse to one keeper deterministically"
        );
        assert!(index_exists(&db, "bookmarks", "idx_bookmarks_user_verse").await);
    }

    #[tokio::test]
    async fn down_drops_the_unique_index() {
        let db = mem_db().await;
        create_fixture_tables(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.down(&mgr).await.unwrap();
        assert!(!index_exists(&db, "bookmarks", "idx_bookmarks_user_verse").await);
    }

    #[tokio::test]
    async fn full_migrator_run_creates_the_unique_index() {
        use crate::Migrator;
        use sea_orm_migration::MigratorTrait;

        let db = mem_db().await;
        Migrator::up(&db, None).await.unwrap();
        assert!(index_exists(&db, "bookmarks", "idx_bookmarks_user_verse").await);
    }
}
