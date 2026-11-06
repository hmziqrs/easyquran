//! Reading threads: one POST /sync pushes rows and pulls a delta inside a single
//! transaction. Rows merge register by register through `eq-sync-core`
//! (contracts/sync.md); each row gets its own result, and a pull delivers changes
//! by `rev`, tombstones included. GET /archive pages through ended + stale threads.

use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::Utc;
use eq_sync_core::{format_stamp, parse_stamp, round_ratio};
use sea_orm::{DatabaseTransaction, TransactionTrait};
use serde_json::{json, Value};
use tracing::instrument;
use validator::Validate;

use crate::{
    db::sea_models::{
        reading_session::{
            self, ProgressListItem, SessionListItem, ThreadOutcome, SESSIONS_DOMAIN,
        },
        sync_counter, user_device,
    },
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::{ValidatedJson, ValidatedQuery},
    quran::QuranStore,
    services::{
        auth::AuthSession,
        write_ceiling::{ceiling_error, CeilingLedger},
    },
    AppState,
};

use super::validator::{
    parse_verse_key, MutationKind, SyncMutation, V1ArchiveQuery, V1ReadingSessionSyncPayload,
};

/// How far a stamp may run ahead of the server. Past this it is rewritten, keeping
/// its device id, and the client adopts it from the returned row.
const CLOCK_SKEW_ALLOWANCE_MS: i64 = 60_000;
const ARCHIVE_DEFAULT_LIMIT: u64 = 20;
const PULL_PAGE: u64 = 200;
/// Below this a client gets 426 and no row is applied.
pub const MIN_PROTOCOL: u32 = 1;

fn invalid(msg: &'static str) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidInput).with_message(msg)
}

/// A stamp too far ahead is rewritten to server + 60 s. It can never dominate.
fn bounded_stamp(stamp: &str, server_ms: i64) -> Option<String> {
    let parsed = parse_stamp(stamp)?;
    let ceiling = server_ms + CLOCK_SKEW_ALLOWANCE_MS;
    if parsed.ms as i64 > ceiling {
        return Some(format_stamp(ceiling as u64, parsed.counter, &parsed.device));
    }
    Some(stamp.to_string())
}

/// Each field group is whole or absent.
fn ensure_groups(m: &SyncMutation) -> Result<(), &'static str> {
    if m.identity_stamp.is_some()
        && (m.origin.is_none() || m.scope.is_none() || m.started_at.is_none())
    {
        return Err("identity register is missing a field");
    }
    if m.cursor_stamp.is_some() && (m.surah.is_none() || m.ayah.is_none() || m.device.is_none()) {
        return Err("cursor register is missing a field");
    }
    if m.lifecycle_stamp.is_some() && m.state.is_none() {
        return Err("lifecycle register is missing its state");
    }
    if m.state.as_deref() == Some("ended") && (m.ended_at.is_none() || m.end_reason.is_none()) {
        return Err("an ended thread needs endedAt and endReason");
    }
    match m.kind {
        MutationKind::SessionUpsert if m.id.is_none() => Err("session.upsert needs an id"),
        MutationKind::ProgressUpsert if m.surah.is_none() || m.furthest_ayah.is_none() => {
            Err("progress.upsert needs surah and furthestAyah")
        }
        _ => Ok(()),
    }
}

/// Verses are validated against the Quran meta's per-surah ayah counts — the
/// flat 1..=286 bound alone lets 1:8 through. Reads only; the DBs stay immutable.
fn ensure_real_verse(quran: &QuranStore, surah: i32, ayah: i32) -> Result<(), &'static str> {
    let real = quran
        .meta()
        .sura(surah as u16)
        .and_then(|s| s.global_of(ayah as u16))
        .is_some();
    if real {
        Ok(())
    } else {
        Err("verse is not within its surah's ayah count")
    }
}

fn ensure_real_key(quran: &QuranStore, key: &str) -> Result<(), &'static str> {
    let (surah, ayah) = parse_verse_key(key).ok_or("malformed verse key")?;
    ensure_real_verse(quran, surah, ayah)
}

