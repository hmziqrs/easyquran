//! SQLite L2 durability for the in-memory rate-limit store; enforcement stays in-process (L1), this layer only periodically flushes a snapshot so blocks survive a restart. Do NOT add a DB write to the request hot path.

// W3a escalation engine. Declared here (rather than in services/mod.rs) so the
// new module adds zero unlisted-file edits: services/escalation.rs is the allowed
// new file and this `#[path]` registration resolves to it. The engine is
// request-hot-path logic that talks to the in-memory gate store only (never the
// DB), matching this file's discipline.
#[path = "escalation.rs"]
pub mod escalation;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rux_request_gate::{BlockScope, BucketSnapshot, InMemoryStore};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseTransaction, DbErr, Statement, TransactionError,
    TransactionTrait, Value,
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

/// Retention for `rate_limit_events` rows (abuse measurement, A5). The sweeper
/// that deletes expired rows ships with A32; until then rows accumulate.
pub(crate) const LEDGER_RETENTION_SECS: i64 = 180 * 86_400;
pub(crate) const LEDGER_RETENTION_CLASS: &str = "abuse_180d";
const EVENT_BLOCK_CREATED: &str = "block_created";
const EVENT_BLOCK_EXPIRED: &str = "block_expired";

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

/// Coarse class of a rate-limit key for the block ledger. Keys are the bucket
/// keys the store already holds; the class adds no new identifier.
pub(crate) fn key_class(key: &str) -> String {
    if let Some(unit) = key.strip_prefix("quran-ban:") {
        return if unit.contains(':') {
            "ip6_unit".to_string()
        } else {
            "ip4_unit".to_string()
        };
    }
    key.split(':').next().unwrap_or("other").to_string()
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
        // Start of the window the next flush attributes block transitions to.
        // Advances only after a committed flush, so a failed flush is retried
        // over the same window and its transitions are not lost.
        let mut window_start = epoch_now();
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
                flush_window(&db, snaps, window_start).await
            };
            match result {
                Ok(flushed_at) => window_start = flushed_at,
                Err(e) => warn!(error = %e, "rate-limit L2 flush failed (non-fatal)"),
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

/// Flush with a default one-tick window, for test fixtures. The periodic task
/// uses [`flush_window`] so each block transition is attributed to one flush.
#[cfg(test)]
pub(crate) async fn flush(
    db: &sea_orm::DatabaseConnection,
    snaps: Vec<BucketSnapshot>,
) -> Result<(), DbErr> {
    flush_window(db, snaps, epoch_now() - FLUSH_INTERVAL_SECS as i64)
        .await
        .map(|_| ())
}

/// One durable flush. `window_start` is the epoch second of the previous
/// successful flush. Returns the epoch second this flush observed; the caller
/// passes it back as the next `window_start`.
///
/// Block-ledger transitions are read against the rows the PREVIOUS flush left
/// behind, inside this transaction and before any write:
/// - `block_created`: a snapshot block is active now and the stored block had
///   already lapsed (stored `block_until_at <= now`, or no row). Extending a
///   block that is still active is not a transition.
/// - `block_expired`: a stored block lapsed inside `(window_start, now]`. Each
///   expiry falls in exactly one window, so it is recorded once.
///
/// Expiries that happen while the process is down are not recorded.
pub(crate) async fn flush_window(
    db: &sea_orm::DatabaseConnection,
    snaps: Vec<BucketSnapshot>,
    window_start: i64,
) -> Result<i64, DbErr> {
    let now = epoch_now();
    // One transaction on the shared connection: the ledger reads, a single
    // DELETE of expired rows, the upserts and the ledger writes commit or roll
    // back together. Per-row autocommits would turn the 10s flush into a write
    // storm once W3a adds a long-lived row per banned identity. Expired rows
    // are purged first; active keys remain only until their fixed or block
    // expiry.
    db.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            let prior = load_prior_blocks(txn).await?;

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

            write_block_transitions(txn, &snaps, &prior, window_start, now).await
        })
    })
    .await
    .map_err(|e| match e {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })?;
    Ok(now)
}

/// A stored block as the previous flush left it.
struct PriorBlock {
    key: String,
    block_until_at: i64,
    scope: String,
}

/// One ledger row to write: a transition observed in this flush.
struct LedgerRow {
    key: String,
    scope: String,
    event: &'static str,
    issued_at: i64,
}

async fn load_prior_blocks(txn: &DatabaseTransaction) -> Result<Vec<PriorBlock>, DbErr> {
    let rows = txn
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT key, block_until_at, block_scope FROM rate_limit_state \
             WHERE block_until_at > 0",
            Vec::<Value>::new(),
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(PriorBlock {
                key: row.try_get_by_index(0)?,
                block_until_at: row.try_get_by_index(1)?,
                scope: row.try_get_by_index(2)?,
            })
        })
        .collect()
}

