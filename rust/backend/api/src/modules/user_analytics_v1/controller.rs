//! First-party reading analytics (next-features Feature 6): opt-in per-user
//! reading events + summary. Append-only ingest writes the raw event and the
//! retention-proof daily rollup in ONE transaction; raw rows are purged past
//! the retention window on ingest (no scheduler dependency). Every query is
//! user-scoped — one user's history is never answerable for another. The
//! admin-facing `analytics_v1` (post-centric) is untouched.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
              Statement, TransactionTrait};
use serde_json::{json, Value};
use tracing::instrument;

use crate::{
    db::sea_models::bookmark_folder::lww_timestamp,
    error::{DbResult, DbResultExt, ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    quran::QuranStore,
    services::auth::AuthSession,
    AppState,
};

use super::validator::{EventKind, ReadingEventItem, V1EventsPayload};

/// Raw events survive 90 days, then only the `reading_daily` rollup answers
/// (Feature 6.3 retention).
const RETENTION_DAYS: i64 = 90;
/// Same clock-skew clamp as reading_session_v1 (EC-34): a fast device cannot
/// stamp events into the future — and a far-future `at` cannot outlive the
/// retention purge forever.
const CLOCK_SKEW_ALLOWANCE_SECS: i64 = 60;

fn clamp_at(at: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    let ceiling = Utc::now() + chrono::Duration::seconds(CLOCK_SKEW_ALLOWANCE_SECS);
    if at.with_timezone(&Utc) > ceiling {
        ceiling.fixed_offset()
    } else {
        at
    }
}

/// Verses validated against the Quran meta's per-surah ayah counts — the
/// flat 1..=286 bound alone lets 1:8 through (Feature 6.3). Reads only; the
/// DBs stay immutable.
fn ensure_real_verse(quran: &QuranStore, surah: i32, ayah: i32) -> Result<(), ErrorResponse> {
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

/// Drop this user's raw events older than the retention window. Kept NULL-
/// safe: a corrupt NULL `created_at` counts as oldest and goes first.
async fn purge_expired(tx: &DatabaseTransaction, user_id: i32) -> DbResult<()> {
    let cutoff = lww_timestamp((Utc::now() - chrono::Duration::days(RETENTION_DAYS)).fixed_offset());
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"DELETE FROM reading_events
           WHERE user_id = ?
             AND (julianday(created_at) IS NULL OR julianday(created_at) < julianday(?))"#,
        [user_id.into(), cutoff.into()],
    ))
    .await
    .map_err_to_response()?;
    Ok(())
}

/// Insert one event + fold it into the rollup. Only `ayah.read` grows the
/// rollup (`ayahs_read`, `seconds`); opens are raw-detail only.
async fn insert_event(
    tx: &DatabaseTransaction,
    user_id: i32,
    e: &ReadingEventItem,
) -> DbResult<()> {
    let created_at = lww_timestamp(clamp_at(e.at));
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"INSERT INTO reading_events
               (user_id, surah, ayah, source_id, kind, seconds, local_day, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        [
            user_id.into(),
            e.surah.into(),
            e.ayah.into(),
            e.source_id.as_deref().into(),
            e.kind.as_str().into(),
            e.seconds.map(i64::from).into(),
            e.local_day.clone().into(),
            created_at.into(),
        ],
    ))
    .await
    .map_err_to_response()?;
    if e.kind == EventKind::AyahRead {
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO reading_daily (user_id, local_day, surah, ayahs_read, seconds)
               VALUES (?, ?, ?, 1, ?)
               ON CONFLICT (user_id, local_day, surah) DO UPDATE SET
                   ayahs_read = reading_daily.ayahs_read + 1,
                   seconds = reading_daily.seconds + excluded.seconds"#,
            [user_id.into(), e.local_day.clone().into(), e.surah.into(),
             e.seconds.map(i64::from).unwrap_or(0).into()],
        ))
        .await
        .map_err_to_response()?;
    }
    Ok(())
}

