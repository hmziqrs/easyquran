use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Media upload hardening (A31). Indexes for the per-uploader quota count (uploader_id) and the
// dedup lookup on every upload (content_hash). Both are plain, non-unique: two uploaders may
// legitimately hold the same bytes, and dedup already picks one row.
//
// Every statement is IF NOT EXISTS and the block is one transaction, so a re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_media_uploader_id ON media (uploader_id)",
    "CREATE INDEX IF NOT EXISTS idx_media_content_hash ON media (content_hash)",
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
        let conn = manager.get_connection();
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_media_uploader_id")
            .await?;
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_media_content_hash")
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
    async fn up_twice_is_safe_and_creates_both_indexes() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        let indexes = count(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN ('idx_media_uploader_id', 'idx_media_content_hash')",
        )
        .await;
        assert_eq!(indexes, 2);
    }
}
