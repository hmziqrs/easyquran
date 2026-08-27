use crate::error::{DbResult, DbResultExt};
use sea_orm::{entity::prelude::*, DatabaseBackend, Order, QueryOrder, Statement};
use tracing::instrument;

use super::super::bookmark_folder::lww_timestamp;
use super::{Column, Entity, Model};

impl Entity {
    /// Last-writer-wins upsert. Returns rows affected (1 = applied, 0 = skipped
    /// by LWW or ownership). The DO UPDATE guard refuses to touch a row owned
    /// by another user (the id is the client-minted UUIDv4, so an id collision
    /// across accounts must be a no-op) and only overwrites when the incoming
    /// timestamp is strictly newer than the stored one.
    #[instrument(skip(conn), fields(user_id = user_id, bookmark_id = id))]
    pub async fn upsert_lww(
        conn: &DbConn,
        user_id: i32,
        id: &str,
        folder_id: Option<&str>,
        surah: i32,
        ayah: i32,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);
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
                     AND excluded.updated_at > bookmarks.updated_at"#,
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

    /// Last-writer-wins delete: `updated_at <= ?` deletes rows at most as new
    /// as the delete marker and refuses strictly newer ones. Returns rows
    /// affected (1 = applied, 0 = skipped — absent row, foreign row, or newer
    /// row).
    #[instrument(skip(conn), fields(user_id = user_id, bookmark_id = id))]
    pub async fn delete_lww(
        conn: &DbConn,
        user_id: i32,
        id: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"DELETE FROM bookmarks
                   WHERE id = ? AND user_id = ? AND updated_at <= ?"#,
                [id.into(), user_id.into(), lww_timestamp(updated_at).into()],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    pub async fn list_for_user(conn: &DbConn, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::UpdatedAt, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }
}