fn ensure_real_verses(quran: &QuranStore, m: &SyncMutation) -> Result<(), &'static str> {
    if m.kind == MutationKind::ProgressUpsert {
        return ensure_real_verse(
            quran,
            m.surah.unwrap_or_default(),
            m.furthest_ayah.unwrap_or_default(),
        );
    }
    if m.cursor_stamp.is_some() {
        ensure_real_verse(
            quran,
            m.surah.unwrap_or_default(),
            m.ayah.unwrap_or_default(),
        )?;
    }
    if m.identity_stamp.is_some() {
        if let Some(end) = m.bound_end.as_deref() {
            ensure_real_key(quran, end)?;
        }
        if let Some(range) = m.scope.as_deref().and_then(|s| s.strip_prefix("range:")) {
            let (a, b) = range.split_once('-').ok_or("malformed range")?;
            ensure_real_key(quran, a)?;
            ensure_real_key(quran, b)?;
        }
    }
    Ok(())
}

/// Only the groups this write carries become registers. Times stay as the client
/// sent them so every replica compares the same bytes.
fn registers_of(m: &SyncMutation, raw: &Value, server_ms: i64) -> Value {
    let id = m.id.clone().unwrap_or_default();
    let mut row = json!({ "id": id });
    if let Some(stamp) = m
        .identity_stamp
        .as_deref()
        .and_then(|s| bounded_stamp(s, server_ms))
    {
        row["identity"] = json!({ "stamp": stamp, "value": {
            "origin": m.origin, "originRef": m.origin_ref, "scope": m.scope,
            "boundEnd": m.bound_end, "startedAt": raw["startedAt"],
        }});
    }
    if let Some(stamp) = m
        .cursor_stamp
        .as_deref()
        .and_then(|s| bounded_stamp(s, server_ms))
    {
        row["cursor"] = json!({ "stamp": stamp, "value": {
            "surah": m.surah, "ayah": m.ayah, "localPage": m.local_page,
            "ratio": m.ratio.map(round_ratio), "sourceId": m.source_id, "device": m.device,
            "deviceId": m.device_id,
        }});
    }
    if let Some(stamp) = m
        .lifecycle_stamp
        .as_deref()
        .and_then(|s| bounded_stamp(s, server_ms))
    {
        row["lifecycle"] = json!({ "stamp": stamp, "value": {
            "state": m.state, "endedAt": raw.get("endedAt").cloned().unwrap_or(Value::Null),
            "endReason": m.end_reason,
        }});
    }
    row
}

fn row_id_of(raw: &Value) -> String {
    if let Some(id) = raw.get("id").and_then(Value::as_str) {
        return id.chars().take(36).collect();
    }
    match raw.get("surah").and_then(Value::as_i64) {
        Some(surah) => format!("progress:{surah}"),
        None => String::new(),
    }
}

fn rejected(id: String, reason: &str) -> Value {
    json!({ "id": id, "status": "rejected", "reason": reason })
}

/// Parse, validate and check the verses of one row without touching the DB. A
/// refused row comes back as its `rejected` verdict and never reaches the ceiling.
fn precheck(quran: &QuranStore, raw: &Value) -> Result<SyncMutation, Value> {
    let row_id = row_id_of(raw);
    let Ok(m) = serde_json::from_value::<SyncMutation>(raw.clone()) else {
        return Err(rejected(row_id, "row does not parse"));
    };
    if m.validate().is_err() {
        return Err(rejected(row_id, "row fails validation"));
    }
    if let Err(reason) = ensure_groups(&m).and_then(|()| ensure_real_verses(quran, &m)) {
        return Err(rejected(row_id, reason));
    }
    Ok(m)
}

