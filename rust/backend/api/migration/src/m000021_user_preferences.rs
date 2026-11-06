use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::TransactionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

// One row per synced preference key: each setting, the home layout document and
// the reading bookmark. Each row is one register (contracts/sync.md), so two
// settings changed on two devices both survive. Lives as long as the account.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS user_preferences (
        user_id INTEGER NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        stamp TEXT NOT NULL,
        rev INTEGER NOT NULL,
        updated_at TEXT NOT NULL,
        expires_at INTEGER NOT NULL DEFAULT 253402300799,
        retention_class TEXT NOT NULL DEFAULT 'account_lifetime',
        PRIMARY KEY (user_id, key),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )",
    "CREATE INDEX IF NOT EXISTS idx_user_preferences_user_rev ON user_preferences (user_id, rev)",
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
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared("DROP TABLE IF EXISTS user_preferences")
            .await?;
        txn.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    #[tokio::test]
    async fn up_twice_is_safe_after_a_full_run() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();
        assert!(mgr.has_table("user_preferences").await.unwrap());
    }
}
