//! Reading threads (docs/reading-model.md §2): one POST /sync does
//! push-then-pull inside a single transaction (whole-row LWW threads,
//! MAX-merged progress), and GET /archive pages through ended + stale threads.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{DatabaseTransaction, TransactionTrait};
use serde_json::json;
use tracing::instrument;

use crate::{
    db::sea_models::reading_session::{self, ProgressListItem, SessionListItem, SessionWrite},
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::{ValidatedJson, ValidatedQuery},
    quran::QuranStore,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{
    parse_verse_key, MutationKind, SyncMutation, V1ArchiveQuery, V1ReadingSessionSyncPayload,
};

/// How far a client clock may run ahead before the server clamps it: a fast
/// device cannot dominate LWW forever. A slow clock is NOT corrected — its
/// edits simply lose LWW.
const CLOCK_SKEW_ALLOWANCE_SECS: i64 = 60;
const ARCHIVE_DEFAULT_LIMIT: u64 = 20;

fn clamp_ts(ts: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let ceiling = Utc::now() + chrono::Duration::seconds(CLOCK_SKEW_ALLOWANCE_SECS);
    if ts.with_timezone(&Utc) > ceiling {
        ceiling.fixed_offset()
    } else {
        ts
    }
}

fn invalid(msg: &'static str) -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidInput).with_message(msg)
}

/// Kind-specific requiredness the derive can't express on the flat struct.
fn ensure_kind_fields(m: &SyncMutation) -> Result<(), ErrorResponse> {
    let missing = match m.kind {
        MutationKind::SessionUpsert => {
            m.id.is_none()
                || m.origin.is_none()
                || m.scope.is_none()
                || m.surah.is_none()
                || m.ayah.is_none()
                || m.device.is_none()
                || m.started_at.is_none()
        }
        MutationKind::SessionDelete => m.id.is_none(),
        MutationKind::ProgressUpsert => m.surah.is_none() || m.furthest_ayah.is_none(),
    };
    if missing {
        return Err(invalid("mutation is missing a field its kind requires"));
    }
    if m.kind == MutationKind::SessionUpsert && m.ended_at.is_some() != m.end_reason.is_some() {
        return Err(invalid("endedAt and endReason travel together"));
    }
    Ok(())
}

/// Verses are validated against the Quran meta's per-surah ayah counts — the
/// flat 1..=286 bound alone lets 1:8 through. Reads only; the DBs stay
/// immutable.
fn ensure_real_verse(quran: &QuranStore, surah: i32, ayah: i32) -> Result<(), ErrorResponse> {
    let real = quran
        .meta()
        .sura(surah as u16)
        .and_then(|s| s.global_of(ayah as u16))
        .is_some();
    if real {
        Ok(())
    } else {
        Err(invalid("verse is not within its surah's ayah count"))
    }
}

fn ensure_real_key(quran: &QuranStore, key: &str) -> Result<(), ErrorResponse> {
    let (surah, ayah) = parse_verse_key(key).ok_or_else(|| invalid("malformed verse key"))?;
    ensure_real_verse(quran, surah, ayah)
}

/// Every verse a mutation names must exist: the cursor, a bounded end, and
/// both ends of a `range:` scope.
fn ensure_real_verses(quran: &QuranStore, m: &SyncMutation) -> Result<(), ErrorResponse> {
    match m.kind {
        MutationKind::SessionUpsert => {
            ensure_real_verse(
                quran,
                m.surah.unwrap_or_default(),
                m.ayah.unwrap_or_default(),
            )?;
            if let Some(end) = m.bound_end.as_deref() {
                ensure_real_key(quran, end)?;
            }
            if let Some(range) = m.scope.as_deref().and_then(|s| s.strip_prefix("range:")) {
                let (a, b) = range
                    .split_once('-')
                    .ok_or_else(|| invalid("malformed range"))?;
                ensure_real_key(quran, a)?;
                ensure_real_key(quran, b)?;
            }
            Ok(())
        }
        MutationKind::ProgressUpsert => ensure_real_verse(
            quran,
            m.surah.unwrap_or_default(),
            m.furthest_ayah.unwrap_or_default(),
        ),
        MutationKind::SessionDelete => Ok(()),
    }
}

/// Apply one mutation. Returns 1 when a row changed (applied), 0 when
/// skipped by LWW, MAX, or the quota.
async fn apply_mutation(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
) -> Result<u64, ErrorResponse> {
    let updated_at = clamp_ts(m.updated_at);
    match m.kind {
        MutationKind::SessionUpsert => {
            // ensure_kind_fields guaranteed every required field.
            let write = SessionWrite {
                id: m.id.as_deref().unwrap_or_default(),
                origin: m.origin.as_deref().unwrap_or_default(),
                origin_ref: m.origin_ref.as_deref(),
                scope: m.scope.as_deref().unwrap_or_default(),
                bound_end: m.bound_end.as_deref(),
                surah: m.surah.unwrap_or_default(),
                ayah: m.ayah.unwrap_or_default(),
                local_page: m.local_page,
                ratio: m.ratio,
                source_id: m.source_id.as_deref(),
                device: m.device.as_deref().unwrap_or_default(),
                started_at: clamp_ts(m.started_at.unwrap_or(m.updated_at)),
                ended_at: m.ended_at.map(clamp_ts),
                end_reason: m.end_reason.as_deref(),
            };
            reading_session::Entity::upsert_lww(tx, user_id, &write, updated_at).await
        }
        MutationKind::SessionDelete => {
            reading_session::Entity::delete_lww(
                tx,
                user_id,
                m.id.as_deref().unwrap_or_default(),
                updated_at,
            )
            .await
        }
        MutationKind::ProgressUpsert => {
            reading_session::Entity::upsert_progress(
                tx,
                user_id,
                m.surah.unwrap_or_default(),
                m.furthest_ayah.unwrap_or_default(),
                updated_at,
            )
            .await
        }
    }
}

type Round = (u64, u64, Vec<SessionListItem>, Vec<ProgressListItem>);

/// One full round inside the caller's transaction: apply every mutation, then
/// read the snapshot from the SAME transaction — the response can never
/// observe a half-applied round.
async fn sync_round(
    tx: &DatabaseTransaction,
    user_id: i32,
    mutations: &[SyncMutation],
) -> DbResult<Round> {
    let mut applied: u64 = 0;
    let mut skipped: u64 = 0;
    for m in mutations {
        if apply_mutation(tx, user_id, m).await? > 0 {
            applied += 1;
        } else {
            skipped += 1;
        }
    }
    let sessions = reading_session::Entity::list_open(tx, user_id).await?;
    let progress = reading_session::Entity::list_progress(tx, user_id).await?;
    Ok((applied, skipped, sessions, progress))
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
    let mutations = payload.mutations.as_deref().unwrap_or(&[]);

    // Validate every mutation before applying any: a bad item mid-queue must
    // reject the whole round, not leave a half-applied push.
    for m in mutations {
        ensure_kind_fields(m)?;
        ensure_real_verses(&state.quran, m)?;
    }

    let tx = state.sea_db.begin().await.map_err_to_response()?;
    let (applied, skipped, sessions, progress) = match sync_round(&tx, user.id, mutations).await {
        Ok(round) => round,
        Err(err) => {
            tx.rollback().await.map_err_to_response()?;
            return Err(err);
        }
    };
    tx.commit().await.map_err_to_response()?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "applied": applied,
            "skipped": skipped,
            "sessions": sessions,
            "progress": progress,
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
            .map(|s| format!("{}|{}", s.updated_at, s.id))
    } else {
        None
    };
    Ok((
        StatusCode::OK,
        Json(json!({ "sessions": sessions, "next": next })),
    ))
}
