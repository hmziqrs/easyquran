//! SQLite L2 durability for the in-memory rate-limit store; enforcement stays in-process (L1), this layer only periodically flushes a snapshot so blocks survive a restart. Do NOT add a DB write to the request hot path.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rux_request_gate::{BucketSnapshot, InMemoryStore};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseBackend, DbErr, Statement, Value};
use tracing::warn;

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS rate_limit_state (
    key TEXT PRIMARY KEY,
    fixed_count INTEGER NOT NULL DEFAULT 0,
    fixed_expires_at INTEGER NOT NULL DEFAULT 0,
    block_until_at INTEGER NOT NULL DEFAULT 0
)";

const FLUSH_INTERVAL_SECS: u64 = 10;

fn epoch_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn ensure_table(db: &DatabaseConnection) {
    if let Err(e) = db
        .execute(Statement::from_string(DatabaseBackend::Sqlite, CREATE_TABLE))
        .await
    {
        warn!(error = %e, "failed to create rate_limit_state table (non-fatal)");
    }
}

pub async fn load(db: &DatabaseConnection) -> Vec<BucketSnapshot> {
    match db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT key, fixed_count, fixed_expires_at, block_until_at FROM rate_limit_state",
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
                key.map(|key| BucketSnapshot {
                    key,
                    fixed_count: fixed_count.max(0) as u64,
                    fixed_expires_at,
                    block_until_at,
                })
            })
            .collect(),
        Err(e) => {
            warn!(error = %e, "failed to load rate_limit_state (starting with an empty store)");
            Vec::new()
        }
    }
}

pub fn spawn_flush_task(db: DatabaseConnection, store: Arc<InMemoryStore>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(e) = flush(&db, &store.snapshot()).await {
                warn!(error = %e, "rate-limit L2 flush failed (non-fatal)");
            }
        }
    });
}

async fn flush(db: &DatabaseConnection, snaps: &[BucketSnapshot]) -> Result<(), DbErr> {
    let now = epoch_now();
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "DELETE FROM rate_limit_state WHERE fixed_expires_at <= ? AND block_until_at <= ?",
        vec![Value::from(now), Value::from(now)],
    ))
    .await?;

    for s in snaps {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO rate_limit_state (key, fixed_count, fixed_expires_at, block_until_at) \
             VALUES (?, ?, ?, ?) \
             ON CONFLICT(key) DO UPDATE SET \
             fixed_count=excluded.fixed_count, \
             fixed_expires_at=excluded.fixed_expires_at, \
             block_until_at=excluded.block_until_at",
            vec![
                Value::from(s.key.clone()),
                Value::from(s.fixed_count as i64),
                Value::from(s.fixed_expires_at),
                Value::from(s.block_until_at),
            ],
        ))
        .await?;
    }
    Ok(())
}