/// Writes one `rate_limit_events` row per block transition (see
/// [`flush_window`] for the rules). Never called per request.
async fn write_block_transitions(
    txn: &DatabaseTransaction,
    snaps: &[BucketSnapshot],
    prior: &[PriorBlock],
    window_start: i64,
    now: i64,
) -> Result<(), DbErr> {
    let prior_until: HashMap<&str, i64> = prior
        .iter()
        .map(|p| (p.key.as_str(), p.block_until_at))
        .collect();

    let mut rows: Vec<LedgerRow> = Vec::new();
    for s in snaps.iter().filter(|s| s.block_until_at > now) {
        let was = prior_until.get(s.key.as_str()).copied().unwrap_or(0);
        if was <= now {
            rows.push(LedgerRow {
                key: s.key.clone(),
                scope: s.block_scope.as_str().to_string(),
                event: EVENT_BLOCK_CREATED,
                issued_at: now,
            });
        }
    }
    for p in prior {
        if p.block_until_at > window_start && p.block_until_at <= now {
            rows.push(LedgerRow {
                key: p.key.clone(),
                scope: p.scope.clone(),
                event: EVENT_BLOCK_EXPIRED,
                issued_at: p.block_until_at,
            });
        }
    }

    for row in rows {
        let class = key_class(&row.key);
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO rate_limit_events \
             (key, key_class, scope, event, issued_at, expires_at, retention_class) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            vec![
                Value::from(row.key),
                Value::from(class),
                Value::from(row.scope),
                Value::from(row.event),
                Value::from(row.issued_at),
                Value::from(row.issued_at + LEDGER_RETENTION_SECS),
                Value::from(LEDGER_RETENTION_CLASS),
            ],
        ))
        .await?;
    }
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