/// Apply one row that passed `precheck` and holds a ceiling slot.
async fn apply_valid(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
    raw: &Value,
    server_ms: i64,
) -> DbResult<Value> {
    let row_id = row_id_of(raw);
    let updated_at = raw
        .get("updatedAt")
        .and_then(Value::as_str)
        .map_or_else(|| Utc::now().to_rfc3339(), str::to_string);

    if m.kind == MutationKind::ProgressUpsert {
        let changed = reading_session::Entity::upsert_progress(
            tx,
            user_id,
            m.surah.unwrap_or_default(),
            m.furthest_ayah.unwrap_or_default(),
            &updated_at,
        )
        .await?;
        let status = if changed { "applied" } else { "superseded" };
        return Ok(json!({ "id": row_id, "status": status }));
    }

    let id = m.id.as_deref().unwrap_or_default();
    let incoming = registers_of(m, raw, server_ms);
    let verdict = match reading_session::Entity::merge_write(
        tx,
        user_id,
        id,
        &incoming,
        &updated_at,
    )
    .await?
    {
        ThreadOutcome::Applied(row) => {
            json!({ "id": id, "status": "applied", "stamps": row.stamps(), "rev": row.rev })
        }
        ThreadOutcome::Superseded(row) => {
            json!({ "id": id, "status": "superseded", "stamps": row.stamps(), "rev": row.rev })
        }
        ThreadOutcome::Capped => json!({ "id": id, "status": "capped" }),
        ThreadOutcome::SupersededBelowFloor => json!({ "id": id, "status": "superseded" }),
        ThreadOutcome::Incomplete => rejected(id.to_string(), "a new thread needs all registers"),
    };
    Ok(verdict)
}

/// One pull page by `rev`, threads and progress merged, plus the page's high mark.
struct PullPage {
    sessions: Vec<SessionListItem>,
    progress: Vec<ProgressListItem>,
    high: i64,
    more: bool,
}

async fn pull_page(
    tx: &DatabaseTransaction,
    user_id: i32,
    after: i64,
    full: bool,
) -> DbResult<PullPage> {
    let sessions =
        reading_session::Entity::page_since(tx, user_id, after, full, PULL_PAGE + 1).await?;
    let progress =
        reading_session::Entity::progress_since(tx, user_id, after, PULL_PAGE + 1).await?;
    let mut revs: Vec<i64> = sessions
        .iter()
        .map(|s| s.rev)
        .chain(progress.iter().map(|p| p.rev))
        .collect();
    revs.sort_unstable();
    let more = revs.len() as u64 > PULL_PAGE;
    let high = revs[..revs.len().min(PULL_PAGE as usize)]
        .last()
        .copied()
        .unwrap_or(after);
    let keep = |rev: i64| rev <= high;
    Ok(PullPage {
        sessions: sessions.into_iter().filter(|s| keep(s.rev)).collect(),
        progress: progress.into_iter().filter(|p| keep(p.rev)).collect(),
        high,
        more,
    })
}

/// Names for the devices a pull's threads point at (devices.md D5). Read inside the
/// round's transaction, so a name and its thread come from the same snapshot.
async fn page_device_names(
    tx: &DatabaseTransaction,
    user_id: i32,
    page: &PullPage,
) -> DbResult<HashMap<String, String>> {
    let ids: Vec<String> = page
        .sessions
        .iter()
        .filter_map(|s| s.device_id.clone())
        .collect();
    user_device::Entity::names_for(tx, user_id, &ids).await
}

/// Adds `deviceName` to a pulled thread row: the name its device resolves to, or null
/// for an unknown, revoked or unnamed device. A tombstone has no device, so it is left
/// as it is.
fn with_device_name(mut row: Value, names: &HashMap<String, String>) -> Value {
    if row.get("state").and_then(Value::as_str) == Some("deleted") {
        return row;
    }
    let name = row
        .get("deviceId")
        .and_then(Value::as_str)
        .and_then(|id| names.get(id))
        .cloned();
    if let Some(obj) = row.as_object_mut() {
        obj.insert("deviceName".to_string(), json!(name));
    }
    row
}

/// What one round decided inside its transaction.
enum Round {
    Done(Settled),
    Refused { retry_after: u64 },
}

struct Settled {
    results: Vec<Value>,
    page: PullPage,
    names: HashMap<String, String>,
    latest: i64,
    floor: i64,
    epoch: String,
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn sync(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1ReadingSessionSyncPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    if payload.expected_user_id != user.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Reading session sync account changed"));
    }
    if payload.client.as_ref().map_or(MIN_PROTOCOL, |c| c.protocol) < MIN_PROTOCOL {
        return Ok((
            StatusCode::UPGRADE_REQUIRED,
            Json(json!({ "error": "protocol_too_old", "minProtocol": MIN_PROTOCOL })),
        ));
    }
    let mutations = payload.mutations.as_deref().unwrap_or(&[]);
    let since = payload.since.unwrap_or(0);
    let server_ms = Utc::now().timestamp_millis();
    let now = Utc::now();

