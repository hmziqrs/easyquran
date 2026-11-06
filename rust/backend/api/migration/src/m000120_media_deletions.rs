use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Media takedown tombstones (A30). One row per removed media item, written in the same
// transaction as the media and variant row deletes. No FK to media or users: the row it
// names is gone by design. `object_keys` lists every storage key the takedown removed.
// 12-month retention class, matching audit evidence.
//
// Every statement is IF NOT EXISTS and the block is one transaction, so a re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS media_deletions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        media_id INTEGER NOT NULL,
        bucket TEXT,
        object_keys TEXT NOT NULL,
        uploader_id INTEGER,
        deleted_by INTEGER,
        deleted_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_media_deletions_media_id ON media_deletions (media_id)",
    "CREATE INDEX IF NOT EXISTS idx_media_deletions_expires_at ON media_deletions (expires_at)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        for stmt in STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS media_deletions")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DbBackend, Statement};
    use sea_orm_migration::MigratorTrait;

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    #[tokio::test]
    async fn up_twice_is_safe_and_tombstones_have_no_fk_and_expiry_not_null() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let fks = count(
            &db,
            "SELECT COUNT(*) FROM pragma_foreign_key_list('media_deletions')",
        )
        .await;
        assert_eq!(fks, 0, "media_deletions must not reference any table");
        let not_null = count(
            &db,
            "SELECT COUNT(*) FROM pragma_table_info('media_deletions') WHERE name IN ('expires_at', 'retention_class', 'object_keys', 'deleted_at') AND \"notnull\" = 1",
        )
        .await;
        assert_eq!(not_null, 4);
    }
}
