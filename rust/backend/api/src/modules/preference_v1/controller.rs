//! Preferences: one row per key, one register per row (contracts/sync.md). One POST
//! /sync pushes rows and pulls a delta inside a single transaction. There are no
//! tombstones: clearing a key writes a null value with a newer stamp.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::Utc;
use eq_sync_core::{canonical_json, format_stamp, merge_settings, parse_stamp};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, Statement, TransactionTrait};
use serde_json::{json, Value};
use tracing::instrument;

use crate::{
    db::sea_models::sync_counter,
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    services::{
        auth::AuthSession,
        write_ceiling::{ceiling_error, CeilingLedger},
    },
    AppState,
};

use super::validator::{valid_key, V1PreferenceSyncPayload, VALUE_MAX_BYTES};

const DOMAIN: &str = "preferences";
const CLOCK_SKEW_ALLOWANCE_MS: i64 = 60_000;
const PULL_PAGE: u64 = 200;
pub const MIN_PROTOCOL: u32 = 1;

fn bounded_stamp(stamp: &str, server_ms: i64) -> Option<String> {
    let parsed = parse_stamp(stamp)?;
    let ceiling = server_ms + CLOCK_SKEW_ALLOWANCE_MS;
    if parsed.ms as i64 > ceiling {
        return Some(format_stamp(ceiling as u64, parsed.counter, &parsed.device));
    }
    Some(stamp.to_string())
}

fn rejected(key: &str, reason: &str) -> Value {
    json!({ "id": key, "status": "rejected", "reason": reason })
}

/// Why a row cannot land, checked before it takes a slot of the daily ceiling.
fn precheck(raw: &Value, server_ms: i64) -> Result<(String, String, Value), Value> {
    let key = raw.get("key").and_then(Value::as_str).unwrap_or_default();
    let shown: String = key.chars().take(64).collect();
    if !valid_key(key) {
        return Err(rejected(&shown, "unknown preference key"));
    }
    let Some(stamp) = raw
        .get("stamp")
        .and_then(Value::as_str)
        .and_then(|s| bounded_stamp(s, server_ms))
    else {
        return Err(rejected(&shown, "stamp fails the format"));
    };
    let value = raw.get("value").cloned().unwrap_or(Value::Null);
    if canonical_json(&value).len() > VALUE_MAX_BYTES {
        return Err(rejected(&shown, "value too large"));
    }
    Ok((key.to_string(), stamp, value))
}

async fn load(tx: &DatabaseTransaction, user_id: i32, key: &str) -> DbResult<Option<Value>> {
    let row = tx
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT value, stamp FROM user_preferences WHERE user_id = ? AND key = ?",
            [user_id.into(), key.into()],
        ))
        .await
        .map_err_to_response()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored: String = row.try_get("", "value").map_err_to_response()?;
    let stamp: String = row.try_get("", "stamp").map_err_to_response()?;
    let value: Value = serde_json::from_str(&stored).unwrap_or(Value::Null);
    Ok(Some(json!({ "stamp": stamp, "value": value })))
}

async fn apply(
    tx: &DatabaseTransaction,
    user_id: i32,
    key: &str,
    stamp: &str,
    value: &Value,
) -> DbResult<Value> {
    let mut incoming = json!({ key: { "stamp": stamp, "value": value } });
    let current = load(tx, user_id, key).await?;
    let current_row = current.map(|register| json!({ key: register }));
    if let Some(row) = &current_row {
        sync_counter::absorb_rewritten_replays(row, &mut incoming);
    }
    let merged = match &current_row {
        Some(row) => merge_settings(row, &incoming),
        None => incoming,
    };
    let register = &merged[key];
    let held = register["stamp"].as_str().unwrap_or_default().to_string();
    let unchanged = current_row
        .as_ref()
        .is_some_and(|row| canonical_json(row) == canonical_json(&merged));
    if unchanged {
        return Ok(json!({ "id": key, "status": "superseded", "stamps": { "row": held } }));
    }
    let rev = sync_counter::next_rev(tx, user_id, DOMAIN).await?;
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO user_preferences (user_id, key, value, stamp, rev, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT (user_id, key) DO UPDATE SET value = excluded.value, \
         stamp = excluded.stamp, rev = excluded.rev, updated_at = excluded.updated_at",
        [
            user_id.into(),
            key.into(),
            register["value"].to_string().into(),
            held.clone().into(),
            rev.into(),
            Utc::now().to_rfc3339().into(),
        ],
    ))
    .await
    .map_err_to_response()?;
    Ok(json!({ "id": key, "status": "applied", "stamps": { "row": held }, "rev": rev }))
}

