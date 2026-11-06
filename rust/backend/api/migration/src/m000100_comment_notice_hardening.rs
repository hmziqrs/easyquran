use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Comment moderation hardening (A27).
//
// 1. post_comments.flags_cleared_at: flags created at or before this instant are
//    no longer counted in flags_count. Rows stay (they are case evidence).
// 2. enforcement_notices gains a subject. A comment notice has no ban, so ban_id
//    becomes nullable, and subject_kind + comment_id say what the notice is about.
//    SQLite cannot drop NOT NULL in place, so the table is rebuilt, the same way
//    m000032 rebuilt user_bans. Every existing row keeps its id and values.
//    `automated` records whether a machine made the decision (false for admin actions).
//
// Re-runnable: the notices rebuild is skipped once subject_kind exists, and the
// column add is guarded by a probe. The whole migration is one transaction.

const ADD_FLAGS_CLEARED: &str = "ALTER TABLE post_comments ADD COLUMN flags_cleared_at TEXT";

const CREATE_REBUILT: &str = "CREATE TABLE IF NOT EXISTS enforcement_notices_rebuilt (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    ban_id INTEGER,
    subject_kind TEXT NOT NULL DEFAULT 'ban' CHECK (subject_kind IN ('ban', 'comment')),
    comment_id INTEGER,
    reason_code TEXT NOT NULL,
    reason_text TEXT,
    corroborator TEXT NOT NULL CHECK (corroborator IN ('credential_reuse', 'content_match', 'ip', 'none')),
    decided_by INTEGER NOT NULL,
    decided_at INTEGER NOT NULL,
    effective_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK (expires_at >= decided_at),
    delivery_state TEXT CHECK (delivery_state IS NULL OR delivery_state IN ('sent', 'suppressed', 'no_contact_details', 'send_failed')),
    sent_at INTEGER,
    appeal_token_hash TEXT,
    created_at INTEGER NOT NULL,
    retention_class TEXT NOT NULL,
    notice_channel TEXT NOT NULL DEFAULT 'email' CHECK (notice_channel IN ('email')),
    shared_device_checked INTEGER NOT NULL DEFAULT 0 CHECK (shared_device_checked IN (0, 1)),
    automated INTEGER NOT NULL DEFAULT 0 CHECK (automated IN (0, 1)),
    CHECK (
        (subject_kind = 'ban' AND ban_id IS NOT NULL AND comment_id IS NULL)
        OR (subject_kind = 'comment' AND comment_id IS NOT NULL AND ban_id IS NULL)
    )
)";

const COPY_ROWS: &str = "INSERT INTO enforcement_notices_rebuilt \
     (id, user_id, ban_id, reason_code, reason_text, corroborator, decided_by, decided_at, \
      effective_at, expires_at, delivery_state, sent_at, appeal_token_hash, created_at, \
      retention_class, notice_channel, shared_device_checked) \
     SELECT id, user_id, ban_id, reason_code, reason_text, corroborator, decided_by, decided_at, \
      effective_at, expires_at, delivery_state, sent_at, appeal_token_hash, created_at, \
      retention_class, notice_channel, shared_device_checked \
     FROM enforcement_notices";

// The id high-water mark carries over so a notice id is never reused.
const SEQ_CARRY_INSERT: &str = "INSERT INTO sqlite_sequence (name, seq) \
     SELECT 'enforcement_notices_rebuilt', 0 \
     WHERE EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'enforcement_notices') \
       AND NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'enforcement_notices_rebuilt')";
const SEQ_CARRY_UPDATE: &str = "UPDATE sqlite_sequence SET seq = MAX(seq, \
     (SELECT seq FROM sqlite_sequence WHERE name = 'enforcement_notices')) \
     WHERE name = 'enforcement_notices_rebuilt'";

const FINISH: &[&str] = &[
    "DROP TABLE IF EXISTS enforcement_notices",
    "ALTER TABLE enforcement_notices_rebuilt RENAME TO enforcement_notices",
    "CREATE INDEX IF NOT EXISTS idx_enforcement_notices_user ON enforcement_notices (user_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_enforcement_notices_appeal_token \
     ON enforcement_notices (appeal_token_hash) WHERE appeal_token_hash IS NOT NULL",
    "CREATE INDEX IF NOT EXISTS idx_enforcement_notices_comment \
     ON enforcement_notices (comment_id) WHERE comment_id IS NOT NULL",
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
        if !has_column(&txn, "post_comments", "flags_cleared_at").await? {
            txn.execute_unprepared(ADD_FLAGS_CLEARED).await?;
        }
        if !has_column(&txn, "enforcement_notices", "subject_kind").await? {
            txn.execute_unprepared(CREATE_REBUILT).await?;
            txn.execute_unprepared(COPY_ROWS).await?;
            txn.execute_unprepared(SEQ_CARRY_INSERT).await?;
            txn.execute_unprepared(SEQ_CARRY_UPDATE).await?;
            for stmt in FINISH {
                txn.execute_unprepared(stmt).await?;
            }
        }
        txn.commit().await
    }

    // Keeps every column and row. Rolling back does not restore the ban-only shape.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DbBackend};
    use sea_orm_migration::MigratorTrait;

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        let row = db
            .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    async fn exec_ok(db: &DatabaseConnection, sql: &str) -> bool {
        db.execute_unprepared(sql).await.is_ok()
    }

    #[tokio::test]
    async fn up_twice_is_safe_and_keeps_both_subject_shapes() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('enforcement_notices') WHERE name = 'subject_kind'"
            )
            .await,
            1
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_table_info('post_comments') WHERE name = 'flags_cleared_at'"
            )
            .await,
            1
        );
        // A comment notice with no ban is accepted; a ban notice with no ban is not.
        assert!(
            exec_ok(
                &db,
                "INSERT INTO enforcement_notices (user_id, ban_id, subject_kind, comment_id, \
                 reason_code, corroborator, decided_by, decided_at, effective_at, expires_at, \
                 created_at, retention_class) VALUES \
                 (1, NULL, 'comment', 7, 'spam', 'none', 1, 10, 10, 10, 10, 'enforcement_12m')"
            )
            .await
        );
        assert!(
            !exec_ok(
                &db,
                "INSERT INTO enforcement_notices (user_id, ban_id, subject_kind, reason_code, \
                 corroborator, decided_by, decided_at, effective_at, expires_at, created_at, \
                 retention_class) VALUES \
                 (1, NULL, 'ban', 'spam', 'none', 1, 10, 10, 10, 10, 'enforcement_12m')"
            )
            .await
        );
    }

    #[tokio::test]
    async fn rebuild_keeps_existing_notice_rows_and_ids() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        // Run the schema up to the migration before this one, then seed a ban-shaped notice.
        Migrator::up(&db, Some(Migrator::migrations().len() as u32 - 1))
            .await
            .unwrap();
        db.execute_unprepared(
            "INSERT INTO enforcement_notices (id, user_id, ban_id, reason_code, corroborator, \
             decided_by, decided_at, effective_at, expires_at, created_at, retention_class) \
             VALUES (42, 1, 9, 'spam', 'none', 1, 10, 10, 20, 10, 'enforcement_12m')",
        )
        .await
        .unwrap();
        Migrator::up(&db, None).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM enforcement_notices WHERE id = 42 AND ban_id = 9 \
                 AND subject_kind = 'ban' AND automated = 0"
            )
            .await,
            1
        );
    }
}
