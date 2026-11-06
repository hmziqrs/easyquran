use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::TransactionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

// One reminder schedule per account and kind (F9). Times are minutes after local
// midnight in `time_zone` (an IANA name). `last_fired_day` is the local day the
// reminder last went out, which makes a reminder fire at most once a day. Lives as
// long as the account: the sweeper skips it and erasure deletes it.
const STMTS: &[&str] = &["CREATE TABLE IF NOT EXISTS reminder_schedules (
        user_id INTEGER NOT NULL,
        kind TEXT NOT NULL,
        enabled INTEGER NOT NULL,
        time_zone TEXT NOT NULL,
        reminder_minute INTEGER NOT NULL,
        quiet_start_minute INTEGER,
        quiet_end_minute INTEGER,
        last_fired_day TEXT,
        updated_at TEXT NOT NULL,
        expires_at INTEGER NOT NULL DEFAULT 253402300799,
        retention_class TEXT NOT NULL DEFAULT 'account_lifetime',
        PRIMARY KEY (user_id, kind),
        FOREIGN KEY (user_id) REFERENCES users (id)
            ON UPDATE CASCADE ON DELETE CASCADE
    )"];

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
        txn.execute_unprepared("DROP TABLE IF EXISTS reminder_schedules")
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
        assert!(mgr.has_table("reminder_schedules").await.unwrap());
    }
}
