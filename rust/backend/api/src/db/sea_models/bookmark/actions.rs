use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use tracing::instrument;

use super::super::bookmark_folder::lww_timestamp;
use super::{Column, Entity, Model};

/// Per-user live-bookmark quota. An upsert that would mint a NEW live row past
/// the cap is refused (surfaced via the round's `skipped` count); updating an
/// already-live id can never grow the set, so it is always allowed.
const LIVE_BOOKMARK_QUOTA: u64 = 10_000;

/// One bookmark write: the live key is (folder, surah, ayah, end_surah,
/// end_ayah); `source_id` is context metadata, never a key part.
pub struct BookmarkWrite<'a> {
    pub id: &'a str,
    pub folder_id: &'a str,
    pub source_id: Option<&'a str>,
    pub surah: i32,
    pub ayah: i32,
    pub end_surah: i32,
    pub end_ayah: i32,
}

impl Entity {
    /// Last-writer-wins upsert over the live key. Returns rows affected
    /// (1 = applied, 0 = skipped by LWW or quota). The id is the row's
    /// IDENTITY and the key a mutable attribute, so a newer push may MOVE a
    /// row between folders. Checks strictly before writes, so a skip is never
    /// destructive:
    ///
    /// 1. Same-id staleness — this user already holds the id (live or
    ///    tombstoned) with an updated_at not strictly older → stale/replay.
    /// 2. Newer rival — another id on the target key (live or tombstoned)
    ///    that is strictly newer → stale push.
    /// 3. Apply — drop the older same-id row, tombstone older live rivals on
    ///    the key (two devices can mint two ids for one key; tombstones keep
    ///    their ids protected against stale replays), then insert.
    ///
    /// Timestamps rank through julianday(); a row whose updated_at does not
    /// parse counts as OLDEST, so corrupt rows stay killable.
    #[instrument(skip(conn, w), fields(user_id = user_id, bookmark_id = w.id))]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        w: &BookmarkWrite<'_>,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);

        let same_id_live = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(w.id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()?;
        if same_id_live == 0
            && Self::count_live_for_user(conn, user_id).await? >= LIVE_BOOKMARK_QUOTA
        {
            return Ok(0);
        }

        let same_id_newer = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT 1 FROM bookmarks
                   WHERE user_id = ? AND id = ?
                     AND julianday(updated_at) IS NOT NULL
                     AND julianday(updated_at) >= julianday(?)"#,
                [user_id.into(), w.id.into(), ts.clone().into()],
            ))
            .await
            .map_err_to_response()?;
        if same_id_newer.is_some() {
            return Ok(0);
        }

        let key: [sea_orm::Value; 7] = [
            user_id.into(),
            w.folder_id.into(),
            w.surah.into(),
            w.ayah.into(),
            w.end_surah.into(),
            w.end_ayah.into(),
            w.id.into(),
        ];
        let newer_rival = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"SELECT 1 FROM bookmarks
                   WHERE user_id = ? AND folder_id = ? AND surah = ? AND ayah = ?
                     AND end_surah = ? AND end_ayah = ? AND id <> ?
                     AND julianday(updated_at) IS NOT NULL
                     AND julianday(updated_at) > julianday(?)"#,
                key.iter().cloned().chain([ts.clone().into()]),
            ))
            .await
            .map_err_to_response()?;
        if newer_rival.is_some() {
            return Ok(0);
        }

        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"DELETE FROM bookmarks WHERE user_id = ? AND id = ?"#,
            [user_id.into(), w.id.into()],
        ))
        .await
        .map_err_to_response()?;

        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET deleted_at = ?, updated_at = ?
               WHERE user_id = ? AND folder_id = ? AND surah = ? AND ayah = ?
                 AND end_surah = ? AND end_ayah = ? AND id <> ?
                 AND deleted_at IS NULL"#,
            [ts.clone().into(), ts.clone().into()]
                .into_iter()
                .chain(key.iter().cloned()),
        ))
        .await
        .map_err_to_response()?;

        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO bookmarks (user_id, id, folder_id, source_id,
                                         surah, ayah, end_surah, end_ayah,
                                         created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                [
                    user_id.into(),
                    w.id.into(),
                    w.folder_id.into(),
                    w.source_id.into(),
                    w.surah.into(),
                    w.ayah.into(),
                    w.end_surah.into(),
                    w.end_ayah.into(),
                    ts.clone().into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins delete = TOMBSTONE: `deleted_at = updated_at = the
    /// mutation instant`, so a stale queued upsert cannot resurrect the row.
    /// Refused when the stored row is strictly newer. Returns rows affected
    /// (1 = applied, 0 = absent, foreign, or newer).
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

    /// Live snapshot rows, deterministic tie order `(updated_at, id)`.
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

    async fn count_live_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()
    }
}
