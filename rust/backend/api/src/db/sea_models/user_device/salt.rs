//! Per-account salt for continuity hints (devices.md D3). One row per user, created
//! on first use and never rotated, so the same machine hints differently under two
//! accounts and identically under one account across installs.

use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::error::{DbResult, DbResultExt};

/// Account lifetime: the sweeper skips it and erasure deletes it with the user.
const ACCOUNT_LIFETIME_EXPIRES_AT: i64 = 253_402_300_799;

/// The account's salt, creating it from `fresh` on first use. Insert-if-absent, then
/// read back, so two racing first calls agree on one value.
pub async fn salt_for<C: ConnectionTrait>(conn: &C, user_id: i32, fresh: &str) -> DbResult<String> {
    conn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO user_device_salts (user_id, salt, created_at, expires_at, retention_class) \
         VALUES (?, ?, ?, ?, 'account_lifetime') ON CONFLICT (user_id) DO NOTHING",
        [
            user_id.into(),
            fresh.into(),
            Utc::now().to_rfc3339().into(),
            ACCOUNT_LIFETIME_EXPIRES_AT.into(),
        ],
    ))
    .await
    .map_err_to_response()?;
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT salt FROM user_device_salts WHERE user_id = ?",
            [user_id.into()],
        ))
        .await
        .map_err_to_response()?;
    match row {
        Some(row) => row.try_get("", "salt").map_err_to_response(),
        None => Err(crate::error::ErrorResponse::new(
            crate::error::ErrorCode::InternalServerError,
        )),
    }
}
