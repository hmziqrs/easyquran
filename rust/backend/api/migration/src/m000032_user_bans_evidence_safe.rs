use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Evidence-safe user_bans (A19b). m000001 created user_bans with a FK cascade
// from users, so deleting an account erased its ban history.
// This rebuilds the table with no foreign keys: user_id, banned_by and
// revoked_by stay plain integers, so account deletion cannot reach ban rows
// (same shape as enforcement_notices, which has no FK to users).
//
// Re-runnable: the copy goes through INSERT OR IGNORE and the table is rebuilt
// on every run, so a second run over the new shape loses nothing. Dedupe runs
// first. It only revokes older active rows (never deletes), so the partial
// unique index can be rebuilt on a populated table. The whole rebuild is one
// transaction. Dropping user_bans fires no cascade: it is the child side of the
// old FK and no table references it.
const DEDUPE_ACTIVE_BANS: &str = "UPDATE user_bans SET revoked_at = created_at \
     WHERE revoked_at IS NULL AND id NOT IN ( \
         SELECT MAX(id) FROM user_bans WHERE revoked_at IS NULL GROUP BY user_id \
     )";

const CREATE_REBUILT: &str = "CREATE TABLE IF NOT EXISTS user_bans_rebuilt (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    reason TEXT,
    banned_by INTEGER,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TEXT,
    revoked_by INTEGER
)";

const COPY_ROWS: &str = "INSERT OR IGNORE INTO user_bans_rebuilt \
     (id, user_id, reason, banned_by, expires_at, created_at, revoked_at, revoked_by) \
     SELECT id, user_id, reason, banned_by, expires_at, created_at, revoked_at, revoked_by \
     FROM user_bans";

// Carry the id high-water mark across the rebuild so a ban id is never reused.
// sqlite_sequence has no unique key on name, hence the NOT EXISTS guard.
const SEQ_CARRY_INSERT: &str = "INSERT INTO sqlite_sequence (name, seq) \
     SELECT 'user_bans_rebuilt', 0 \
     WHERE EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'user_bans') \
       AND NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'user_bans_rebuilt')";
const SEQ_CARRY_UPDATE: &str = "UPDATE sqlite_sequence SET seq = MAX(seq, \
     (SELECT seq FROM sqlite_sequence WHERE name = 'user_bans')) \
     WHERE name = 'user_bans_rebuilt'";

const FINISH: &[&str] = &[
    "DROP TABLE IF EXISTS user_bans",
    "ALTER TABLE user_bans_rebuilt RENAME TO user_bans",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_bans_one_active \
     ON user_bans (user_id) WHERE revoked_at IS NULL",
    "CREATE INDEX IF NOT EXISTS idx_user_bans_user_id ON user_bans (user_id)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared(DEDUPE_ACTIVE_BANS).await?;
        txn.execute_unprepared(CREATE_REBUILT).await?;
        txn.execute_unprepared(COPY_ROWS).await?;
        txn.execute_unprepared(SEQ_CARRY_INSERT).await?;
        txn.execute_unprepared(SEQ_CARRY_UPDATE).await?;
        for stmt in FINISH {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    // Keeps the evidence-safe shape and every row. Rolling back does not
    // restore the cascade.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
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

    async fn connect() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        db
    }

    async fn seed_user(db: &DatabaseConnection, id: i32) {
        db.execute_unprepared(&format!(
            "INSERT INTO users (id, name, email, role, is_verified, two_fa_enabled, \
             session_auth_secret, created_at, updated_at) VALUES \
             ({id}, 'u{id}', 'u{id}@x.test', 0, 1, 0, 's', \
             '2024-01-01T00:00:00+00:00', '2024-01-01T00:00:00+00:00')"
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn up_twice_is_safe_after_a_full_run() {
        let db = connect().await;
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
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('user_bans')"
            )
            .await,
            0
        );
    }

    #[tokio::test]
    async fn rebuild_on_populated_table_keeps_rows_and_drops_user_fk() {
        let db = connect().await;
        // Everything up to m000031, then the old CASCADE shape with data.
        let all = Migrator::migrations().len() as u32;
        Migrator::up(&db, Some(all - 1)).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('user_bans')"
            )
            .await,
            3,
            "old shape has three FKs before the rebuild"
        );
        seed_user(&db, 1).await;
        seed_user(&db, 2).await;
        // The m000030 partial index already allows one active row per user, so
        // the populated shape has at most one active ban per account.
        db.execute_unprepared(
            "INSERT INTO user_bans (id, user_id, reason, banned_by, created_at, revoked_at) VALUES \
             (1, 1, 'spam', 2, '2024-01-01T00:00:00+00:00', '2024-01-02T00:00:00+00:00'), \
             (2, 1, 'spam', 2, '2024-02-01T00:00:00+00:00', NULL), \
             (3, 2, 'abuse', NULL, '2024-03-01T00:00:00+00:00', NULL), \
             (4, 1, 'spam', 2, '2024-04-01T00:00:00+00:00', '2024-04-02T00:00:00+00:00')",
        )
        .await
        .unwrap();

        Migrator::up(&db, None).await.unwrap();

        // Nothing deleted; the one active row per user survives the rebuild.
        assert_eq!(count(&db, "SELECT COUNT(*) FROM user_bans").await, 4);
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM user_bans WHERE revoked_at IS NULL"
            )
            .await,
            2
        );
        assert_eq!(
            count(
                &db,
                "SELECT id FROM user_bans WHERE user_id = 1 AND revoked_at IS NULL"
            )
            .await,
            2
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM pragma_foreign_key_list('user_bans')"
            )
            .await,
            0
        );

        // Deleting the account leaves its ban history in place.
        db.execute_unprepared("DELETE FROM users WHERE id = 1")
            .await
            .unwrap();
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM user_bans WHERE user_id = 1").await,
            3
        );
    }

    #[tokio::test]
    async fn rebuild_never_reuses_a_deleted_ban_id() {
        let db = connect().await;
        Migrator::up(&db, None).await.unwrap();
        seed_user(&db, 1).await;
        db.execute_unprepared(
            "INSERT INTO user_bans (id, user_id, revoked_at) VALUES (1, 1, '2024-01-01'), (2, 1, '2024-01-01')",
        )
        .await
        .unwrap();
        db.execute_unprepared("DELETE FROM user_bans WHERE id = 2")
            .await
            .unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        db.execute_unprepared(
            "INSERT INTO user_bans (user_id, revoked_at) VALUES (1, '2024-01-01')",
        )
        .await
        .unwrap();
        assert_eq!(
            count(&db, "SELECT MAX(id) FROM user_bans").await,
            3,
            "new ban must not reuse id 2"
        );
    }
}
