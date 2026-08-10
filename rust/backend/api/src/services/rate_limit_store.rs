//! SQLite L2 durability for the in-memory rate-limit store; enforcement stays in-process (L1), this layer only periodically flushes a snapshot so blocks survive a restart. Do NOT add a DB write to the request hot path.

// W3a escalation engine. Declared here (rather than in services/mod.rs) so the
// new module adds zero unlisted-file edits: services/escalation.rs is the allowed
// new file and this `#[path]` registration resolves to it. The engine is
// request-hot-path logic that talks to the in-memory gate store only (never the
// DB), matching this file's discipline.
#[path = "escalation.rs"]
pub mod escalation;

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rux_request_gate::{BlockScope, BucketSnapshot, InMemoryStore};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DbErr, Statement, TransactionError, TransactionTrait, Value,
};
use tracing::warn;

use crate::config::env::env_u64;

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS rate_limit_state (
    key TEXT PRIMARY KEY,
    fixed_count INTEGER NOT NULL DEFAULT 0,
    fixed_expires_at INTEGER NOT NULL DEFAULT 0,
    block_until_at INTEGER NOT NULL DEFAULT 0,
    block_scope TEXT NOT NULL DEFAULT 'Temp'
)";

const FLUSH_INTERVAL_SECS: u64 = 10;

/// A slow companion flush (the translation-popularity snapshot) rides the same
/// task every Nth tick (~60s at N=6) so the shared SeaORM connection never has a
/// second spawned writer competing for it. The cadence lives here on purpose:
/// the companion is wired by the bin as an opaque callback, so this module never
/// imports the quran/translation stack (no layering inversion).
const SLOW_FLUSH_EVERY_N_TICKS: u32 = 6;

const ACTIVE_BAN_MAX_DEFAULT: u64 = 2_000;

// One async operation lock shared by the periodic flush and (future) admin
// ban mutations (W3c DELETE /admin/bans). Holding it serializes durable
// writes so a periodic snapshot cannot resurrect a just-lifted ban and an
// admin delete cannot interleave with an in-flight flush. It is held only
// across SQLite I/O; the in-memory bucket mutex is never held across I/O
// (snapshot() copies out under the bucket lock, then releases it).
pub(crate) static PERSISTENCE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn epoch_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn active_ban_max() -> usize {
    env_u64("QURAN_ACTIVE_BAN_MAX", ACTIVE_BAN_MAX_DEFAULT) as usize
}

pub async fn ensure_table(db: &sea_orm::DatabaseConnection) {
    if let Err(e) = db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            CREATE_TABLE,
        ))
        .await
    {
        warn!(error = %e, "failed to create rate_limit_state table (non-fatal)");
    }
}

pub async fn load(db: &sea_orm::DatabaseConnection) -> Vec<BucketSnapshot> {
    match db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT key, fixed_count, fixed_expires_at, block_until_at, block_scope \
             FROM rate_limit_state",
            Vec::<Value>::new(),
        ))
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .filter_map(|row| {
                let key: Option<String> = row.try_get_by_index(0).ok();
                let fixed_count: i64 = row.try_get_by_index(1).unwrap_or(0);
                let fixed_expires_at: i64 = row.try_get_by_index(2).unwrap_or(0);
                let block_until_at: i64 = row.try_get_by_index(3).unwrap_or(0);
                let scope_raw: String = row
                    .try_get_by_index(4)
                    .unwrap_or_else(|_| "Temp".to_string());
                let block_scope = match BlockScope::from_db_str(&scope_raw) {
                    Some(s) => s,
                    None => {
                        warn!(
                            scope = %scope_raw,
                            "unknown block_scope in rate_limit_state (fail-closed as Temp)"
                        );
                        BlockScope::Temp
                    }
                };
                key.map(|key| BucketSnapshot {
                    key,
                    fixed_count: fixed_count.max(0) as u64,
                    fixed_expires_at,
                    block_until_at,
                    block_scope,
                })
            })
            .collect(),
        Err(e) => {
            warn!(error = %e, "failed to load rate_limit_state (starting with an empty store)");
            Vec::new()
        }
    }
}