// Durable clear for an identity's login-shaped buckets (A8 admin clear). Same
// contract as delete_ban_rows: call while holding PERSISTENCE_LOCK, and clear L1
// only after this commits.
pub async fn delete_limit_rows(
    db: &sea_orm::DatabaseConnection,
    keys: &[String],
) -> Result<(), DbErr> {
    if keys.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "DELETE FROM rate_limit_state WHERE key IN ({})",
        vec!["?"; keys.len()].join(", ")
    );
    let values: Vec<Value> = keys.iter().cloned().map(Value::from).collect();
    db.transaction::<_, (), DbErr>(|txn| {
        Box::pin(async move {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                values,
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

    async fn ledger_rows(db: &DatabaseConnection) -> Vec<(String, String, String, i64, i64)> {
        db.query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT key, event, scope, issued_at, expires_at FROM rate_limit_events ORDER BY id",
            Vec::<Value>::new(),
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                r.try_get_by_index::<String>(0).unwrap(),
                r.try_get_by_index::<String>(1).unwrap(),
                r.try_get_by_index::<String>(2).unwrap(),
                r.try_get_by_index::<i64>(3).unwrap(),
                r.try_get_by_index::<i64>(4).unwrap(),
            )
        })
        .collect()
    }

    fn long_block(key: &str, until: i64) -> BucketSnapshot {
        BucketSnapshot {
            key: key.to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at: until,
            block_scope: BlockScope::Long,
        }
    }

    #[test]
    fn key_class_separates_unit_families_and_limiter_keys() {
        assert_eq!(key_class("quran-ban:203.0.113.5/32"), "ip4_unit");
        assert_eq!(key_class("quran-ban:2001:db8::/64"), "ip6_unit");
        assert_eq!(key_class("ratelimit:203.0.113.5/32:quran-v1"), "ratelimit");
        assert_eq!(key_class("email:someone"), "email");
    }

    #[tokio::test]
    async fn delete_limit_rows_removes_only_the_listed_keys() {
        let db = mem_db().await;
        let until = epoch_now() + 900;
        let snap = |key: &str| BucketSnapshot {
            key: key.to_string(),
            fixed_count: 1,
            fixed_expires_at: until,
            block_until_at: 0,
            block_scope: BlockScope::Temp,
        };
        flush(
            &db,
            vec![snap("login:a@x.io"), snap("totp:1"), snap("totp:2")],
        )
        .await
        .unwrap();
        delete_limit_rows(&db, &["login:a@x.io".to_string(), "totp:1".to_string()])
            .await
            .unwrap();
        let mut left: Vec<String> = load(&db).await.into_iter().map(|r| r.key).collect();
        left.sort();
        assert_eq!(left, vec!["totp:2".to_string()]);
    }

    #[tokio::test]
    async fn block_created_writes_exactly_one_ledger_row() {
        let db = mem_db().await;
        let now = epoch_now();
        let until = now + 3600;
        let key = "quran-ban:203.0.113.40/32";

        // Transition: no stored block -> active block. One row.
        flush_window(&db, vec![long_block(key, until)], now - 10)
            .await
            .unwrap();
        let rows = ledger_rows(&db).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        let (k, event, scope, issued, expires) = &rows[0];
        assert_eq!(k, key);
        assert_eq!(event, "block_created");
        assert_eq!(scope, "Long");
        assert_eq!(*issued, now);
        assert_eq!(*expires, now + LEDGER_RETENTION_SECS);

        // Steady state across many flushes while the block holds: no new rows.
        for _ in 0..5 {
            flush_window(&db, vec![long_block(key, until)], now)
                .await
                .unwrap();
        }
        // Extending a still-active block is not a transition either.
        flush_window(&db, vec![long_block(key, until + 600)], now)
            .await
            .unwrap();
        assert_eq!(ledger_rows(&db).await.len(), 1);
    }

    #[tokio::test]
    async fn block_expiry_writes_exactly_one_ledger_row() {
        let db = mem_db().await;
        let now = epoch_now();
        let key = "quran-ban:203.0.113.41/32";
        // Stored block lapsed 5 s ago; the previous flush was 20 s ago, so the
        // expiry falls inside this window.
        db.execute_unprepared(&format!(
            "INSERT INTO rate_limit_state \
             (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
             VALUES ('{key}', 0, 0, {}, 'Temp')",
            now - 5
        ))
        .await
        .unwrap();

        flush_window(&db, Vec::new(), now - 20).await.unwrap();
        let rows = ledger_rows(&db).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].1, "block_expired");
        assert_eq!(rows[0].2, "Temp");
        assert_eq!(rows[0].3, now - 5, "expiry is stamped with its own time");

        // This flush's DELETE purges the lapsed row, and later windows no
        // longer contain its expiry, so nothing is written a second time.
        flush_window(&db, Vec::new(), now).await.unwrap();
        flush_window(&db, Vec::new(), now + 10).await.unwrap();
        assert_eq!(ledger_rows(&db).await.len(), 1);
    }

    #[tokio::test]
    async fn reblock_after_expiry_records_both_transitions() {
        let db = mem_db().await;
        let now = epoch_now();
        let key = "quran-ban:203.0.113.42/32";
        db.execute_unprepared(&format!(
            "INSERT INTO rate_limit_state \
             (key, fixed_count, fixed_expires_at, block_until_at, block_scope) \
             VALUES ('{key}', 0, 0, {}, 'Temp')",
            now - 5
        ))
        .await
        .unwrap();

        // Same window: the old block lapsed and a new one is set. Both are
        // transitions, so both rows are written.
        flush_window(&db, vec![long_block(key, now + 3600)], now - 20)
            .await
            .unwrap();
        let mut events: Vec<String> = ledger_rows(&db).await.into_iter().map(|r| r.1).collect();
        events.sort();
        assert_eq!(events, vec!["block_created", "block_expired"]);
    }

    #[tokio::test]
    async fn limiter_block_writes_one_ledger_row_end_to_end() {
        // The real L1 store sets the block; the periodic flush loop then runs
        // several times while it holds. Exactly one ledger row results.
        let db = mem_db().await;
        let store = InMemoryStore::default();
        let key = "quran-ban:203.0.113.60/32";
        store
            .set_block(key, BlockScope::Long, Duration::from_secs(3600))
            .await
            .unwrap();

        let mut window_start = epoch_now() - FLUSH_INTERVAL_SECS as i64;
        for _ in 0..4 {
            window_start = flush_window(&db, store.snapshot(), window_start)
                .await
                .unwrap();
        }
        let rows = ledger_rows(&db).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].0, key);
        assert_eq!(rows[0].1, "block_created");
        assert_eq!(rows[0].2, "Long");
    }

    #[tokio::test]
    async fn plain_fixed_window_traffic_writes_no_ledger_rows() {
        let db = mem_db().await;
        let now = epoch_now();
        let snap = BucketSnapshot {
            key: "ratelimit:203.0.113.43/32:quran-v1".to_string(),
            fixed_count: 7,
            fixed_expires_at: now + 60,
            block_until_at: 0,
            block_scope: BlockScope::Temp,
        };
        for _ in 0..3 {
            flush_window(&db, vec![snap.clone()], now - 10)
                .await
                .unwrap();
        }
        assert!(ledger_rows(&db).await.is_empty());
    }

    #[tokio::test]
    async fn flush_window_returns_the_observed_second() {
        let db = mem_db().await;
        let before = epoch_now();
        let observed = flush_window(&db, Vec::new(), before).await.unwrap();
        assert!(observed >= before);
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
