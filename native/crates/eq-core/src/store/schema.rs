//! Schema and migrations of the local app database. One schema for both file
//! kinds (signed-out `local.sqlite` and each `account-{id}.sqlite`).
//!
//! - `rows`: one row per synced register row. `row` is the JSON object (so
//!   fields a newer client added survive), `stamps` the register stamps as a
//!   JSON array, `dirty` the queue flag ("the row is the queue"), `rejected`
//!   a server refusal that stops resending until the next edit.
//! - `kv`: small values: sync positions, the clock, documents (goals, home
//!   layout, cards), the reader tab.
//!
//! A file whose `user_version` is newer than this build is left untouched and
//! sync pauses (contracts/sync.md "Versions").

use rusqlite::Connection;

use super::StoreError;

/// The schema this build writes.
pub const SCHEMA_VERSION: i64 = 1;

const V1: &str = "
CREATE TABLE IF NOT EXISTS rows (
    domain   TEXT    NOT NULL,
    id       TEXT    NOT NULL,
    row      TEXT    NOT NULL,
    stamps   TEXT    NOT NULL DEFAULT '[]',
    dirty    INTEGER NOT NULL DEFAULT 0,
    rejected INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (domain, id)
);
CREATE INDEX IF NOT EXISTS rows_dirty ON rows (domain) WHERE dirty = 1;
CREATE TABLE IF NOT EXISTS kv (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// What opening found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaState {
    Current,
    /// Written by a newer build: read nothing, write nothing, sync paused.
    Future(i64),
}

/// Brings `conn` to [`SCHEMA_VERSION`], or reports a newer file.
pub fn migrate(conn: &Connection, on_disk: bool) -> Result<SchemaState, StoreError> {
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Ok(SchemaState::Future(version));
    }
    if on_disk {
        // WAL keeps a reader from blocking the writer; NORMAL is durable at
        // each commit boundary in WAL mode except across power loss.
        let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
    }
    if version < 1 {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(V1)?;
        tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        tx.commit()?;
    }
    Ok(SchemaState::Current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_once_and_refuses_a_newer_file() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&conn, false).unwrap(), SchemaState::Current);
        assert_eq!(migrate(&conn, false).unwrap(), SchemaState::Current);
        conn.execute_batch("PRAGMA user_version = 99;").unwrap();
        assert_eq!(migrate(&conn, false).unwrap(), SchemaState::Future(99));
    }
}
