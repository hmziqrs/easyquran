//! Durable API-demand evidence for translation prewarm. The in-process pool counts demand in a
//! lock-free side table; this layer periodically snapshots it into SQLite so restarts can prewarm
//! the likely cold-start translations. Enforcement (TinyLFU admission, TTL/LRU/byte-bound
//! eviction) stays in the pool — the persisted score governs RESTART PREWARM ONLY. Do NOT add a
//! DB write to the request hot path.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, TransactionError,
    TransactionTrait, Value,
};
use tracing::warn;

/// Half-life for the decayed-demand score. Const, not env — operable tunables live behind the
/// collection/prewarm switches, not the math. 7 days balances "last week's traffic pattern still
/// informs cold start" against "ancient popularity doesn't freeze the top-N".
const HALF_LIFE_SECS: f64 = 7.0 * 24.0 * 60.0 * 60.0;

/// Cap on the health-snapshot ranking (PoolStats::top_demand / TranslationPoolHealth).
const HEALTH_TOP_N: usize = 10;

fn epoch_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `score * 0.5^(elapsed / HALF_LIFE)`. Negative elapsed (clock skew) clamps to zero so a row
/// cannot gain score by appearing to be from the future. Decay is applied AT READ so rows that
/// stopped being hit don't keep a frozen score and outrank recently-popular ones forever.
pub(crate) fn decay_score(score: f64, elapsed_secs: f64) -> f64 {
    let elapsed = elapsed_secs.max(0.0);
    score * 0.5f64.powf(elapsed / HALF_LIFE_SECS)
}

/// Ranked candidates for boot prewarm, decayed to now. Non-catalogue rows are filtered later by
/// the pool (the table outlives catalogue changes); this just reads + decays + sorts.
pub async fn load_ranked(db: &DatabaseConnection) -> Vec<(String, f64)> {
    let now = epoch_now();
    match db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT id, score, updated_at FROM translation_popularity",
            Vec::<Value>::new(),
        ))
        .await
    {
        Ok(rows) => {
            let mut ranked: Vec<(String, f64)> = rows
                .into_iter()
                .filter_map(|row| {
                    let id: String = row.try_get_by_index(0).ok()?;
                    let score: f64 = row.try_get_by_index(1).unwrap_or(0.0);
                    let updated_at: i64 = row.try_get_by_index(2).unwrap_or(now);
                    let elapsed = (now - updated_at).max(0) as f64;
                    Some((id, decay_score(score, elapsed)))
                })
                .collect();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            ranked
        }
        Err(e) => {
            warn!(
                error = %e,
                "failed to load translation_popularity (starting with no prewarm candidates)"
            );
            Vec::new()
        }
    }
}