/// Spawn the single periodic durability task. Every `FLUSH_INTERVAL_SECS` it
/// snapshots the in-memory gate store into SQLite under `PERSISTENCE_LOCK`. When
/// `every_sixth_tick` is `Some`, that callback runs every `SLOW_FLUSH_EVERY_N_TICKS`-th
/// tick on the SAME task — used by the bin to attach the translation-popularity
/// flush so a second spawned writer never competes for the shared SeaORM connection.
/// The callback runs OUTSIDE `PERSISTENCE_LOCK` (it owns its own transaction against
/// a separate table and never contends with admin ban mutations) and owns its own
/// `DatabaseConnection` clone, so no guard is held across its `.await`.
pub fn spawn_flush_task<E, EFut>(
    db: sea_orm::DatabaseConnection,
    store: Arc<InMemoryStore>,
    every_sixth_tick: Option<E>,
) where
    E: Fn() -> EFut + Send + Sync + 'static,
    EFut: std::future::Future<Output = ()> + Send + 'static,
{
    let max = active_ban_max();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut tick = 0u32;
        loop {
            ticker.tick().await;
            tick = tick.wrapping_add(1);
            // Serialize durable writes against admin mutations; snapshot() is
            // taken under the lock so a concurrent un-ban cannot lose to a
            // stale snapshot. The bucket mutex is released inside snapshot()
            // before any SQLite I/O below.
            let result = {
                let _guard = PERSISTENCE_LOCK.lock().await;
                let snaps = store.snapshot();
                if max > 0 {
                    let now = epoch_now();
                    let active = snaps.iter().filter(|s| s.block_until_at > now).count();
                    if active > max {
                        // Over capacity (e.g. config lowered or pre-existing
                        // state): keep all active bans, keep fixed limiting,
                        // signal saturation. New ban creation is declined at
                        // the L1 gate (W3a escalation) before it reaches here.
                        store.incr_saturation();
                    }
                }
                flush(&db, snaps).await
            };
            if let Err(e) = result {
                warn!(error = %e, "rate-limit L2 flush failed (non-fatal)");
            }
            store.prune();
            // Slow companion (translation popularity) shares this task: every
            // Nth tick, outside the persistence lock. The callback owns its own
            // transaction against a separate table, so it must not be serialized
            // with admin ban mutations (that would only widen the critical
            // section and re-introduce the cross-store contention W1 removed).
            if tick.is_multiple_of(SLOW_FLUSH_EVERY_N_TICKS) {
                if let Some(cb) = every_sixth_tick.as_ref() {
                    cb().await;
                }
            }
        }
    });
}

pub(crate) async fn flush(
    db: &sea_orm::DatabaseConnection,
    snaps: Vec<BucketSnapshot>,
) -> Result<(), DbErr> {
    let now = epoch_now();
    // One transaction on the shared connection: a single DELETE of expired
    // rows followed by the upserts. Per-row autocommits would turn the 10s
    // flush into a write storm once W3a adds a long-lived row per banned
    // identity. Expired rows are purged first; active keys remain only until
    // their fixed or block expiry.
    db.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "DELETE FROM rate_limit_state WHERE fixed_expires_at <= ? AND block_until_at <= ?",
                vec![Value::from(now), Value::from(now)],
            ))
            .await?;

            for s in &snaps {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "INSERT INTO rate_limit_state \
                     (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
                     VALUES (?, ?, ?, ?, ?) \
                     ON CONFLICT(key) DO UPDATE SET \
                     fixed_count=excluded.fixed_count, \
                     fixed_expires_at=excluded.fixed_expires_at, \
                     block_until_at=excluded.block_until_at, \
                     block_scope=excluded.block_scope",
                    vec![
                        Value::from(s.key.clone()),
                        Value::from(s.fixed_count as i64),
                        Value::from(s.fixed_expires_at),
                        Value::from(s.block_until_at),
                        Value::from(s.block_scope.as_str()),
                    ],
                ))
                .await?;
            }
            Ok(())
        })
    })
    .await
    .map_err(|e| match e {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })?;
    Ok(())
}

