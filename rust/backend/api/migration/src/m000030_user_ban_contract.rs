use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Ban contract core (A19). One active ban per account, enforced by a partial
// unique index. Dedupe keeps the newest active row and revokes the older ones
// at their own created_at, so no ban row is deleted (evidence stays).
//
// enforcement_notices is the case record for each ban decision: it has no FK
// to users, so it outlives the account. expires_at is the ban expiry (CHECK >=
// decided_at); retention_class drives the sweeper (A32).
const DEDUPE_ACTIVE_BANS: &str = "UPDATE user_bans SET revoked_at = created_at \
     WHERE revoked_at IS NULL AND id NOT IN ( \
         SELECT MAX(id) FROM user_bans WHERE revoked_at IS NULL GROUP BY user_id \
     )";

const STMTS: &[&str] = &[
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_bans_one_active \
     ON user_bans (user_id) WHERE revoked_at IS NULL",
    "CREATE INDEX IF NOT EXISTS idx_user_bans_user_id ON user_bans (user_id)",
    "CREATE TABLE IF NOT EXISTS enforcement_notices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL,
        ban_id INTEGER NOT NULL,
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
        retention_class TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS idx_enforcement_notices_user ON enforcement_notices (user_id)",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs (resource_type, resource_id)",
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared(DEDUPE_ACTIVE_BANS).await?;
        for stmt in STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    // Drops the active-ban index only. The notices table and every row in it,
    // and every ban row, are kept on rollback.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared("DROP INDEX IF EXISTS idx_user_bans_one_active")
            .await?;
        txn.commit().await
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
    async fn up_twice_is_safe_after_a_full_run() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'idx_user_bans_one_active'"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn dedupe_keeps_newest_active_ban_and_down_keeps_notices() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.down(&mgr).await.unwrap();
        db.execute_unprepared(
            "INSERT INTO users (id, name, email, role, is_verified, two_fa_enabled, \
             session_auth_secret, created_at, updated_at) VALUES \
             (1, 'a', 'a@x.test', 0, 1, 0, 's', '2024-01-01T00:00:00+00:00', '2024-01-01T00:00:00+00:00')",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO user_bans (id, user_id, created_at) VALUES \
             (1, 1, '2024-01-01T00:00:00+00:00'), (2, 1, '2024-02-01T00:00:00+00:00')",
        )
        .await
        .unwrap();
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM user_bans WHERE revoked_at IS NULL"
            )
            .await,
            1
        );
        assert_eq!(
            count(&db, "SELECT id FROM user_bans WHERE revoked_at IS NULL").await,
            2
        );

        db.execute_unprepared(
            "INSERT INTO enforcement_notices (user_id, ban_id, reason_code, corroborator, \
             decided_by, decided_at, effective_at, expires_at, created_at, retention_class) \
             VALUES (1, 2, 'spam', 'none', 1, 10, 10, 20, 10, 'enforcement_12m')",
        )
        .await
        .unwrap();
        Migration.down(&mgr).await.unwrap();
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM enforcement_notices").await,
            1
        );
        assert_eq!(count(&db, "SELECT COUNT(*) FROM user_bans").await, 2);
        assert!(db
            .execute_unprepared("INSERT INTO user_bans (user_id) VALUES (1)")
            .await
            .is_ok());
    }
}
