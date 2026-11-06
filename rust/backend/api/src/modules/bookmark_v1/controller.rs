//! Folders and bookmarks: one POST /sync pushes rows and pulls a delta inside a
//! single transaction. Rows merge by stamp through `eq-sync-core`
//! (contracts/sync.md), each row gets its own result, and a pull delivers changes
//! by `rev`, tombstones included.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::Utc;
use eq_sync_core::{format_stamp, parse_stamp};
use sea_orm::{DatabaseTransaction, TransactionTrait};
use serde_json::{json, Value};
use tracing::instrument;
use validator::Validate;

use crate::{
    db::sea_models::{bookmark, bookmark_folder, sync_counter},
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    quran::QuranStore,
    services::{
        auth::AuthSession,
        write_ceiling::{ceiling_error, CeilingLedger},
    },
    AppState,
};

use super::validator::{MutationKind, SyncMutation, V1BookmarkSyncPayload};

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

/// Cross-field range validation the flat derive can't express (EC-39): the
/// end must not precede the start, every endpoint must exist (per-surah ayah
/// counts via `global_of` — the flat 1..=286 bound alone lets 1:8 through),
/// and the span is capped at 50 verses like the reader's `?keys` API.
fn ensure_valid_range(quran: &QuranStore, m: &SyncMutation) -> Result<(), &'static str> {
    if m.kind != MutationKind::BookmarkUpsert {
        return Ok(());
    }
    let (Some(surah), Some(ayah)) = (m.surah, m.ayah) else {
        return Err("surah and ayah are required for bookmark.upsert");
    };
    let (end_surah, end_ayah) = (m.end_surah.unwrap_or(surah), m.end_ayah.unwrap_or(ayah));
    if (end_surah, end_ayah) < (surah, ayah) {
        return Err("range end must not precede its start");
    }
    let meta = quran.meta();
    let start = meta
        .sura(surah as u16)
        .and_then(|s| s.global_of(ayah as u16));
    let end = meta
        .sura(end_surah as u16)
        .and_then(|s| s.global_of(end_ayah as u16));
    let (Some(start), Some(end)) = (start, end) else {
        return Err("range endpoint is not a verse");
    };
    if end - start + 1 > 50 {
        return Err("range span exceeds 50 verses");
    }
    Ok(())
}

fn rejected(id: String, reason: &str) -> Value {
    json!({ "id": id, "status": "rejected", "reason": reason })
}

fn updated_at_of(raw: &Value) -> String {
    raw.get("updatedAt")
        .and_then(Value::as_str)
        .map_or_else(|| Utc::now().to_rfc3339(), str::to_string)
}

fn folder_registers(m: &SyncMutation, server_ms: i64) -> Value {
    let mut row = json!({ "id": m.id });
    if let (Some(stamp), Some(name)) = (
        m.name_stamp
            .as_deref()
            .and_then(|s| bounded_stamp(s, server_ms)),
        m.name.as_deref(),
    ) {
        row["name"] = json!({ "stamp": stamp, "value": name });
    }
    if let Some(stamp) = m
        .position_stamp
        .as_deref()
        .and_then(|s| bounded_stamp(s, server_ms))
    {
        row["position"] = json!({ "stamp": stamp, "value": m.position });
    }
    if let Some(stamp) = m
        .deleted_stamp
        .as_deref()
        .and_then(|s| bounded_stamp(s, server_ms))
    {
        row["deleted"] = json!({ "stamp": stamp, "value": m.deleted.unwrap_or(false) });
    }
    row
}

fn folder_stamps(row: &bookmark_folder::Model) -> Value {
    json!({ "name": row.name_stamp, "position": row.position_stamp, "deleted": row.deleted_stamp })
}

async fn apply_folder(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
    raw: &Value,
    server_ms: i64,
) -> DbResult<Value> {
    use bookmark_folder::FolderOutcome as Out;
    let incoming = folder_registers(m, server_ms);
    let updated_at = updated_at_of(raw);
    let verdict = match bookmark_folder::Entity::merge_write(
        tx,
        user_id,
        &m.id,
        &incoming,
        &updated_at,
    )
    .await?
    {
        Out::Applied(row) => {
            json!({ "id": m.id, "status": "applied", "stamps": folder_stamps(&row), "rev": row.rev })
        }
        Out::Superseded(row) => {
            json!({ "id": m.id, "status": "superseded", "stamps": folder_stamps(&row), "rev": row.rev })
        }
        Out::Capped => json!({ "id": m.id, "status": "capped" }),
        Out::SupersededBelowFloor => json!({ "id": m.id, "status": "superseded" }),
        Out::Incomplete => rejected(m.id.clone(), "a new folder needs a name"),
    };
    Ok(verdict)
}

