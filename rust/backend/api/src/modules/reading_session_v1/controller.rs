use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{DatabaseTransaction, TransactionTrait};
use serde_json::json;
use tracing::instrument;

use crate::{
    db::sea_models::{bookmark_folder::lww_timestamp, reading_session, reading_session_position,
                     reading_session_progress},
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    quran::QuranStore,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{MutationKind, SyncMutation, V1ReadingSessionSyncPayload};

/// How far a client clock may run ahead before the server clamps it (§9.3,
/// EC-34): a fast device cannot dominate LWW forever. A slow clock is NOT
/// corrected — its edits simply lose LWW; accepted limit for v1.
const CLOCK_SKEW_ALLOWANCE_SECS: i64 = 60;

fn clamp_updated_at(updated_at: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let ceiling = Utc::now() + chrono::Duration::seconds(CLOCK_SKEW_ALLOWANCE_SECS);
    if updated_at.with_timezone(&Utc) > ceiling {
        ceiling.fixed_offset()
    } else {
        updated_at
    }
}

/// Kind-specific requiredness the derive can't express on the flat wire
/// struct (§9.2 mutation fields).
fn ensure_kind_fields(m: &SyncMutation) -> Result<(), ErrorResponse> {
    let message = match m.kind {
        MutationKind::SessionUpsert if m.id.is_none() || m.name.is_none() => {
            Some("id and name are required for session.upsert")
        }
        MutationKind::SessionDelete if m.id.is_none() => {
            Some("id is required for session.delete")
        }
        MutationKind::PositionUpsert
            if m.session_id.is_none() || m.surah.is_none() || m.ayah.is_none() =>
        {
            Some("sessionId, surah, and ayah are required for position.upsert")
        }
        MutationKind::ProgressUpsert
            if m.session_id.is_none() || m.surah.is_none() || m.furthest_ayah.is_none() =>
        {
            Some("sessionId, surah, and furthestAyah are required for progress.upsert")
        }
        _ => None,
    };
    match message {
        Some(msg) => Err(ErrorResponse::new(ErrorCode::InvalidInput).with_message(msg)),
        None => Ok(()),
    }
}

/// Verses are validated against the Quran meta's per-surah ayah counts — the
/// flat 1..=286 bound alone lets 1:8 through (§9.1). Reads only; the DBs stay
/// immutable.
fn ensure_real_verse(
    quran: &QuranStore,
    surah: i32,
    ayah: i32,
) -> Result<(), ErrorResponse> {
    let real = quran
        .meta()
        .sura(surah as u16)
        .and_then(|s| s.global_of(ayah as u16))
        .is_some();
    if real {
        Ok(())
    } else {
        Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("verse is not within its surah's ayah count"))
    }
}

/// Apply one mutation, LWW/MAX-resolved. Returns 1 when a row changed
/// (applied), 0 when skipped by LWW, the ownership guard, or the quota.
async fn apply_mutation(
    tx: &DatabaseTransaction,
    user_id: i32,
    m: &SyncMutation,
) -> Result<u64, ErrorResponse> {
    let updated_at = clamp_updated_at(m.updated_at);
    match m.kind {
        MutationKind::SessionUpsert => {
            // ensure_kind_fields guaranteed Some(id)/Some(name); absent
            // kind/translation keep the column default / NULL.
            let archived = m.archived_at.map(lww_timestamp);
            reading_session::Entity::upsert_lww(
                tx,
                user_id,
                m.id.as_deref().unwrap_or_default(),
                m.name.as_deref().unwrap_or_default(),
                m.session_kind.as_deref().unwrap_or("free"),
                m.translation_id.as_deref(),
                archived.as_deref(),
                updated_at,
            )
            .await
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
        MutationKind::PositionUpsert => {
            let surah = m.surah.unwrap_or_default();
            let ayah = m.ayah.unwrap_or_default();
            reading_session_position::Entity::upsert_lww(
                tx,
                user_id,
                m.session_id.as_deref().unwrap_or_default(),
                &format!("{surah}:{ayah}"),
                surah,
                ayah,
                m.local_page,
                m.ratio,
                m.source_id.as_deref(),
                updated_at,
            )
            .await
        }
        MutationKind::ProgressUpsert => {
            reading_session_progress::Entity::upsert_max(
                tx,
                user_id,
                m.session_id.as_deref().unwrap_or_default(),
                m.surah.unwrap_or_default(),
                m.furthest_ayah.unwrap_or_default(),
                updated_at,
            )
            .await
        }
    }
}

/// One full round inside the caller's transaction: apply every mutation
/// (LWW/MAX-resolved), then read the snapshot from the SAME transaction —
/// the response can never observe a half-applied round. Snapshot is
/// unbounded by design (same accepted trade-off as bookmarks, EC-41); the
/// pagination seam is exactly these three list_for_user reads.
async fn sync_round(
    tx: &DatabaseTransaction,
    user_id: i32,
    mutations: &[SyncMutation],
) -> DbResult<(
    u64,
    u64,
    Vec<reading_session::SessionListItem>,
    Vec<reading_session_position::PositionListItem>,
    Vec<reading_session_progress::ProgressListItem>,
)> {
    let mut applied: u64 = 0;
    let mut skipped: u64 = 0;
    for m in mutations {
        if apply_mutation(tx, user_id, m).await? > 0 {
            applied += 1;
        } else {
            skipped += 1;
        }
    }
    let sessions: Vec<reading_session::SessionListItem> =
        reading_session::Entity::list_for_user(tx, user_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
    let positions: Vec<reading_session_position::PositionListItem> =
        reading_session_position::Entity::list_for_user(tx, user_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
    let progress: Vec<reading_session_progress::ProgressListItem> =
        reading_session_progress::Entity::list_for_user(tx, user_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
    Ok((applied, skipped, sessions, positions, progress))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn sync(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1ReadingSessionSyncPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    // expectedUserId is REQUIRED here (§9.2): no legacy clients exist for
    // this module, so a mismatch is always an account change, never a stale
    // tab — reject before any read or write.
    if payload.expected_user_id != user.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Reading session sync account changed"));
    }
    let mutations = payload.mutations.as_deref().unwrap_or(&[]);

    // Validate every mutation before applying any: a bad item mid-queue must
    // reject the whole round, not leave a half-applied push.
    for m in mutations {
        ensure_kind_fields(m)?;
        match m.kind {
            MutationKind::PositionUpsert => {
                ensure_real_verse(&state.quran, m.surah.unwrap_or_default(), m.ayah.unwrap_or_default())?;
            }
            MutationKind::ProgressUpsert => {
                ensure_real_verse(&state.quran, m.surah.unwrap_or_default(), m.furthest_ayah.unwrap_or_default())?;
            }
            MutationKind::SessionUpsert | MutationKind::SessionDelete => {}
        }
    }

    // Push and pull share ONE transaction: any error rolls the whole round
    // back — no partial state survives.
    let tx = state.sea_db.begin().await.map_err_to_response()?;
    let (applied, skipped, sessions, positions, progress) =
        match sync_round(&tx, user.id, mutations).await {
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
            "positions": positions,
            "progress": progress,
        })),
    ))
}
