//! Per-user, per-domain sync counters: `rev` orders delta pulls, the floor bounds
//! how far back a client may stand. A write takes its `rev` here inside its own
//! transaction, so a pull can never see a row whose `rev` was not yet committed.

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

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
