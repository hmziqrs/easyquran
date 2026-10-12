use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use tracing::instrument;

use super::super::bookmark_folder::lww_timestamp;
use super::{Column, Entity, Model};

/// Per-user live-bookmark quota (§9.1; settled at 10 000 in §11.12). An upsert
/// that would mint a NEW live row past the cap is refused (surfaced via the
/// round's `skipped` count); updating an already-live id can never grow the
/// set, so it is always allowed.
const LIVE_BOOKMARK_QUOTA: u64 = 10_000;

impl Entity {
    /// Last-writer-wins upsert over the scoped verse key
    /// (user_id, session_id, surah, ayah, end_surah, end_ayah; '' session +
    /// end = start are the global/single-ayah sentinels). Returns rows
    /// affected (1 = applied, 0 = skipped by LWW, ownership, or quota). The
    /// id is the row's IDENTITY and the key a mutable attribute, so a newer
    /// push may MOVE a row between keys. Statements inside the caller's
    /// transaction, checks strictly before writes:
    ///
    /// 1. Same-id staleness check — if THIS user already holds the id (on any
    ///    key, live or tombstoned — a tombstone's updated_at is its delete
    ///    time) with a comparable updated_at not strictly older than the
    ///    incoming mutation, the push is stale or a replay: return 0 with NO
    ///    writes executed. Checking first is what keeps a skip
    ///    non-destructive — running the key-collapse write before this
    ///    check could kill the target key's older rivals and then refuse
    ///    the upsert, a "skipped" push that still destroyed data.
    /// 2. Newer-rival check — a rival on the target key (id ≠ incoming, live
    ///    OR tombstoned — a newer tombstone is the key's latest state)
    ///    that is strictly NEWER means the push is stale: skip (0 rows)
    ///    instead of clobbering it. An explicit SELECT beats layering
    ///    ON CONFLICT DO NOTHING onto the insert: DO NOTHING would blur
    ///    "stale skip" into rows_affected and could mask a genuine write
    ///    failure, while the explicit check keeps 0 meaning precisely
    ///    LWW-refused.
    /// 3. Legitimate apply — drop the same-id row (step 1 proved it
    ///    older-or-corrupt, possibly on another key: this is the move path),
    ///    tombstone the target key's rivals that are not strictly newer
    ///    (keeping idx_bookmarks_user_verse satisfiable before the insert;
    ///    tombstoned — not deleted — so their ids stay protected against
    ///    stale replays), then the by-id upsert. The DO UPDATE guard refuses
    ///    to touch a row owned by another user (an id collision across
    ///    accounts must be a no-op) — after step 3a it is a last line of
    ///    defense, and 0 still means precisely LWW/ownership-refused.
    ///
    /// Timestamps compare through julianday() — the m000008-normalized TEXT
    /// could still meet a legacy 'YYYY-MM-DD HH:MM:SS' row, and julianday()
    /// ranks both by actual instant (datetime() would truncate sub-second
    /// precision and make same-second micros LWW-equal). A row whose
    /// updated_at does not parse (julianday NULL) is treated as OLDEST in
    /// every comparison: it never blocks a mutation and always loses LWW,
    /// so corrupt rows stay killable instead of immortal.
    #[instrument(skip(conn), fields(user_id = user_id, bookmark_id = id))]
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        folder_id: Option<&str>,
        session_id: &str,
        source_id: Option<&str>,
        surah: i32,
        ayah: i32,
        end_surah: i32,
        end_ayah: i32,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);

        // Step 0: quota — only a NEW live row can grow the set.
        let same_id_live = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT rowid FROM bookmarks
                   WHERE id = ? AND user_id = ? AND deleted_at IS NULL"#,
                [id.into(), user_id.into()],
            ))
            .await
            .map_err_to_response()?;
        if same_id_live.is_none()
            && Self::count_live_for_user(conn, user_id).await? >= LIVE_BOOKMARK_QUOTA
        {
            return Ok(0);
        }

        // Step 1: same-id row (any key) that is comparable and not strictly
        // older → stale push/replay, refuse before touching anything.
        let same_id_newer = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT rowid FROM bookmarks
                   WHERE id = ? AND user_id = ?
                     AND julianday(updated_at) IS NOT NULL
                     AND julianday(updated_at) >= julianday(?)"#,
                [id.into(), user_id.into(), ts.clone().into()],
            ))
            .await
            .map_err_to_response()?;
        if same_id_newer.is_some() {
            return Ok(0);
        }

        // Step 2: strictly newer (and comparable — corrupt NULL loses) rival
        // on the target key, tombstones included → stale push, refuse.
        let newer_rival = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT rowid FROM bookmarks
                   WHERE user_id = ? AND session_id = ? AND surah = ? AND ayah = ?
                     AND end_surah = ? AND end_ayah = ? AND id <> ?
                     AND julianday(updated_at) IS NOT NULL
                     AND julianday(updated_at) > julianday(?)"#,
                [
                    user_id.into(),
                    session_id.into(),
                    surah.into(),
                    ayah.into(),
                    end_surah.into(),
                    end_ayah.into(),
                    id.into(),
                    ts.clone().into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        if newer_rival.is_some() {
            return Ok(0);
        }

        // Step 3a: drop the same-id row — older-or-corrupt by step 1, on this
        // or another key (the legitimate move).
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"DELETE FROM bookmarks WHERE id = ? AND user_id = ?"#,
            [id.into(), user_id.into()],
        ))
        .await
        .map_err_to_response()?;

        // Step 3b: key-collapse — ids are client-minted, so two devices can
        // mint two uuids for one key. Older LIVE rivals become TOMBSTONES
        // (not deletes) so their ids stay protected against stale replays
        // while the incoming row takes the key (§9.3).
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET deleted_at = ?, updated_at = ?
               WHERE user_id = ? AND session_id = ? AND surah = ? AND ayah = ?
                 AND end_surah = ? AND end_ayah = ? AND id <> ?
                 AND deleted_at IS NULL
                 AND (julianday(updated_at) IS NULL
                      OR julianday(updated_at) <= julianday(?))"#,
            [
                ts.clone().into(),
                ts.clone().into(),
                user_id.into(),
                session_id.into(),
                surah.into(),
                ayah.into(),
                end_surah.into(),
                end_ayah.into(),
                id.into(),
                ts.clone().into(),
            ],
        ))
        .await
        .map_err_to_response()?;

        // Step 3c: by-id upsert — the key is clear and the same-id row is
        // gone, so this inserts; the DO UPDATE guard stays for an id
        // collision with another account (no-op there).
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO bookmarks (id, user_id, folder_id, session_id, source_id,
                                         surah, ayah, end_surah, end_ayah,
                                         created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT (id) DO UPDATE SET
                       folder_id = excluded.folder_id,
                       session_id = excluded.session_id,
                       source_id = excluded.source_id,
                       surah = excluded.surah,
                       ayah = excluded.ayah,
                       end_surah = excluded.end_surah,
                       end_ayah = excluded.end_ayah,
                       updated_at = excluded.updated_at
                   WHERE bookmarks.user_id = excluded.user_id
                     AND (julianday(bookmarks.updated_at) IS NULL
                          OR julianday(excluded.updated_at) > julianday(bookmarks.updated_at))"#,
                [
                    id.into(),
                    user_id.into(),
                    folder_id.into(),
                    session_id.into(),
                    source_id.into(),
                    surah.into(),
                    ayah.into(),
                    end_surah.into(),
                    end_ayah.into(),
                    ts.clone().into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins delete = TOMBSTONE (§9.3): `deleted_at = updated_at =
    /// the mutation's clamped updatedAt`, never a row removal, so a stale
    /// queued upsert cannot resurrect the verse. Refused when the stored row
    /// is strictly newer; a corrupt timestamp (julianday NULL) counts as
    /// oldest, so the row is deletable instead of immortal. Returns rows
    /// affected (1 = applied, 0 = skipped — absent row, foreign row, or
    /// newer row).
    #[instrument(skip(conn), fields(user_id = user_id, bookmark_id = id))]
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
                r#"UPDATE bookmarks SET deleted_at = ?, updated_at = ?
                   WHERE id = ? AND user_id = ?
                     AND (julianday(updated_at) IS NULL
                          OR julianday(updated_at) <= julianday(?))"#,
                [ts.clone().into(), ts.clone().into(), id.into(), user_id.into(), ts.into()],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Live (non-tombstoned) snapshot rows, deterministic tie order
    /// `(updated_at, id)` (EC-35).
    pub async fn list_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .order_by(Column::UpdatedAt, Order::Asc)
            .order_by(Column::Id, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }

    /// Live (non-tombstoned) bookmark count for the per-user quota check.
    pub async fn count_live_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()
    }
}
