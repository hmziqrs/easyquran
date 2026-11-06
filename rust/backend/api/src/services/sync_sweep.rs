//! Tombstone scrub. A deleted row keeps its content for 24 h so Undo works, then
//! its content becomes neutral values (the columns are NOT NULL) and it ignores any
//! further write. Pulls already deliver tombstones as stamps and identity only.
//! Removing tombstones and raising the floor belongs to the retention sweeper.

use chrono::Utc;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, TransactionTrait,
};

const SCRUB_AFTER_HOURS: i64 = 24;

const SCRUBS: [&str; 3] = [
    "UPDATE reading_sessions SET origin = 'link', origin_ref = NULL, scope = 'surah:1', \
     bound_end = NULL, surah = 1, ayah = 1, local_page = NULL, ratio = NULL, source_id = NULL, \
     device = 'phone', device_id = NULL, started_at = '1970-01-01T00:00:00.000Z', ended_at = NULL, \
     end_reason = NULL, scrubbed_at = ? \
     WHERE state = 'deleted' AND scrubbed_at IS NULL AND julianday(deleted_at) < julianday(?)",
    // A bookmark tombstone keeps its key: the rival-key rule needs it.
    "UPDATE bookmarks SET source_id = NULL, scrubbed_at = ? \
     WHERE deleted_at IS NOT NULL AND scrubbed_at IS NULL AND julianday(deleted_at) < julianday(?)",
    "UPDATE bookmark_folders SET name = 'deleted', position = NULL, scrubbed_at = ? \
     WHERE deleted_at IS NOT NULL AND scrubbed_at IS NULL AND julianday(deleted_at) < julianday(?)",
];

/// Returns how many rows were scrubbed.
pub async fn scrub_tombstones(db: &DatabaseConnection) -> Result<u64, DbErr> {
    scrub_older_than(db, Utc::now() - chrono::Duration::hours(SCRUB_AFTER_HOURS)).await
}

pub async fn scrub_older_than(
    db: &DatabaseConnection,
    cutoff: chrono::DateTime<Utc>,
) -> Result<u64, DbErr> {
    let now = Utc::now().to_rfc3339();
    let cutoff = cutoff.to_rfc3339();
    let txn = db.begin().await?;
    let mut total = 0;
    for sql in SCRUBS {
        let res = txn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                [now.clone().into(), cutoff.clone().into()],
            ))
            .await?;
        total += res.rows_affected();
    }
    txn.commit().await?;
    Ok(total)
}