async fn pull(tx: &DatabaseTransaction, user_id: i32, since: i64) -> DbResult<(Vec<Value>, bool)> {
    let sql = format!(
        "SELECT key, value, stamp, rev FROM user_preferences \
         WHERE user_id = ? AND rev > ? ORDER BY rev ASC LIMIT {}",
        PULL_PAGE + 1
    );
    let rows = tx
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            &sql,
            [user_id.into(), since.into()],
        ))
        .await
        .map_err_to_response()?;
    let more = rows.len() as u64 > PULL_PAGE;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows.iter().take(PULL_PAGE as usize) {
        let key: String = row.try_get("", "key").map_err_to_response()?;
        let stored: String = row.try_get("", "value").map_err_to_response()?;
        let stamp: String = row.try_get("", "stamp").map_err_to_response()?;
        let rev: i64 = row.try_get("", "rev").map_err_to_response()?;
        let value: Value = serde_json::from_str(&stored).unwrap_or(Value::Null);
        out.push(json!({ "key": key, "value": value, "stamp": stamp, "rev": rev }));
    }
    Ok((out, more))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn sync(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1PreferenceSyncPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    if payload.expected_user_id != user.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Preference sync account changed"));
    }
    if payload.client.as_ref().map_or(MIN_PROTOCOL, |c| c.protocol) < MIN_PROTOCOL {
        return Ok((
            StatusCode::UPGRADE_REQUIRED,
            Json(json!({ "error": "protocol_too_old", "minProtocol": MIN_PROTOCOL })),
        ));
    }
    let mutations = payload.mutations.as_deref().unwrap_or(&[]);
    let since = payload.since.unwrap_or(0);
    let now = Utc::now();
    let server_ms = now.timestamp_millis();

    // Each row that can land takes one slot of the daily ceiling; a pull takes none.
    let mut ledger = CeilingLedger::default();
    let mut verdicts: Vec<Result<(String, String, Value), Value>> = Vec::new();
    let mut refused_after = None;
    for raw in mutations {
        match precheck(raw, server_ms) {
            Err(verdict) => verdicts.push(Err(verdict)),
            Ok(row) => {
                let charge = ledger.charge(&state.gate_store, user.id, now).await?;
                if charge.accepted {
                    verdicts.push(Ok(row));
                } else {
                    refused_after = Some(charge.retry_after);
                    verdicts.push(Err(json!({ "id": row.0, "status": "capped" })));
                }
            }
        }
    }
    let every_row_refused = !verdicts.is_empty()
        && verdicts.iter().all(|v| match v {
            Err(verdict) => verdict["status"] == "capped",
            Ok(_) => false,
        });
    if let (true, Some(retry_after)) = (every_row_refused, refused_after) {
        ledger.settle(&state.sea_db, user.id, now).await;
        return Err(ceiling_error(retry_after));
    }

    let tx = state.sea_db.begin().await.map_err_to_response()?;
    let round = async {
        let mut results = Vec::with_capacity(verdicts.len());
        for verdict in &verdicts {
            results.push(match verdict {
                Ok((key, stamp, value)) => apply(&tx, user.id, key, stamp, value).await?,
                Err(refused) => refused.clone(),
            });
        }
        let (rows, more) = pull(&tx, user.id, since).await?;
        let latest = sync_counter::current_rev(&tx, user.id, DOMAIN).await?;
        let epoch = sync_counter::sync_epoch(&tx).await?;
        DbResult::Ok((results, rows, more, latest, epoch))
    }
    .await;
    let (results, rows, more, latest, epoch) = match round {
        Ok(round) => round,
        Err(err) => {
            tx.rollback().await.map_err_to_response()?;
            return Err(err);
        }
    };
    tx.commit().await.map_err_to_response()?;
    ledger.settle(&state.sea_db, user.id, now).await;

    let rev = rows
        .last()
        .and_then(|row| row["rev"].as_i64())
        .unwrap_or(since);
    Ok((
        StatusCode::OK,
        Json(json!({
            "serverTime": server_ms,
            "syncEpoch": epoch,
            "results": results,
            "preferences": rows,
            "rev": rev,
            "latestRev": latest,
            "more": more,
        })),
    ))
}
