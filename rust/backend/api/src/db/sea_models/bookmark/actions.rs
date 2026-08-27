use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use tracing::instrument;

use super::super::bookmark_folder::lww_timestamp;
use super::{Column, Entity, Model};

impl Entity {
    /// Last-writer-wins upsert, verse-collapsed. Returns rows affected
    /// (1 = applied, 0 = skipped by LWW or ownership). Three statements inside
    /// the caller's transaction:
    ///
    /// 1. Verse-collapse DELETE — the row identity is the verse
    ///    (user_id, surah, ayah), but ids are client-minted UUIDv4, so two
    ///    devices can mint two uuids for one verse. Rival rows for the SAME
    ///    verse that are not strictly newer than the incoming mutation lose
    ///    LWW and die here, keeping idx_bookmarks_user_verse satisfiable
    ///    before the insert.
    /// 2. Newer-rival check — a rival that survived step 1 is strictly NEWER
    ///    than the incoming mutation: the push is stale, skip (0 rows) instead
    ///    of clobbering it. An explicit SELECT beats layering
    ///    ON CONFLICT(user_id, surah, ayah) DO NOTHING onto the insert: DO
    ///    NOTHING would blur "stale skip" into rows_affected and could mask a
    ///    genuine write failure, while the explicit check keeps 0 meaning
    ///    precisely LWW-refused.
    /// 3. By-id upsert — the DO UPDATE guard refuses to touch a row owned by
    ///    another user (an id collision across accounts must be a no-op) and
    ///    only overwrites when the incoming timestamp is strictly newer.
    ///
    /// Timestamps compare through julianday() — the m000008-normalized TEXT
    /// could still meet a legacy 'YYYY-MM-DD HH:MM:SS' row, and julianday()
    /// ranks both by actual instant (datetime() would truncate sub-second
    /// precision and make same-second micros LWW-equal).
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

        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"DELETE FROM bookmarks
               WHERE user_id = ? AND surah = ? AND ayah = ? AND id <> ?
                 AND julianday(updated_at) <= julianday(?)"#,
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

        let newer_rival = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT rowid FROM bookmarks
                   WHERE user_id = ? AND surah = ? AND ayah = ? AND id <> ?"#,
                [user_id.into(), surah.into(), ayah.into(), id.into()],
            ))
            .await
            .map_err_to_response()?;
        if newer_rival.is_some() {
            return Ok(0);
        }

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
                     AND julianday(excluded.updated_at) > julianday(bookmarks.updated_at)"#,
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
    /// most as new as the delete marker and refuses strictly newer ones.
    /// Returns rows affected (1 = applied, 0 = skipped — absent row, foreign
    /// row, or newer row).
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
                     AND julianday(updated_at) <= julianday(?) "#,
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
