use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::TransactionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Email change, verify before switch (A12). A change request parks the new
// address here with a code hash; `users.email` only moves once that code is
// proven on the new mailbox. A typo therefore leaves the account untouched and
// signed in: the owner submits the right address again and the row is replaced.
// One pending change per user (user_id is the key). Not evidence, so no FK:
// rows expire by `expires_at` and are dropped on read. Account-lifetime rules
// do not apply, so retention is `pending_email_change`.
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS email_change_pending (
        user_id INTEGER PRIMARY KEY NOT NULL,
        new_email VARCHAR(320) NOT NULL,
        identity_hash TEXT,
        code_hash TEXT NOT NULL,
        expires_at INTEGER NOT NULL,
        retention_class TEXT NOT NULL,
        created_at TIMESTAMP WITH TIME ZONE NOT NULL,
        updated_at TIMESTAMP WITH TIME ZONE NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_email_change_pending_expires_at ON email_change_pending (expires_at)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        for stmt in STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared("DROP TABLE IF EXISTS email_change_pending")
            .await?;
        txn.commit().await?;
        Ok(())
    }
}