/// Validate-then-apply ingest round: every event's verse is checked BEFORE
/// any write (a bad item mid-batch rejects the whole round, never a partial
/// import), then raw inserts + rollup upserts + the retention purge share one
/// transaction. Returns the accepted count. Retry duplication is a known
/// append-only limit (no idempotency key on the wire) — accepted for v1.
pub async fn ingest_events(
    db: &DatabaseConnection,
    quran: &QuranStore,
    user_id: i32,
    payload: &V1EventsPayload,
) -> Result<u64, ErrorResponse> {
    let events = payload.events.as_deref().unwrap_or(&[]);
    for e in events {
        ensure_real_verse(quran, e.surah, e.ayah)?;
    }
    let tx = db.begin().await.map_err_to_response()?;
    let round = async {
        purge_expired(&tx, user_id).await?;
        for e in events {
            insert_event(&tx, user_id, e).await?;
        }
        Ok(events.len() as u64)
    }
    .await;
    match round {
        Ok(accepted) => {
            tx.commit().await.map_err_to_response()?;
            Ok(accepted)
        }
        Err(err) => {
            tx.rollback().await.map_err_to_response()?;
            Err(err)
        }
    }
}

fn i64_col(row: &sea_orm::QueryResult, col: &str) -> i64 {
    row.try_get::<i64>("", col).unwrap_or(0)
}

fn str_col(row: &sea_orm::QueryResult, col: &str) -> String {
    row.try_get::<String>("", col).unwrap_or_default()
}

/// Most-read summary, all queries user-scoped. Top surahs + day counts read
/// the ROLLUP (retention-proof); top ayahs read raw events (ayah-level detail
/// intentionally ages out with the purge — less sensitive data retained
/// longer than needed). SQL pattern per Feature 6.2 (GROUP BY … ORDER BY
/// COUNT/SUM DESC), deterministic tie order `(surah[, ayah]) ASC`.
pub async fn summary_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Value, ErrorResponse> {
    let stmt = |sql: &str| Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, [user_id.into()]);
    let totals = db
        .query_one(stmt(
            "SELECT COUNT(*) AS events, COALESCE(SUM(seconds), 0) AS seconds \
             FROM reading_events WHERE user_id = ?",
        ))
        .await
        .map_err_to_response()?
        .map(|r| (i64_col(&r, "events"), i64_col(&r, "seconds")))
        .unwrap_or((0, 0));
    let top_surahs = db
        .query_all(stmt(
            "SELECT surah, CAST(SUM(ayahs_read) AS INTEGER) AS reads \
             FROM reading_daily WHERE user_id = ? \
             GROUP BY surah ORDER BY reads DESC, surah ASC LIMIT 5",
        ))
        .await
        .map_err_to_response()?;
    let top_ayahs = db
        .query_all(stmt(
            "SELECT surah, ayah, COUNT(*) AS reads FROM reading_events \
             WHERE user_id = ? AND kind = 'ayah.read' \
             GROUP BY surah, ayah ORDER BY reads DESC, surah ASC, ayah ASC LIMIT 5",
        ))
        .await
        .map_err_to_response()?;
    let days = db
        .query_all(stmt(
            "SELECT local_day, CAST(SUM(ayahs_read) AS INTEGER) AS ayahs_read, \
                    CAST(SUM(seconds) AS INTEGER) AS seconds \
             FROM reading_daily WHERE user_id = ? \
             GROUP BY local_day ORDER BY local_day DESC LIMIT 30",
        ))
        .await
        .map_err_to_response()?;
    Ok(json!({
        "totals": { "events": totals.0, "seconds": totals.1 },
        "topSurahs": top_surahs.iter().map(|r| json!({
            "surah": i64_col(r, "surah"), "reads": i64_col(r, "reads")
        })).collect::<Vec<_>>(),
        "topAyahs": top_ayahs.iter().map(|r| json!({
            "surah": i64_col(r, "surah"), "ayah": i64_col(r, "ayah"), "reads": i64_col(r, "reads")
        })).collect::<Vec<_>>(),
        "days": days.iter().map(|r| json!({
            "localDay": str_col(r, "local_day"),
            "ayahsRead": i64_col(r, "ayahs_read"),
            "seconds": i64_col(r, "seconds"),
        })).collect::<Vec<_>>(),
    }))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn events(
    state: State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1EventsPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    // expectedUserId REQUIRED (same §9.2 rationale as reading_session_v1): a
    // mismatch is always a stale tab / account switch, rejected before any
    // write so one account's reading never lands in another's history.
    if payload.expected_user_id != user.id {
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Reading analytics account changed"));
    }
    let accepted = ingest_events(&state.sea_db, &state.quran, user.id, &payload).await?;
    Ok((StatusCode::OK, Json(json!({ "accepted": accepted }))))
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id = auth.user.as_ref().map(|u| u.id)))]
pub async fn summary(
    state: State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    let body = summary_for_user(&state.sea_db, user.id).await?;
    Ok((StatusCode::OK, Json(body)))
}
