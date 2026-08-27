use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use tracing::instrument;

use super::super::bookmark_folder::lww_timestamp;
use super::{Column, Entity, Model};

impl Entity {
    /// Last-writer-wins upsert, verse-collapsed. Returns rows affected
    /// (1 = applied, 0 = skipped by LWW or ownership). The id is the row's
    /// IDENTITY and the verse a mutable attribute, so a newer push may MOVE
    /// a row between verses. Statements inside the caller's transaction,
    /// checks strictly before deletes:
    ///
    /// 1. Same-id staleness check — if THIS user already holds the id (on any
    ///    verse) with a comparable updated_at not strictly older than the
    ///    incoming mutation, the push is stale or a replay: return 0 with NO
    ///    deletes executed. Checking first is what keeps a skip
    ///    non-destructive — running the verse-collapse delete before this
    ///    check could kill the target verse's older rivals and then refuse
    ///    the upsert, a "skipped" push that still destroyed data.
    /// 2. Newer-rival check — a rival on the target verse (id ≠ incoming)
    ///    that is strictly NEWER means the push is stale: skip (0 rows)
    ///    instead of clobbering it. An explicit SELECT beats layering
    ///    ON CONFLICT(user_id, surah, ayah) DO NOTHING onto the insert: DO
    ///    NOTHING would blur "stale skip" into rows_affected and could mask a
    ///    genuine write failure, while the explicit check keeps 0 meaning
    ///    precisely LWW-refused.
    /// 3. Legitimate apply, delete-then-insert — drop the same-id row (step 1
    ///    proved it older-or-corrupt, possibly on another verse: this is the
    ///    move path), collapse the target verse's rivals that are not
    ///    strictly newer (keeping idx_bookmarks_user_verse satisfiable before
    ///    the insert), then the by-id upsert. The DO UPDATE guard refuses to
    ///    touch a row owned by another user (an id collision across accounts
    ///    must be a no-op) — after step 3a it is a last line of defense, and
    ///    0 still means precisely LWW/ownership-refused.
    ///
    /// Timestamps compare through julianday() — the m000008-normalized TEXT
    /// could still meet a legacy 'YYYY-MM-DD HH:MM:SS' row, and julianday()
    /// ranks both by actual instant (datetime() would truncate sub-second
    /// precision and make same-second micros LWW-equal). A row whose
    /// updated_at does not parse (julianday NULL) is treated as OLDEST in
    /// every comparison: it never blocks a mutation and always loses LWW,
    /// so corrupt rows stay killable instead of immortal.
    #[instrument(skip(conn), fields(user_id = user_id, bookmark_id = id))]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        folder_id: Option<&str>,
        surah: i32,
        ayah: i32,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);

        // Step 1: same-id row (any verse) that is comparable and not strictly
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
        // on the target verse → stale push, refuse without deleting.
        let newer_rival = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT rowid FROM bookmarks
                   WHERE user_id = ? AND surah = ? AND ayah = ? AND id <> ?
                     AND julianday(updated_at) IS NOT NULL
                     AND julianday(updated_at) > julianday(?)"#,
                [
                    user_id.into(),
                    surah.into(),
                    ayah.into(),
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
        // or another verse (the legitimate move).
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"DELETE FROM bookmarks WHERE id = ? AND user_id = ?"#,
            [id.into(), user_id.into()],
        ))
        .await
        .map_err_to_response()?;

        // Step 3b: verse-collapse — ids are client-minted UUIDv4, so two
        // devices can mint two uuids for one verse. Rivals for the SAME verse
        // that are not strictly newer (or whose corrupt timestamp reads as
        // oldest) lose LWW and die here.
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"DELETE FROM bookmarks
               WHERE user_id = ? AND surah = ? AND ayah = ? AND id <> ?
                 AND (julianday(updated_at) IS NULL
                      OR julianday(updated_at) <= julianday(?))"#,
            [
                user_id.into(),
                surah.into(),
                ayah.into(),
                id.into(),
                ts.clone().into(),
            ],
        ))
        .await
        .map_err_to_response()?;

        // Step 3c: by-id upsert — the verse is clear and the same-id row is
        // gone, so this inserts; the DO UPDATE guard stays for an id
        // collision with another account (no-op there).
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO bookmarks (id, user_id, folder_id, surah, ayah, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT (id) DO UPDATE SET
                       folder_id = excluded.folder_id,
                       surah = excluded.surah,
                       ayah = excluded.ayah,
                       updated_at = excluded.updated_at
                   WHERE bookmarks.user_id = excluded.user_id
                     AND (julianday(bookmarks.updated_at) IS NULL
                          OR julianday(excluded.updated_at) > julianday(bookmarks.updated_at))"#,
                [
                    id.into(),
                    user_id.into(),
                    folder_id.into(),
                    surah.into(),
                    ayah.into(),
                    ts.clone().into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins delete: the julianday() comparison deletes rows at
    /// most as new as the delete marker and refuses strictly newer ones. A
    /// corrupt timestamp (julianday NULL) counts as oldest, so the row is
    /// deletable instead of immortal. Returns rows affected (1 = applied,
    /// 0 = skipped — absent row, foreign row, or newer row).
    #[instrument(skip(conn), fields(user_id = user_id, bookmark_id = id))]
    pub async fn delete_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"DELETE FROM bookmarks
                   WHERE id = ? AND user_id = ?
                     AND (julianday(updated_at) IS NULL
                          OR julianday(updated_at) <= julianday(?)) "#,
                [id.into(), user_id.into(), lww_timestamp(updated_at).into()],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    pub async fn list_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::UpdatedAt, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }
}
