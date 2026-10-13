//! `reading_sessions` + `reading_progress` (docs/reading-model.md §2). A
//! session is one continuous reading thread with its cursor inline; one row
//! per thread, last-writer-wins on `updated_at`. Progress is account-level and
//! MAX-merged so it never regresses across devices.

use crate::error::{DbResult, DbResultExt};
use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseBackend, DatabaseTransaction, QueryResult, Statement};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::bookmark_folder::lww_timestamp;

/// Per-user live-row quota. A NEW thread past the cap is refused (surfaced
/// via the round's `skipped` count); updating an existing id is always fine.
const LIVE_SESSION_QUOTA: u64 = 10_000;
/// Open threads untouched this long turn stale (derived, never stored).
pub const STALE_AFTER_DAYS: i64 = 30;
/// Open threads returned by one sync snapshot, newest first.
const OPEN_SNAPSHOT_LIMIT: u64 = 20;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_sessions")]
pub struct Model {
    /// Composite PK (user_id, id): the same client-minted id under two
    /// accounts is two threads, never a collision.
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// index | search | bookmark | card | snippet | link
    pub origin: String,
    /// Origin detail, e.g. the card id that opened the thread.
    pub origin_ref: Option<String>,
    /// What the thread reads: `surah:N`, `page:N`, `juz:N`, `hizb:N`,
    /// `snippet:slug`, `range:S:A-S:A`.
    pub scope: String,
    /// `S:A` end of a bounded scope; reaching it completes the thread.
    pub bound_end: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    /// phone | tablet | desktop — the handoff label.
    pub device: String,
    pub started_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub ended_at: Option<DateTimeWithTimeZone>,
    /// completed | restarted | dismissed
    pub end_reason: Option<String>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Wire item (camelCase); user_id and the tombstone never leave the server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
    pub id: String,
    pub origin: String,
    pub origin_ref: Option<String>,
    pub scope: String,
    pub bound_end: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    pub device: String,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
    pub end_reason: Option<String>,
}

impl SessionListItem {
    fn from_row(row: &QueryResult) -> Result<Self, DbErr> {
        Ok(Self {
            id: row.try_get("", "id")?,
            origin: row.try_get("", "origin")?,
            origin_ref: row.try_get("", "origin_ref")?,
            scope: row.try_get("", "scope")?,
            bound_end: row.try_get("", "bound_end")?,
            surah: row.try_get("", "surah")?,
            ayah: row.try_get("", "ayah")?,
            local_page: row.try_get("", "local_page")?,
            ratio: row.try_get("", "ratio")?,
            source_id: row.try_get("", "source_id")?,
            device: row.try_get("", "device")?,
            started_at: row.try_get("", "started_at")?,
            updated_at: row.try_get("", "updated_at")?,
            ended_at: row.try_get("", "ended_at")?,
            end_reason: row.try_get("", "end_reason")?,
        })
    }
}

/// One thread write (whole row, LWW).
pub struct SessionWrite<'a> {
    pub id: &'a str,
    pub origin: &'a str,
    pub origin_ref: Option<&'a str>,
    pub scope: &'a str,
    pub bound_end: Option<&'a str>,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<&'a str>,
    pub device: &'a str,
    pub started_at: DateTimeWithTimeZone,
    pub ended_at: Option<DateTimeWithTimeZone>,
    pub end_reason: Option<&'a str>,
}

/// Per-surah furthest ayah (wire camelCase).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressListItem {
    pub surah: i32,
    pub furthest_ayah: i32,
    pub updated_at: String,
}

const SELECT_COLUMNS: &str = "id, origin, origin_ref, scope, bound_end, surah, ayah, local_page, \
     ratio, source_id, device, started_at, updated_at, ended_at, end_reason";

fn stale_cutoff() -> String {
    lww_timestamp((Utc::now() - chrono::Duration::days(STALE_AFTER_DAYS)).fixed_offset())
}

