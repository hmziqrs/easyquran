//! Reading insights (docs/reading-model.md §4): per-user verse reads +
//! personal summary. Ingest writes the raw event and both retention-proof
//! rollups (day × surah, surah × ayah) in ONE transaction, idempotent on the
//! client-minted event id; raw rows are purged past the retention window on
//! ingest (no scheduler dependency). Every query is user-scoped. The
//! admin-facing `analytics_v1` (post-centric) is untouched.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, Statement,
    TransactionTrait,
};
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

use super::validator::{ReadingEventItem, V1EventsPayload};

/// Raw events survive 90 days, then only the rollups answer.
const RETENTION_DAYS: i64 = 90;
/// Same clock-skew clamp as reading_session_v1: a fast device cannot
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
        Err(ErrorResponse::new(ErrorCode::InvalidInput)
            .with_message("verse is not within its surah's ayah count"))
    }
}

/// Drop this user's raw events older than the retention window. Kept NULL-
/// safe: a corrupt NULL `created_at` counts as oldest and goes first.
async fn purge_expired(tx: &DatabaseTransaction, user_id: i32) -> DbResult<()> {
    let cutoff =
        lww_timestamp((Utc::now() - chrono::Duration::days(RETENTION_DAYS)).fixed_offset());
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

/// Insert one event; only a NEW event id folds into the rollups, so a
/// retried batch never double-counts.
async fn insert_event(
    tx: &DatabaseTransaction,
    user_id: i32,
    e: &ReadingEventItem,
) -> DbResult<()> {
    let created_at = lww_timestamp(clamp_at(e.at));
    let inserted = tx
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO reading_events
                   (user_id, id, session_id, surah, ayah, source_id, ms, local_day, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (user_id, id) DO NOTHING"#,
            [
                user_id.into(),
                e.id.clone().into(),
                e.session_id.clone().into(),
                e.surah.into(),
                e.ayah.into(),
                e.source_id.clone().into(),
                e.ms.into(),
                e.local_day.clone().into(),
                created_at.clone().into(),
            ],
        ))
        .await
        .map_err_to_response()?;
    if inserted.rows_affected() == 0 {
        return Ok(());
    }
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"INSERT INTO reading_daily (user_id, local_day, surah, ayahs_read, ms)
           VALUES (?, ?, ?, 1, ?)
           ON CONFLICT (user_id, local_day, surah) DO UPDATE SET
               ayahs_read = reading_daily.ayahs_read + 1,
               ms = reading_daily.ms + excluded.ms"#,
        [
            user_id.into(),
            e.local_day.clone().into(),
            e.surah.into(),
            e.ms.into(),
        ],
    ))
    .await
    .map_err_to_response()?;
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"INSERT INTO reading_ayah_totals (user_id, surah, ayah, reads, ms, last_read_at)
           VALUES (?, ?, ?, 1, ?, ?)
           ON CONFLICT (user_id, surah, ayah) DO UPDATE SET
               reads = reading_ayah_totals.reads + 1,
               ms = reading_ayah_totals.ms + excluded.ms,
               last_read_at = MAX(reading_ayah_totals.last_read_at, excluded.last_read_at)"#,
        [
            user_id.into(),
            e.surah.into(),
            e.ayah.into(),
            e.ms.into(),
            created_at.into(),
        ],
    ))
    .await
    .map_err_to_response()?;
    Ok(())
}

/// Validate-then-apply ingest round: every event's verse is checked BEFORE
/// any write (a bad item mid-batch rejects the whole round, never a partial
/// import), then raw inserts + rollup upserts + the retention purge share one
/// transaction. Returns the accepted count (replayed ids count as accepted —
/// the client may drop them).
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

/// Personal summary, every query user-scoped and read from the rollups, so
/// nothing ages out with the raw-event purge. Deterministic tie order
/// `(surah[, ayah]) ASC`.
pub async fn summary_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Value, ErrorResponse> {
    let stmt =
        |sql: &str| Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, [user_id.into()]);
    let totals = db
        .query_one(stmt(
            "SELECT CAST(COALESCE(SUM(ayahs_read), 0) AS INTEGER) AS ayahs, \
                    CAST(COALESCE(SUM(ms), 0) AS INTEGER) AS ms, \
                    COUNT(DISTINCT local_day) AS days \
             FROM reading_daily WHERE user_id = ?",
        ))
        .await
        .map_err_to_response()?
        .map(|r| (i64_col(&r, "ayahs"), i64_col(&r, "ms"), i64_col(&r, "days")))
        .unwrap_or((0, 0, 0));
    let top_surahs = db
        .query_all(stmt(
            "SELECT surah, CAST(SUM(ayahs_read) AS INTEGER) AS reads, \
                    CAST(SUM(ms) AS INTEGER) AS ms \
             FROM reading_daily WHERE user_id = ? \
             GROUP BY surah ORDER BY reads DESC, surah ASC LIMIT 5",
        ))
        .await
        .map_err_to_response()?;
    let top_ayahs = db
        .query_all(stmt(
            "SELECT surah, ayah, reads, ms FROM reading_ayah_totals \
             WHERE user_id = ? ORDER BY reads DESC, surah ASC, ayah ASC LIMIT 5",
        ))
        .await
        .map_err_to_response()?;
    let days = db
        .query_all(stmt(
            "SELECT local_day, CAST(SUM(ayahs_read) AS INTEGER) AS ayahs_read, \
                    CAST(SUM(ms) AS INTEGER) AS ms \
             FROM reading_daily WHERE user_id = ? \
             GROUP BY local_day ORDER BY local_day DESC LIMIT 30",
        ))
        .await
        .map_err_to_response()?;
    Ok(json!({
        "totals": { "ayahs": totals.0, "ms": totals.1, "days": totals.2 },
        "topSurahs": top_surahs.iter().map(|r| json!({
            "surah": i64_col(r, "surah"), "reads": i64_col(r, "reads"), "ms": i64_col(r, "ms")
        })).collect::<Vec<_>>(),
        "topAyahs": top_ayahs.iter().map(|r| json!({
            "surah": i64_col(r, "surah"), "ayah": i64_col(r, "ayah"),
            "reads": i64_col(r, "reads"), "ms": i64_col(r, "ms")
        })).collect::<Vec<_>>(),
        "days": days.iter().map(|r| json!({
            "localDay": str_col(r, "local_day"),
            "ayahsRead": i64_col(r, "ayahs_read"),
            "ms": i64_col(r, "ms"),
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
    // expectedUserId REQUIRED: a mismatch is always a stale tab / account
    // switch, rejected before any write so one account's reading never lands
    // in another's history.
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
