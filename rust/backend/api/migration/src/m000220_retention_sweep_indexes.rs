use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Sweep indexes (A32). `retention.rs` picks rows by age with `julianday(<column>)` (or the
// `MAX(...)` expression for `user_bans`) compared to a constant cutoff. A plain index on the
// column cannot serve that predicate, so each index is an expression index over the exact
// expression the sweeper writes. Without them every sweep reads the whole table.
//
// Expression text must match `retention.rs` `Expiry::AgeDays` byte for byte in meaning:
// `julianday(t.<col>)`, or `julianday(t.<col>, 'unixepoch')` where the column holds epoch
// seconds (`enforcement_notices.created_at`, `case_evidence.captured_at`).
//
// `rate_limit_events` has `expires_at` (ExpiresAt rule) but no index on it; the sweep
// filters on `expires_at < now` and would scan it. Every statement is IF NOT EXISTS and runs
// in one transaction, so a re-run is a no-op.
const STMTS: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_user_sessions_last_seen_jd ON user_sessions (julianday(last_seen))",
    "CREATE INDEX IF NOT EXISTS idx_devices_last_seen_jd ON devices (julianday(last_seen_at))",
    "CREATE INDEX IF NOT EXISTS idx_user_devices_last_seen_jd ON user_devices (julianday(last_seen_at))",
    "CREATE INDEX IF NOT EXISTS idx_post_views_created_jd ON post_views (julianday(created_at))",
    "CREATE INDEX IF NOT EXISTS idx_reading_sessions_deleted_jd ON reading_sessions (julianday(deleted_at))",
    "CREATE INDEX IF NOT EXISTS idx_bookmarks_deleted_jd ON bookmarks (julianday(deleted_at))",
    "CREATE INDEX IF NOT EXISTS idx_bookmark_folders_deleted_jd ON bookmark_folders (julianday(deleted_at))",
    "CREATE INDEX IF NOT EXISTS idx_user_bans_age_jd ON user_bans (MAX(julianday(created_at), IFNULL(julianday(revoked_at), 0), IFNULL(julianday(expires_at), 0)))",
    "CREATE INDEX IF NOT EXISTS idx_enforcement_notices_created_jd ON enforcement_notices (julianday(created_at, 'unixepoch'))",
    "CREATE INDEX IF NOT EXISTS idx_case_evidence_captured_jd ON case_evidence (julianday(captured_at, 'unixepoch'))",
    "CREATE INDEX IF NOT EXISTS idx_reports_created_jd ON reports (julianday(created_at))",
    "CREATE INDEX IF NOT EXISTS idx_post_comments_updated_jd ON post_comments (julianday(updated_at))",
    "CREATE INDEX IF NOT EXISTS idx_comment_flags_created_jd ON comment_flags (julianday(created_at))",
    "CREATE INDEX IF NOT EXISTS idx_notifications_created_jd ON notifications (julianday(created_at))",
    "CREATE INDEX IF NOT EXISTS idx_rate_limit_events_expires_at ON rate_limit_events (expires_at)",
];

const INDEX_NAMES: &[&str] = &[
    "idx_user_sessions_last_seen_jd",
    "idx_devices_last_seen_jd",
    "idx_user_devices_last_seen_jd",
    "idx_post_views_created_jd",
    "idx_reading_sessions_deleted_jd",
    "idx_bookmarks_deleted_jd",
    "idx_bookmark_folders_deleted_jd",
    "idx_user_bans_age_jd",
    "idx_enforcement_notices_created_jd",
    "idx_case_evidence_captured_jd",
    "idx_reports_created_jd",
    "idx_post_comments_updated_jd",
    "idx_comment_flags_created_jd",
    "idx_notifications_created_jd",
    "idx_rate_limit_events_expires_at",
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
        for name in INDEX_NAMES {
            conn.execute_unprepared(&format!("DROP INDEX IF EXISTS {name}"))
                .await?;
        }
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
    async fn up_twice_is_safe_and_every_sweep_index_exists() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        for name in INDEX_NAMES {
            let found = count(
                &db,
                &format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = '{name}'"
                ),
            )
            .await;
            assert_eq!(found, 1, "missing index {name}");
        }
    }

    #[tokio::test]
    async fn sweep_predicate_uses_an_index_on_the_age_column() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let plan = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "EXPLAIN QUERY PLAN SELECT rowid FROM user_sessions WHERE julianday(last_seen) < julianday(1700000000, 'unixepoch', '-90 days')".to_string(),
            ))
            .await
            .unwrap();
        let text: String = plan
            .iter()
            .map(|row| row.try_get_by_index::<String>(3).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            text.contains("idx_user_sessions_last_seen_jd"),
            "plan did not use the sweep index: {text}"
        );
    }
}