impl Entity {
    /// Last-writer-wins whole-row upsert. DO UPDATE only on a strictly newer
    /// timestamp (julianday ranks by instant; a corrupt NULL counts as
    /// oldest). A newer upsert clears the tombstone and may clear `ended_at`
    /// (reopening a thread from history). Returns 1 = applied, 0 = skipped by
    /// LWW or quota.
    #[instrument(skip(conn, w), fields(user_id = user_id, session_id = w.id))]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        w: &SessionWrite<'_>,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let exists = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(w.id))
            .count(conn)
            .await
            .map_err_to_response()?;
        if exists == 0 && Self::count_live_for_user(conn, user_id).await? >= LIVE_SESSION_QUOTA {
            return Ok(0);
        }
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO reading_sessions (user_id, id, origin, origin_ref, scope, bound_end,
                       surah, ayah, local_page, ratio, source_id, device,
                       started_at, updated_at, ended_at, end_reason)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT (user_id, id) DO UPDATE SET
                       origin = excluded.origin,
                       origin_ref = excluded.origin_ref,
                       scope = excluded.scope,
                       bound_end = excluded.bound_end,
                       surah = excluded.surah,
                       ayah = excluded.ayah,
                       local_page = excluded.local_page,
                       ratio = excluded.ratio,
                       source_id = excluded.source_id,
                       device = excluded.device,
                       started_at = excluded.started_at,
                       updated_at = excluded.updated_at,
                       ended_at = excluded.ended_at,
                       end_reason = excluded.end_reason,
                       deleted_at = NULL
                   WHERE julianday(reading_sessions.updated_at) IS NULL
                      OR julianday(excluded.updated_at) > julianday(reading_sessions.updated_at)"#,
                [
                    user_id.into(),
                    w.id.into(),
                    w.origin.into(),
                    w.origin_ref.into(),
                    w.scope.into(),
                    w.bound_end.into(),
                    w.surah.into(),
                    w.ayah.into(),
                    w.local_page.into(),
                    w.ratio.into(),
                    w.source_id.into(),
                    w.device.into(),
                    lww_timestamp(w.started_at).into(),
                    lww_timestamp(updated_at).into(),
                    w.ended_at.map(lww_timestamp).into(),
                    w.end_reason.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins delete = TOMBSTONE (removal from history). Refused
    /// when the stored row is strictly newer. 1 = applied, 0 = skipped.
    #[instrument(skip(conn), fields(user_id = user_id, session_id = id))]
    pub async fn delete_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"UPDATE reading_sessions SET deleted_at = ?, updated_at = ?
                   WHERE user_id = ? AND id = ?
                     AND (julianday(updated_at) IS NULL
                          OR julianday(updated_at) <= julianday(?))"#,
                [
                    ts.clone().into(),
                    ts.clone().into(),
                    user_id.into(),
                    id.into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Open threads: live, not ended, touched within STALE_AFTER_DAYS —
    /// newest first, capped for the Continue surfaces.
    pub async fn list_open<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
    ) -> DbResult<Vec<SessionListItem>> {
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM reading_sessions \
             WHERE user_id = ? AND deleted_at IS NULL AND ended_at IS NULL \
               AND julianday(updated_at) >= julianday(?) \
             ORDER BY updated_at DESC, id DESC LIMIT {OPEN_SNAPSHOT_LIMIT}"
        );
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [user_id.into(), stale_cutoff().into()],
            ))
            .await
            .map_err_to_response()?;
        rows.iter()
            .map(SessionListItem::from_row)
            .collect::<Result<_, _>>()
            .map_err_to_response()
    }

    /// Archive page: live threads that ended or went stale, newest first,
    /// keyset-paginated on `(updated_at, id)` strictly before the cursor.
    pub async fn list_archive<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        before: Option<(&str, &str)>,
        limit: u64,
    ) -> DbResult<Vec<SessionListItem>> {
        let mut values: Vec<sea_orm::Value> = vec![user_id.into(), stale_cutoff().into()];
        let cursor = match before {
            Some((ts, id)) => {
                values.extend([ts.into(), ts.into(), id.into()]);
                "AND (updated_at < ? OR (updated_at = ? AND id < ?))"
            }
            None => "",
        };
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM reading_sessions \
             WHERE user_id = ? AND deleted_at IS NULL \
               AND (ended_at IS NOT NULL OR julianday(updated_at) < julianday(?)) {cursor} \
             ORDER BY updated_at DESC, id DESC LIMIT {limit}"
        );
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                values,
            ))
            .await
            .map_err_to_response()?;
        rows.iter()
            .map(SessionListItem::from_row)
            .collect::<Result<_, _>>()
            .map_err_to_response()
    }

    /// MAX-merge one surah's furthest ayah: never regresses across devices;
    /// `updated_at` follows the greater instant. 1 = row changed, 0 = no-op.
    pub async fn upsert_progress(
        conn: &DatabaseTransaction,
        user_id: i32,
        surah: i32,
        furthest_ayah: i32,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO reading_progress (user_id, surah, furthest_ayah, updated_at)
                   VALUES (?, ?, ?, ?)
                   ON CONFLICT (user_id, surah) DO UPDATE SET
                       furthest_ayah = excluded.furthest_ayah,
                       updated_at = excluded.updated_at
                   WHERE excluded.furthest_ayah > reading_progress.furthest_ayah"#,
                [
                    user_id.into(),
                    surah.into(),
                    furthest_ayah.into(),
                    lww_timestamp(updated_at).into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    pub async fn list_progress(
        conn: &DatabaseTransaction,
        user_id: i32,
    ) -> DbResult<Vec<ProgressListItem>> {
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT surah, furthest_ayah, updated_at FROM reading_progress \
                 WHERE user_id = ? ORDER BY surah ASC",
                [user_id.into()],
            ))
            .await
            .map_err_to_response()?;
        rows.iter()
            .map(|r| {
                Ok(ProgressListItem {
                    surah: r.try_get("", "surah")?,
                    furthest_ayah: r.try_get("", "furthest_ayah")?,
                    updated_at: r.try_get("", "updated_at")?,
                })
            })
            .collect::<Result<_, DbErr>>()
            .map_err_to_response()
    }

    async fn count_live_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()
    }
}
