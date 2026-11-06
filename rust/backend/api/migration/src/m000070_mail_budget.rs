use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Mail budget and suppression governance (A29).
//
// email_suppression.blocks_transactional: constant default 1, so existing rows keep
// blocking. Rows that came from the mail webhook are set to 0: a webhook bounce or
// complaint cannot block password reset or verification until an admin reviews it.
//
// newsletter_subscribers: expires_at and retention_class (A29 item 5). Confirmed rows
// keep 12 months, pending and unsubscribed rows 30 days. Tokens were plaintext UUIDs;
// they are now stored as HMAC. An HMAC cannot be computed in SQL, so legacy plaintext
// tokens are blanked. Pending legacy subscribers must subscribe again (pre-launch).
// Confirmed legacy subscribers keep working through the one-click link, which does
// not use the stored token.
//
// Every guarded ADD runs on a populated table: constant defaults only. SQLite has no
// ADD COLUMN IF NOT EXISTS, so each add is guarded by a column probe.
const ADD_BLOCKS: &str = "ALTER TABLE email_suppression ADD COLUMN blocks_transactional INTEGER NOT NULL DEFAULT 1 CHECK (blocks_transactional IN (0, 1))";
const ADD_EXPIRES: &str =
    "ALTER TABLE newsletter_subscribers ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0";
const ADD_CLASS: &str = "ALTER TABLE newsletter_subscribers ADD COLUMN retention_class TEXT NOT NULL DEFAULT 'newsletter_pending'";

const BACKFILL_STMTS: &[&str] = &[
    "UPDATE email_suppression SET blocks_transactional = 0 WHERE source = 'mail-webhook'",
    "UPDATE newsletter_subscribers SET token = '' WHERE length(token) <> 64",
    "UPDATE newsletter_subscribers
        SET expires_at = CAST(strftime('%s', 'now') AS INTEGER) + 31536000,
            retention_class = 'newsletter_confirmed'
        WHERE status = 'confirmed'",
    "UPDATE newsletter_subscribers
        SET expires_at = CAST(strftime('%s', 'now') AS INTEGER) + 2592000,
            retention_class = 'newsletter_unsubscribed'
        WHERE status = 'unsubscribed'",
    "UPDATE newsletter_subscribers
        SET expires_at = CAST(strftime('%s', 'now') AS INTEGER) + 2592000,
            retention_class = 'newsletter_pending'
        WHERE status NOT IN ('confirmed', 'unsubscribed')",
    "CREATE INDEX IF NOT EXISTS idx_newsletter_subscribers_expires_at ON newsletter_subscribers (expires_at)",
    "CREATE INDEX IF NOT EXISTS idx_newsletter_subscribers_token ON newsletter_subscribers (token)",
];

async fn has_column<C: ConnectionTrait>(
    conn: &C,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 FROM pragma_table_info(?) WHERE name = ?",
            [table.into(), column.into()],
        ))
        .await?;
    Ok(row.is_some())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        if !has_column(&txn, "email_suppression", "blocks_transactional").await? {
            txn.execute_unprepared(ADD_BLOCKS).await?;
        }
        if !has_column(&txn, "newsletter_subscribers", "expires_at").await? {
            txn.execute_unprepared(ADD_EXPIRES).await?;
        }
        if !has_column(&txn, "newsletter_subscribers", "retention_class").await? {
            txn.execute_unprepared(ADD_CLASS).await?;
        }
        for stmt in BACKFILL_STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    // Keeps the columns and the rows on rollback.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection};
    use sea_orm_migration::MigratorTrait;

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                sql.to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    #[tokio::test]
    async fn up_twice_adds_columns_once_and_keeps_defaults() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('email_suppression') WHERE name = 'blocks_transactional' AND \"notnull\" = 1"
            )
            .await,
            1
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('newsletter_subscribers') WHERE name IN ('expires_at', 'retention_class') AND \"notnull\" = 1"
            )
            .await,
            2
        );
    }

    #[tokio::test]
    async fn webhook_suppression_backfill_and_legacy_token_blank() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db.execute_unprepared(
            "INSERT INTO email_suppression (recipient, reason, source, permanent, last_seen, created_at, updated_at)
             VALUES ('a@x.com', 'complaint', 'mail-webhook', 1, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO newsletter_subscribers (email, status, token, created_at, updated_at)
             VALUES ('b@x.com', 'confirmed', '2f1c6b0e-1111-4222-8333-444455556666', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
        )
        .await
        .unwrap();
        // Rows written after the full run: re-run the backfill, as a restore would.
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM email_suppression WHERE recipient = 'a@x.com' AND blocks_transactional = 0"
            )
            .await,
            1
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM newsletter_subscribers WHERE email = 'b@x.com' AND token = '' AND retention_class = 'newsletter_confirmed' AND expires_at > 0"
            )
            .await,
            1
        );
    }
}
