use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Offline-sync bookmarks + bookmark folders (bookmark_v1). Clients are
// offline-first: they mint UUIDv4 ids locally, queue mutations, and push them
// to POST /bookmark/v1/sync in FIFO rounds; the server resolves conflicts with
// last-writer-wins on updated_at (canonical UTC RFC3339 TEXT, compared
// lexicographically in the raw-SQL actions). folder_id is NULL = root; deleting
// a folder detaches its bookmarks to root (explicitly in the delete action,
// plus ON DELETE SET NULL as the schema-level backstop).
const CREATE_BOOKMARK_FOLDERS: &str = "CREATE TABLE IF NOT EXISTS bookmark_folders (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE CASCADE
)";

const CREATE_BOOKMARK_FOLDERS_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_bookmark_folders_user ON bookmark_folders (user_id)";

const CREATE_BOOKMARKS: &str = "CREATE TABLE IF NOT EXISTS bookmarks (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    folder_id TEXT,
    surah INTEGER NOT NULL,
    ayah INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (folder_id) REFERENCES bookmark_folders (id)
        ON UPDATE CASCADE ON DELETE SET NULL
)";

const CREATE_BOOKMARKS_USER_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_bookmarks_user ON bookmarks (user_id)";

const CREATE_BOOKMARKS_USER_FOLDER_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_bookmarks_user_folder ON bookmarks (user_id, folder_id)";

const STMTS: &[&str] = &[
    CREATE_BOOKMARK_FOLDERS,
    CREATE_BOOKMARK_FOLDERS_INDEX,
    CREATE_BOOKMARKS,
    CREATE_BOOKMARKS_USER_INDEX,
    CREATE_BOOKMARKS_USER_FOLDER_INDEX,
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
            "DROP TABLE IF EXISTS bookmarks",
            "DROP TABLE IF EXISTS bookmark_folders",
        ] {
            conn.execute_unprepared(stmt).await?;
        }
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

    async fn column_names(db: &sea_orm::DatabaseConnection, table: &str) -> Vec<String> {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!("PRAGMA table_info({table})"),
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|r| r.try_get_by_index::<String>(1).unwrap())
            .collect()
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

    async fn seed_users(db: &sea_orm::DatabaseConnection) {
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO users (id, name) VALUES (1, 'a'), (2, 'b')")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn fresh_migration_creates_both_tables() {
        let db = mem_db().await;
        seed_users(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        for (table, expected) in [
            (
                "bookmark_folders",
                vec!["id", "user_id", "name", "created_at", "updated_at"],
            ),
            (
                "bookmarks",
                vec![
                    "id",
                    "user_id",
                    "folder_id",
                    "surah",
                    "ayah",
                    "created_at",
                    "updated_at",
                ],
            ),
        ] {
            let cols = column_names(&db, table).await;
            for name in expected {
                assert!(
                    cols.iter().any(|c| c == name),
                    "{table} missing column {name}: {cols:?}"
                );
            }
        }

        assert!(index_exists(&db, "bookmark_folders", "idx_bookmark_folders_user").await);
        assert!(index_exists(&db, "bookmarks", "idx_bookmarks_user").await);
        assert!(index_exists(&db, "bookmarks", "idx_bookmarks_user_folder").await);
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let db = mem_db().await;
        seed_users(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            column_names(&db, "bookmark_folders").await.len(),
            5,
            "no duplicate columns on re-run"
        );
        assert_eq!(column_names(&db, "bookmarks").await.len(), 7);
    }

    #[tokio::test]
    async fn preexisting_tables_are_left_untouched() {
        let db = mem_db().await;
        seed_users(&db).await;
        // Out-of-band provisioned DB already carries its own bookmark tables.
        db.execute_unprepared(
            "CREATE TABLE bookmark_folders (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL, \
             name TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO bookmark_folders (id, user_id, name, created_at, updated_at) \
             VALUES ('f1', 1, 'Tafsir', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let count = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM bookmark_folders",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index::<i64>(0)
            .unwrap();
        assert_eq!(count, 1, "existing folder rows must survive the migration");
    }

    #[tokio::test]
    async fn folder_delete_sets_child_bookmarks_to_root() {
        let db = mem_db().await;
        seed_users(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        db.execute_unprepared(
            "INSERT INTO bookmark_folders (id, user_id, name) VALUES ('f1', 1, 'Tafsir')",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO bookmarks (id, user_id, folder_id, surah, ayah) \
             VALUES ('b1', 1, 'f1', 2, 255)",
        )
        .await
        .unwrap();
        db.execute_unprepared("DELETE FROM bookmark_folders WHERE id = 'f1'")
            .await
            .unwrap();

        let folder_id: Option<String> = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT folder_id FROM bookmarks WHERE id = 'b1'",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index(0)
            .unwrap();
        assert_eq!(
            folder_id, None,
            "ON DELETE SET NULL must move child bookmarks to root"
        );
    }

    #[tokio::test]
    async fn down_drops_both_tables() {
        let db = mem_db().await;
        seed_users(&db).await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.down(&mgr).await.unwrap();
        for table in ["bookmark_folders", "bookmarks"] {
            assert!(!mgr.has_table(table).await.unwrap());
        }
    }

    #[tokio::test]
    async fn full_migrator_run_creates_both_tables() {
        use crate::Migrator;
        use sea_orm_migration::MigratorTrait;

        let db = mem_db().await;
        Migrator::up(&db, None).await.unwrap();
        for table in ["bookmark_folders", "bookmarks"] {
            assert!(
                SchemaManager::new(&db).has_table(table).await.unwrap(),
                "Migrator::up must create {table}"
            );
        }
    }
}