    // Each valid row takes one slot of the daily ceiling (A7). A pull-only round
    // takes none, so pulls keep working after the day's writes are spent.
    let mut ledger = CeilingLedger::default();
    let tx = state.sea_db.begin().await.map_err_to_response()?;
    let round = async {
        let mut results = Vec::with_capacity(mutations.len());
        let mut granted = false;
        for raw in mutations {
            let m = match precheck(&state.quran, raw) {
                Ok(m) => m,
                Err(verdict) => {
                    results.push(verdict);
                    continue;
                }
            };
            let charge = ledger.charge(&state.gate_store, user.id, now).await?;
            if !charge.accepted {
                // Nothing fit this round: hold the pushes (429). Once a row got
                // a slot, the rest wait as `capped` and stay dirty.
                if !granted {
                    return DbResult::Ok(Round::Refused {
                        retry_after: charge.retry_after,
                    });
                }
                results.push(json!({ "id": row_id_of(raw), "status": "capped" }));
                continue;
            }
            granted = true;
            results.push(apply_valid(&tx, user.id, &m, raw, server_ms).await?);
        }
        let full = since == 0;
        let after = if full {
            payload.after.unwrap_or(-1)
        } else {
            since
        };
        let page = pull_page(&tx, user.id, after, full).await?;
        let names = page_device_names(&tx, user.id, &page).await?;
        let latest = sync_counter::current_rev(&tx, user.id, SESSIONS_DOMAIN).await?;
        let floor = sync_counter::floor_rev(&tx, user.id, SESSIONS_DOMAIN).await?;
        let epoch = sync_counter::sync_epoch(&tx).await?;
        DbResult::Ok(Round::Done(Settled {
            results,
            page,
            names,
            latest,
            floor,
            epoch,
        }))
    }
    .await;
    // A refused round rolls back, so nothing is half-applied behind the 429.
    let settled = match round {
        Ok(Round::Done(settled)) => match tx.commit().await.map_err_to_response() {
            Ok(()) => Ok(settled),
            Err(err) => Err(err),
        },
        Ok(Round::Refused { retry_after }) => {
            let _ = tx.rollback().await;
            Err(ceiling_error(retry_after))
        }
        Err(err) => {
            let _ = tx.rollback().await;
            Err(err)
        }
    };
    // The ledger row is written only now, after the transaction has ended.
    ledger.settle(&state.sea_db, user.id, now).await;
    let Settled {
        results,
        page,
        names,
        latest,
        floor,
        epoch,
    } = settled?;

    let count = |status: &str| results.iter().filter(|r| r["status"] == status).count();
    let rev = if page.more || since > 0 {
        page.high
    } else {
        latest
    };
    let sessions: Vec<Value> = page
        .sessions
        .iter()
        .map(|s| with_device_name(s.to_wire(), &names))
        .collect();
    Ok((
        StatusCode::OK,
        Json(json!({
            "serverTime": server_ms,
            "syncEpoch": epoch,
            "results": results,
            "applied": count("applied"),
            "superseded": count("superseded"),
            "rejected": count("rejected"),
            "capped": count("capped"),
            "sessions": sessions,
            "progress": page.progress,
            "rev": rev,
            "latestRev": latest,
            "more": page.more,
            "tombstoneFloor": floor,
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, query), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn archive(
    state: State<AppState>,
    auth: AuthSession,
    query: ValidatedQuery<V1ArchiveQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    let limit = query.limit.unwrap_or(ARCHIVE_DEFAULT_LIMIT);
    let before = match query.before.as_deref() {
        Some(cursor) => Some(
            cursor
                .split_once('|')
                .ok_or_else(|| invalid("malformed cursor"))?,
        ),
        None => None,
    };
    // One extra row tells us whether another page exists.
    let mut sessions =
        reading_session::Entity::list_archive(&state.sea_db, user.id, before, limit + 1).await?;
    let next = if sessions.len() as u64 > limit {
        sessions.truncate(limit as usize);
        sessions
            .last()
            .map(|s| format!("{}|{}", s.cursor_stamp.clone().unwrap_or_default(), s.id))
    } else {
        None
    };
    Ok((
        StatusCode::OK,
        Json(json!({ "sessions": sessions, "next": next })),
    ))
}
