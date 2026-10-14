use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Restores `newsletter_subscribers`, which the entity (`newsletter_subscriber`),
// the newsletter controller, and the seeder all use, but which was dropped from
// the chain when m20250811_000011 was squashed into m000001_init (4876e8e1).
// Same columns and indexes as the pre-squash table. IF NOT EXISTS keeps any DB
// that already carries it untouched. Status values: pending | confirmed |
// unsubscribed (see ruxlog-types SubscriberStatus).
const STMTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS newsletter_subscribers (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        email VARCHAR(320) NOT NULL,
        status VARCHAR(32) NOT NULL DEFAULT 'pending',
        token VARCHAR(128) NOT NULL,
        created_at TIMESTAMP WITH TIME ZONE NOT NULL,
        updated_at TIMESTAMP WITH TIME ZONE NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS uq_newsletter_subscribers_email ON newsletter_subscribers (email)",
    "CREATE INDEX IF NOT EXISTS idx_newsletter_subscribers_status ON newsletter_subscribers (status)",
    "CREATE INDEX IF NOT EXISTS idx_newsletter_subscribers_created_at ON newsletter_subscribers (created_at)",
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
        conn.execute_unprepared("DROP TABLE IF EXISTS newsletter_subscribers").await?;
        Ok(())
    }
}