// Durable un-ban (W3c DELETE /admin/bans). Deletes the exact ban row AND its
// fixed-rate row in ONE transaction so a crash between the two cannot leave a
// lifted ban half-resurrected. Call while holding PERSISTENCE_LOCK so a
// concurrent periodic snapshot cannot rewrite either row; only after this
// commits may the caller clear the matching L1 buckets. A DbErr here MUST NOT
// be followed by an L1 clear — the ban stays enforced and the op is retried.
pub async fn delete_ban_rows(
    db: &sea_orm::DatabaseConnection,
    ban_key: &str,
    fixed_key: &str,
) -> Result<(), DbErr> {
    let ban_key = ban_key.to_string();
    let fixed_key = fixed_key.to_string();
    db.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "DELETE FROM rate_limit_state WHERE key = ? OR key = ?",
                vec![Value::from(ban_key), Value::from(fixed_key)],
            ))
            .await?;
            Ok(())
        })
    })
    .await
    .map_err(|e| match e {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use rux_request_gate::{AbuseLimiterConfig, LimiterDecision, RateLimitStore};
    use sea_orm::{ConnectOptions, Database, DatabaseConnection};

    const CFG: AbuseLimiterConfig = AbuseLimiterConfig {
        temp_block_attempts: 3,
        temp_block_range: 60,
        temp_block_duration: 300,
        block_retry_limit: 5,
        block_range: 3600,
        block_duration: 86400,
    };

    async fn mem_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        // Build the fixture through the REAL m000002 migration DDL, not the
        // store's duplicate ensure_table() string. The persistence tests below
        // must pin the canonical migration schema; if the two CREATE_TABLE
        // copies drifted, only ensure_table (not the migration) would be
        // exercised here, hiding the divergence. ensure_table's own drift
        // guard is the test at the bottom of this module.
        Migrator::up(&db, None).await.unwrap();
        db
    }

    #[tokio::test]
    async fn long_block_survives_l2_restart() {
        let db = mem_db().await;
        let store = InMemoryStore::default();
        let future = epoch_now() + 3600;
        store.restore(vec![BucketSnapshot {
            key: "ip".to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at: future,
            block_scope: BlockScope::Long,
        }]);

        let snaps = store.snapshot();
        assert!(snaps
            .iter()
            .any(|s| s.block_scope == BlockScope::Long && s.block_until_at > epoch_now()));

        flush(&db, snaps).await.unwrap();

        let loaded = load(&db).await;
        assert!(loaded
            .iter()
            .any(|s| s.block_scope == BlockScope::Long && s.key == "ip"));

        let store2 = InMemoryStore::default();
        store2.restore(loaded);
        match store2.abuse_check("ip", CFG).await.unwrap() {
            LimiterDecision::Blocked {
                scope: BlockScope::Long,
                ..
            } => (),
            other => panic!("expected Long block after restart, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invalid_scope_restores_as_temp() {
        let db = mem_db().await;
        let future = epoch_now() + 3600;
        db.execute_unprepared(&format!(
            "INSERT INTO rate_limit_state \
             (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
             VALUES ('k', 0, 0, {future}, 'Weird')"
        ))
        .await
        .unwrap();

        let snaps = load(&db).await;
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].block_scope, BlockScope::Temp);

        let store = InMemoryStore::default();
        store.restore(snaps);
        match store.abuse_check("k", CFG).await.unwrap() {
            LimiterDecision::Blocked {
                scope: BlockScope::Temp,
                ..
            } => (),
            other => panic!("expected Temp fail-closed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn flush_purges_expired_rows_and_keeps_active() {
        let db = mem_db().await;
        let now = epoch_now();
        let past = now - 100;
        let fut = now + 1000;
        db.execute_unprepared(&format!(
            "INSERT INTO rate_limit_state \
             (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
             VALUES ('expired', 1, {past}, {past}, 'Temp'), \
                    ('active', 1, {fut}, 0, 'Temp')"
        ))
        .await
        .unwrap();

        // Empty snapshot: the transaction's DELETE purges the expired row,
        // no upserts touch the active fixed-only window.
        flush(&db, Vec::new()).await.unwrap();

        let rows = load(&db).await;
        assert!(rows.iter().all(|r| r.key != "expired"));
        assert!(rows.iter().any(|r| r.key == "active"));
    }

    #[tokio::test]
    async fn delete_ban_rows_removes_ban_and_fixed_rows_atomically() {
        let db = mem_db().await;
        let now = epoch_now();
        let fut = now + 3600;
        db.execute_unprepared(&format!(
            "INSERT INTO rate_limit_state \
             (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
             VALUES ('quran-ban:203.0.113.5/32', 0, 0, {fut}, 'Long'), \
                    ('ratelimit:203.0.113.5/32:quran-v1', 9, {fut}, 0, 'Temp'), \
                    ('quran-ban:2001:db8::/64', 0, 0, {fut}, 'Temp')"
        ))
        .await
        .unwrap();

        delete_ban_rows(
            &db,
            "quran-ban:203.0.113.5/32",
            "ratelimit:203.0.113.5/32:quran-v1",
        )
        .await
        .unwrap();

        let rows = load(&db).await;
        assert!(!rows.iter().any(|r| r.key == "quran-ban:203.0.113.5/32"));
        assert!(!rows
            .iter()
            .any(|r| r.key == "ratelimit:203.0.113.5/32:quran-v1"));
        // A different unit is untouched (the DELETE is exact, never a wildcard).
        assert!(rows.iter().any(|r| r.key == "quran-ban:2001:db8::/64"));
    }

    #[tokio::test]
    async fn restart_after_delete_does_not_restore_the_row() {
        let db = mem_db().await;
        let fut = epoch_now() + 3600;
        db.execute_unprepared(&format!(
            "INSERT INTO rate_limit_state \
             (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
             VALUES ('quran-ban:198.51.100.7/32', 0, 0, {fut}, 'Temp')"
        ))
        .await
        .unwrap();

        delete_ban_rows(
            &db,
            "quran-ban:198.51.100.7/32",
            "ratelimit:198.51.100.7/32:quran-v1",
        )
        .await
        .unwrap();

        // Simulate a restart: the L2 table is the only source of truth a fresh
        // process loads. With the row gone, restore() cannot recreate the ban.
        let loaded = load(&db).await;
        assert!(loaded.iter().all(|r| r.key != "quran-ban:198.51.100.7/32"));
        let store = InMemoryStore::default();
        store.restore(loaded);
        assert!(store
            .ban_status("quran-ban:198.51.100.7/32")
            .await
            .unwrap()
            .is_none());
    }

    // (name, type, notnull, dflt_value, pk) per column from PRAGMA table_info.
    // Normalized so two CREATE_TABLE strings can be compared by their observable
    // SQLite schema, not their whitespace.
    async fn table_info(
        db: &DatabaseConnection,
    ) -> Vec<(String, String, i64, Option<String>, i64)> {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "PRAGMA table_info(rate_limit_state)",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|r| {
                (
                    r.try_get_by_index::<String>(1).unwrap(),
                    r.try_get_by_index::<String>(2).unwrap(),
                    r.try_get_by_index::<i64>(3).unwrap(),
                    r.try_get_by_index::<Option<String>>(4).ok().flatten(),
                    r.try_get_by_index::<i64>(5).unwrap(),
                )
            })
            .collect()
    }

    // The store's runtime ensure_table() (a fallback the admin_bans live path
    // still uses) and the m000002 migration emit the same rate_limit_state DDL
    // from two independent CREATE_TABLE constants. The persistence tests above
    // now build fixtures through the migration only; without this guard the two
    // constants could drift undetected (production boot would use ensure_table,
    // tests the migration). Pin them to identical column metadata.
    #[tokio::test]
    async fn ensure_table_ddl_matches_migration_ddl() {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let runtime_db = Database::connect(opt).await.unwrap();
        ensure_table(&runtime_db).await;

        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let migration_db = Database::connect(opt).await.unwrap();
        Migrator::up(&migration_db, None).await.unwrap();

        let runtime_info = table_info(&runtime_db).await;
        let migration_info = table_info(&migration_db).await;
        assert_eq!(
            runtime_info, migration_info,
            "ensure_table DDL and m000002 migration DDL have drifted apart; \
             rate_limit_state must expose identical columns in both paths"
        );
    }
}