async fn apply_bookmark(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
    raw: &Value,
    server_ms: i64,
) -> DbResult<Value> {
    use bookmark::BookmarkOutcome as Out;
    let Some(stamp) = m.stamp.as_deref().and_then(|s| bounded_stamp(s, server_ms)) else {
        return Ok(rejected(m.id.clone(), "bookmark.upsert needs a stamp"));
    };
    let default_id = bookmark_folder::default_folder_id(user_id);
    let surah = m.surah.unwrap_or_default();
    let ayah = m.ayah.unwrap_or_default();
    let updated_at = updated_at_of(raw);
    let write = bookmark::BookmarkWrite {
        id: &m.id,
        stamp: &stamp,
        folder_id: m.folder_id.as_deref().unwrap_or(&default_id),
        source_id: m.source_id.as_deref(),
        surah,
        ayah,
        end_surah: m.end_surah.unwrap_or(surah),
        end_ayah: m.end_ayah.unwrap_or(ayah),
        deleted: m.deleted.unwrap_or(false),
        updated_at: &updated_at,
    };
    let verdict = match bookmark::Entity::merge_write(tx, user_id, &write).await? {
        Out::Applied(row) => {
            json!({ "id": m.id, "status": "applied", "stamps": { "row": row.stamp }, "rev": row.rev })
        }
        Out::Superseded(row) => {
            json!({ "id": m.id, "status": "superseded", "stamps": { "row": row.stamp }, "rev": row.rev })
        }
        Out::Capped => json!({ "id": m.id, "status": "capped" }),
        Out::SupersededBelowFloor => json!({ "id": m.id, "status": "superseded" }),
    };
    Ok(verdict)
}

/// Parse, validate and range-check one row without touching the DB. A refused
/// row comes back as its `rejected` verdict and never reaches the ceiling.
fn precheck(quran: &QuranStore, raw: &Value) -> Result<SyncMutation, Value> {
    let row_id: String = raw
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .take(36)
        .collect();
    let Ok(m) = serde_json::from_value::<SyncMutation>(raw.clone()) else {
        return Err(rejected(row_id, "row does not parse"));
    };
    if m.validate().is_err() {
        return Err(rejected(row_id, "row fails validation"));
    }
    if let Err(reason) = ensure_valid_range(quran, &m) {
        return Err(rejected(row_id, reason));
    }
    Ok(m)
}

/// Apply one row that passed `precheck` and holds a ceiling slot. A bad row is
/// refused alone; the others still land.
async fn apply_valid(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
    raw: &Value,
    server_ms: i64,
) -> DbResult<Value> {
    match m.kind {
        MutationKind::FolderUpsert => apply_folder(tx, user_id, m, raw, server_ms).await,
        MutationKind::BookmarkUpsert => apply_bookmark(tx, user_id, m, raw, server_ms).await,
    }
}

fn folder_wire(row: &bookmark_folder::Model) -> Value {
    if row.deleted_at.is_some() {
        return json!({
            "id": row.id, "deleted": true, "rev": row.rev,
            "nameStamp": row.name_stamp, "positionStamp": row.position_stamp,
            "deletedStamp": row.deleted_stamp,
        });
    }
    json!({
        "id": row.id, "name": row.name, "isDefault": row.is_default, "position": row.position,
        "deleted": false, "rev": row.rev, "updatedAt": row.updated_at,
        "nameStamp": row.name_stamp, "positionStamp": row.position_stamp,
        "deletedStamp": row.deleted_stamp,
    })
}

fn bookmark_wire(row: &bookmark::Model) -> Value {
    // A tombstone keeps its key: the rival-key rule needs it.
    let mut wire = json!({
        "id": row.id, "folderId": row.folder_id, "surah": row.surah, "ayah": row.ayah,
        "endSurah": row.end_surah, "endAyah": row.end_ayah,
        "deleted": row.deleted_at.is_some(), "stamp": row.stamp, "rev": row.rev,
    });
    if row.deleted_at.is_none() {
        wire["sourceId"] = json!(row.source_id);
        wire["createdAt"] = json!(row.created_at);
        wire["updatedAt"] = json!(row.updated_at);
    }
    wire
}

struct PullPage {
    folders: Vec<bookmark_folder::Model>,
    bookmarks: Vec<bookmark::Model>,
    high: i64,
    more: bool,
}

