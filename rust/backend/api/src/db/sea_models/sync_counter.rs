//! Per-user, per-domain sync counters: `rev` orders delta pulls, the floor bounds
//! how far back a client may stand. A write takes its `rev` here inside its own
//! transaction, so a pull can never see a row whose `rev` was not yet committed.

use eq_sync_core::{canonical_json, parse_stamp};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::Value;

use crate::error::{DbResult, DbResultExt};

pub async fn next_rev<C: ConnectionTrait>(conn: &C, user_id: i32, domain: &str) -> DbResult<i64> {
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO sync_counters (user_id, domain, rev) VALUES (?, ?, 1) \
             ON CONFLICT (user_id, domain) DO UPDATE SET rev = rev + 1 RETURNING rev",
            [user_id.into(), domain.into()],
        ))
        .await
        .map_err_to_response()?;
    match row {
        Some(r) => r.try_get("", "rev").map_err_to_response(),
        None => Ok(1),
    }
}

async fn read_i64<C: ConnectionTrait>(
    conn: &C,
    user_id: i32,
    domain: &str,
    column: &str,
) -> DbResult<i64> {
    let sql = format!("SELECT {column} AS v FROM sync_counters WHERE user_id = ? AND domain = ?");
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            &sql,
            [user_id.into(), domain.into()],
        ))
        .await
        .map_err_to_response()?;
    match row {
        Some(r) => r.try_get("", "v").map_err_to_response(),
        None => Ok(0),
    }
}

pub async fn current_rev<C: ConnectionTrait>(
    conn: &C,
    user_id: i32,
    domain: &str,
) -> DbResult<i64> {
    read_i64(conn, user_id, domain, "rev").await
}

pub async fn floor_rev<C: ConnectionTrait>(conn: &C, user_id: i32, domain: &str) -> DbResult<i64> {
    read_i64(conn, user_id, domain, "floor_rev").await
}

pub async fn floor_ms<C: ConnectionTrait>(conn: &C, user_id: i32, domain: &str) -> DbResult<i64> {
    read_i64(conn, user_id, domain, "floor_ms").await
}

/// A stamp rewritten to `server + 60 s` rises with the server's clock, so the same
/// bytes sent twice would look newer the second time. A register holding the same
/// value from the same device and counter is that replay: keep the stored one.
pub fn absorb_rewritten_replays(current: &Value, incoming: &mut Value) {
    let Some(fields) = incoming.as_object_mut() else {
        return;
    };
    for (name, register) in fields.iter_mut() {
        let Some(stored) = current.get(name) else {
            continue;
        };
        let origin = |r: &Value| {
            let stamp = r.get("stamp").and_then(Value::as_str)?;
            let parsed = parse_stamp(stamp)?;
            Some((parsed.counter, parsed.device))
        };
        let same_origin = origin(stored).is_some() && origin(stored) == origin(register);
        let same_value =
            stored.get("value").map(canonical_json) == register.get("value").map(canonical_json);
        if same_origin && same_value {
            *register = stored.clone();
        }
    }
}

/// `table` is a fixed name from this crate, never input.
pub async fn is_scrubbed<C: ConnectionTrait>(
    conn: &C,
    table: &str,
    user_id: i32,
    id: &str,
) -> DbResult<bool> {
    let sql = format!("SELECT scrubbed_at FROM {table} WHERE user_id = ? AND id = ?");
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            &sql,
            [user_id.into(), id.into()],
        ))
        .await
        .map_err_to_response()?;
    match row {
        Some(r) => {
            let at: Option<String> = r.try_get("", "scrubbed_at").map_err_to_response()?;
            Ok(at.is_some())
        }
        None => Ok(false),
    }
}

/// The epoch a restore bumps (`app_constants.sync_epoch`). A client that sees it
/// change does a full pull and marks every local row dirty.
pub async fn sync_epoch<C: ConnectionTrait>(conn: &C) -> DbResult<String> {
    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT value FROM app_constants WHERE key = 'sync_epoch'".to_string(),
        ))
        .await
        .map_err_to_response()?;
    match row {
        Some(r) => r.try_get("", "value").map_err_to_response(),
        None => Ok("1".to_string()),
    }
}