/// One transaction for the whole snapshot: SELECT all rows, decay in Rust, then upsert the rows
/// with new hits. Returns the ranked top-N (decayed to now) for the health snapshot.
///
/// Score invariant committed at `now`:
/// `score_now = decay(old_score, now - old_updated_at) + committed_hits`, `updated_at = now`,
/// `hits_total += committed_hits`, `last_hit_at = now`. This makes one new hit on an old row add
/// only `1 + decayed_remnant`, never refreshing the old score undiminished.
///
/// `pow()`/`exp()`/`ln()` are absent from this SQLite build (libsqlite3-sys 0.30.1 has no
/// `SQLITE_ENABLE_MATH_FUNCTIONS`), so decay is computed in Rust. The table is ≤115 rows.
pub async fn flush(
    db: &DatabaseConnection,
    snapshot: &[(String, u64)],
    catalogue_ids: &HashSet<String>,
    now: i64,
) -> Result<Vec<(String, f64)>, DbErr> {
    // The transaction callback is a `for<'c>` HRTB, so it cannot borrow caller
    // data — capture owned copies (table is <=115 rows, cheap).
    let snapshot_owned: Vec<(String, u64)> = snapshot.to_vec();
    let catalogue_owned: HashSet<String> = catalogue_ids.clone();
    db.transaction::<_, Vec<(String, f64)>, DbErr>(|txn| {
        Box::pin(async move {
            let rows = txn
                .query_all(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "SELECT id, score, hits_total, updated_at FROM translation_popularity",
                    Vec::<Value>::new(),
                ))
                .await?;
            // id -> (score, hits_total, updated_at). The decay reference is always the row's own
            // updated_at, never the flush time.
            let mut stored: HashMap<String, (f64, i64, i64)> = HashMap::new();
            let mut stale: Vec<String> = Vec::new();
            for row in rows {
                let id: String = row.try_get_by_index(0).unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let score: f64 = row.try_get_by_index(1).unwrap_or(0.0);
                let hits_total: i64 = row.try_get_by_index(2).unwrap_or(0);
                let updated_at: i64 = row.try_get_by_index(3).unwrap_or(now);
                if !catalogue_owned.contains(&id) {
                    // Durable table outlives catalogue changes: drop rows whose id is no longer
                    // readable so a removed translation doesn't squat a prewarm slot forever.
                    stale.push(id);
                } else {
                    stored.insert(id, (score, hits_total, updated_at));
                }
            }
            for id in &stale {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "DELETE FROM translation_popularity WHERE id = ?",
                    vec![Value::from(id.clone())],
                ))
                .await?;
            }
            // Upsert only ids with new hits. Skipping committed_hits == 0 rewrites no untouched
            // row — decay happens at read, so rewriting it stores no information.
            for (id, committed_hits) in snapshot_owned.iter().filter(|(_, h)| *h > 0) {
                if !catalogue_owned.contains(id) {
                    continue;
                }
                let (prev_score, prev_hits_total, prev_updated_at) =
                    stored.get(id).copied().unwrap_or((0.0, 0, now));
                let elapsed = (now - prev_updated_at).max(0) as f64;
                let score_now = decay_score(prev_score, elapsed) + *committed_hits as f64;
                let hits_inc = *committed_hits as i64;
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "INSERT INTO translation_popularity \
                     (id, score, hits_total, last_hit_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?) \
                     ON CONFLICT(id) DO UPDATE SET \
                     score=excluded.score, \
                     hits_total=translation_popularity.hits_total + excluded.hits_total, \
                     last_hit_at=excluded.last_hit_at, \
                     updated_at=excluded.updated_at",
                    vec![
                        Value::from(id.clone()),
                        Value::from(score_now),
                        Value::from(hits_inc),
                        Value::from(now),
                        Value::from(now),
                    ],
                ))
                .await?;
                stored.insert(id.clone(), (score_now, prev_hits_total + hits_inc, now));
            }
            // Rank every surviving catalogue row by decayed score for the health snapshot.
            let mut ranked: Vec<(String, f64)> = stored
                .iter()
                .map(|(id, (score, _, updated_at))| {
                    let elapsed = (now - *updated_at).max(0) as f64;
                    (id.clone(), decay_score(*score, elapsed))
                })
                .collect();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            ranked.truncate(HEALTH_TOP_N);
            Ok(ranked)
        })
    })
    .await
    .map_err(|e| match e {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, Database};

    const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS translation_popularity (
        id TEXT PRIMARY KEY,
        score REAL NOT NULL DEFAULT 0,
        hits_total INTEGER NOT NULL DEFAULT 0,
        last_hit_at INTEGER,
        updated_at INTEGER NOT NULL
    )";

    async fn mem_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        db.execute_unprepared(CREATE_TABLE).await.unwrap();
        db
    }

    async fn insert_row(db: &DatabaseConnection, id: &str, score: f64, updated_at: i64) {
        db.execute_unprepared(&format!(
            "INSERT INTO translation_popularity (id, score, hits_total, last_hit_at, updated_at) \
             VALUES ('{id}', {score}, 0, NULL, {updated_at})"
        ))
        .await
        .unwrap();
    }

    async fn read_row(db: &DatabaseConnection, id: &str) -> (f64, i64, Option<i64>, i64) {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT score, hits_total, last_hit_at, updated_at \
                     FROM translation_popularity WHERE id = '{id}'"
                ),
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap();
        (
            row.try_get_by_index(0).unwrap(),
            row.try_get_by_index(1).unwrap(),
            row.try_get_by_index(2).unwrap(),
            row.try_get_by_index(3).unwrap(),
        )
    }

    fn cat(id: &str) -> HashSet<String> {
        let mut s = HashSet::new();
        s.insert(id.to_string());
        s
    }

    #[test]
    fn decay_halves_across_one_half_life() {
        let half = HALF_LIFE_SECS;
        let decayed = decay_score(1.0, half);
        assert!(
            (decayed - 0.5).abs() < 1e-9,
            "one half-life halves the score: got {decayed}"
        );
        // Two half-lives → quarter.
        let decayed = decay_score(1.0, half * 2.0);
        assert!(
            (decayed - 0.25).abs() < 1e-9,
            "two half-lives quarter: {decayed}"
        );
    }

    #[test]
    fn decay_clamps_negative_elapsed_to_zero() {
        // Clock skew (elapsed < 0) must not inflate the score.
        let decayed = decay_score(1.0, -1000.0);
        assert!(
            (decayed - 1.0).abs() < 1e-9,
            "negative elapsed is a no-op: {decayed}"
        );
    }

    #[test]
    fn thirty_day_old_score_plus_one_hit_is_not_fresh_undiminished() {
        // score_now = decay(10, 30 days) + 1. decay(10, 30 days) is small (~0.53); the one new
        // hit must NOT restore the score to 11 or anywhere near 10.
        let thirty_days = 30.0 * 24.0 * 60.0 * 60.0;
        let score_now = decay_score(10.0, thirty_days) + 1.0;
        assert!(
            score_now < 2.0,
            "old score must decay before the new hit: got {score_now}"
        );
        assert!(
            score_now > 1.0,
            "the new hit is preserved on top of the decayed remnant: got {score_now}"
        );
    }

    #[test]
    fn ranking_prefers_recent_lower_score_over_old_high_score() {
        // Old high (100, 30 days) decays to ~5.3; recent low (10, now) stays at 10. The recent
        // lower score ranks ABOVE the old high one — decay-at-read prefers recency.
        let now = epoch_now();
        let thirty_days = 30.0 * 24.0 * 60.0 * 60.0;
        let a = ("old.high".to_string(), decay_score(100.0, thirty_days));
        let b = ("recent.low".to_string(), decay_score(10.0, 0.0));
        let mut ranked = [a, b];
        ranked.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(ranked[0].0, "recent.low", "recent lower outranks old high");
        let _ = now;
    }

    #[tokio::test]
    async fn flush_writes_decayed_score_and_increments_hits_total() {
        let db = mem_db().await;
        let now = epoch_now();
        let one_week_ago = now - (HALF_LIFE_SECS as i64);
        insert_row(&db, "en.sahih", 4.0, one_week_ago).await;
        let snapshot = vec![("en.sahih".to_string(), 2u64)];
        let ranked = flush(&db, &snapshot, &cat("en.sahih"), now).await.unwrap();
        // decay(4.0, 1 half-life) = 2.0; +2 hits = 4.0.
        let (score, hits_total, last_hit_at, updated_at) = read_row(&db, "en.sahih").await;
        assert!(
            (score - 4.0).abs() < 1e-9,
            "score_now = decayed + hits: {score}"
        );
        assert_eq!(hits_total, 2, "hits_total increments by committed_hits");
        assert_eq!(last_hit_at, Some(now));
        assert_eq!(updated_at, now);
        assert_eq!(ranked[0].0, "en.sahih");
        assert!((ranked[0].1 - 4.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn flush_skips_zero_hit_ids() {
        let db = mem_db().await;
        let now = epoch_now();
        insert_row(&db, "en.sahih", 5.0, now).await;
        // committed_hits == 0 → no rewrite (decay at read stores no information).
        let snapshot = vec![("en.sahih".to_string(), 0u64)];
        flush(&db, &snapshot, &cat("en.sahih"), now).await.unwrap();
        let (score, _, _, updated_at) = read_row(&db, "en.sahih").await;
        assert!((score - 5.0).abs() < 1e-9, "untouched row not rewritten");
        assert_eq!(updated_at, now, "updated_at unchanged");
    }

    #[tokio::test]
    async fn flush_drops_non_catalogue_rows() {
        let db = mem_db().await;
        let now = epoch_now();
        insert_row(&db, "en.sahih", 5.0, now).await;
        insert_row(&db, "removed.id", 9.0, now).await;
        let snapshot = vec![("en.sahih".to_string(), 1u64)];
        flush(&db, &snapshot, &cat("en.sahih"), now).await.unwrap();
        // removed.id is gone; en.sahih kept + updated.
        let count: i64 = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM translation_popularity WHERE id = 'removed.id'",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index(0)
            .unwrap();
        assert_eq!(count, 0, "non-catalogue row dropped by flush");
        let (score, _, _, _) = read_row(&db, "en.sahih").await;
        assert!((score - 6.0).abs() < 1e-9, "catalogue row updated");
    }

    #[tokio::test]
    async fn flush_against_missing_table_returns_err() {
        // No table created → the txn's SELECT errors. A failed flush leaves the caller's demand
        // counts intact (the flush caller subtracts only on Ok), so the next tick retries.
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        let now = epoch_now();
        let snapshot = vec![("en.sahih".to_string(), 1u64)];
        let res = flush(&db, &snapshot, &cat("en.sahih"), now).await;
        assert!(res.is_err(), "missing table must surface as an error");
    }
}