async fn pull_page(
    tx: &DatabaseTransaction,
    user_id: i32,
    after: i64,
    full: bool,
) -> DbResult<PullPage> {
    let folders =
        bookmark_folder::Entity::page_since(tx, user_id, after, full, PULL_PAGE + 1).await?;
    let bookmarks = bookmark::Entity::page_since(tx, user_id, after, full, PULL_PAGE + 1).await?;
    let mut revs: Vec<i64> = folders
        .iter()
        .map(|f| f.rev)
        .chain(bookmarks.iter().map(|b| b.rev))
        .collect();
    revs.sort_unstable();
    let more = revs.len() as u64 > PULL_PAGE;
    let high = revs[..revs.len().min(PULL_PAGE as usize)]
        .last()
        .copied()
        .unwrap_or(after);
    Ok(PullPage {
        folders: folders.into_iter().filter(|f| f.rev <= high).collect(),
        bookmarks: bookmarks.into_iter().filter(|b| b.rev <= high).collect(),
        high,
        more,
    })
}

/// Snapshot guard (A7): a full pull whose live rows exceed a quota is refused
/// with 507 instead of being cut short. Quotas stop new rows, so this only fires
/// when stored rows exceed what a client can hold.
async fn snapshot_too_large(tx: &DatabaseTransaction, user_id: i32) -> DbResult<bool> {
    let bookmarks = bookmark::Entity::count_live_for_user(tx, user_id).await?;
    let folders = bookmark_folder::Entity::count_live_for_user(tx, user_id).await?;
    Ok(bookmarks > bookmark::LIVE_BOOKMARK_QUOTA || folders > bookmark_folder::LIVE_FOLDER_QUOTA)
}

fn snapshot_too_large_error() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::SnapshotTooLarge)
        .with_details("full pull exceeds the live bookmark or folder quota")
}

/// What one round decided inside its transaction.
enum Round {
    Done(Settled),
    Refused { retry_after: u64 },
    TooLarge,
}

struct Settled {
    results: Vec<Value>,
    page: PullPage,
    latest: i64,
    floor: i64,
    epoch: String,
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn sync(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1BookmarkSyncPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    if payload.expected_user_id != user.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Bookmark sync account changed"));
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

    // Push and pull share ONE transaction, so a folder delete cannot race a write
    // into it and any error rolls the whole round back. Each valid row takes one
    // slot of the daily ceiling (A7); a pull-only round takes none.
    let mut ledger = CeilingLedger::default();
    let tx = state.sea_db.begin().await.map_err_to_response()?;
    let round = async {
        bookmark_folder::Entity::ensure_default(&tx, user.id).await?;
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
                results.push(json!({ "id": m.id, "status": "capped" }));
                continue;
            }
            granted = true;
            results.push(apply_valid(&tx, user.id, &m, raw, server_ms).await?);
        }
        let full = since == 0;
        if full && snapshot_too_large(&tx, user.id).await? {
            return DbResult::Ok(Round::TooLarge);
        }
        let after = if full {
            payload.after.unwrap_or(-1)
        } else {
            since
        };
        let page = pull_page(&tx, user.id, after, full).await?;
        let domain = bookmark::BOOKMARKS_DOMAIN;
        let latest = sync_counter::current_rev(&tx, user.id, domain).await?;
        let floor = sync_counter::floor_rev(&tx, user.id, domain).await?;
        let epoch = sync_counter::sync_epoch(&tx).await?;
        DbResult::Ok(Round::Done(Settled {
            results,
            page,
            latest,
            floor,
            epoch,
        }))
    }
    .await;
    // Only a committed round keeps its writes. A refused or oversize round rolls
    // back, so nothing is half-applied behind the error.
    let settled = match round {
        Ok(Round::Done(settled)) => match tx.commit().await.map_err_to_response() {
            Ok(()) => Ok(settled),
            Err(err) => Err(err),
        },
        Ok(Round::Refused { retry_after }) => {
            let _ = tx.rollback().await;
            Err(ceiling_error(retry_after))
        }
        Ok(Round::TooLarge) => {
            let _ = tx.rollback().await;
            Err(snapshot_too_large_error())
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
            "folders": page.folders.iter().map(folder_wire).collect::<Vec<_>>(),
            "bookmarks": page.bookmarks.iter().map(bookmark_wire).collect::<Vec<_>>(),
            "defaultFolderId": bookmark_folder::default_folder_id(user.id),
            "rev": rev,
            "latestRev": latest,
            "more": page.more,
            "tombstoneFloor": floor,
        })),
    ))
}
