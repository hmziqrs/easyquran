use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use tracing::instrument;

use super::{Column, Entity, Model};

/// Canonical LWW timestamp: UTC + RFC3339 with fixed-width micros ("...Z").
/// Fixed width makes the TEXT columns lexicographically orderable, so the raw
/// SQL LWW comparisons are exact chronological comparisons — as long as every
/// write path normalizes through this helper and legacy
/// 'YYYY-MM-DD HH:MM:SS' rows have been normalized by migration m000008. The
/// raw comparisons additionally rank via julianday() (see below) so even a
/// stray legacy row resolves by actual instant, not lexeme.
pub(crate) fn lww_timestamp(ts: DateTimeWithTimeZone) -> String {
    ts.with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

impl Entity {
    /// Last-writer-wins upsert. Returns rows affected (1 = applied, 0 = skipped
    /// by LWW or ownership). The DO UPDATE guard refuses to touch a row owned
    /// by another user and only overwrites when the incoming timestamp is
    /// strictly newer. Timestamps compare through julianday() so a legacy
    /// CURRENT_TIMESTAMP-shaped row and a canonical RFC3339-micros row resolve
    /// by actual instant (datetime() would truncate sub-second precision).
    #[instrument(skip(conn), fields(user_id = user_id, folder_id = id))]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        name: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO bookmark_folders (id, user_id, name, created_at, updated_at)
                   VALUES (?, ?, ?, ?, ?)
                   ON CONFLICT (id) DO UPDATE SET
                       name = excluded.name,
                       updated_at = excluded.updated_at
                   WHERE bookmark_folders.user_id = excluded.user_id
                     AND julianday(excluded.updated_at) > julianday(bookmark_folders.updated_at)"#,
                [
                    id.into(),
                    user_id.into(),
                    name.into(),
                    ts.clone().into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins delete: refuses to remove a row newer than the delete
    /// marker (julianday() ranks legacy and canonical TEXT by instant).
    /// Applied deletes first detach child bookmarks to root (folder_id
    /// = NULL). Returns rows affected by the DELETE itself (1 = applied,
    /// 0 = skipped — absent row, foreign row, or newer row).
    #[instrument(skip(conn), fields(user_id = user_id, folder_id = id))]
    pub async fn delete_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let removed = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"DELETE FROM bookmark_folders
                   WHERE id = ? AND user_id = ?
                     AND julianday(updated_at) <= julianday(?) "#,
                [id.into(), user_id.into(), lww_timestamp(updated_at).into()],
            ))
            .await
            .map_err_to_response()?;
        if removed.rows_affected() == 0 {
            return Ok(0);
        }
        // Children to root; only after the delete itself applied, so a refused
        // (older) delete leaves the folder and its children untouched.
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET folder_id = NULL
               WHERE folder_id = ? AND user_id = ?"#,
            [id.into(), user_id.into()],
        ))
        .await
        .map_err_to_response()?;
        Ok(1)
    }

    pub async fn list_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::UpdatedAt, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }

    /// Ownership-resolved folder lookup: a folder id only counts when it
    /// belongs to this user, so a bookmark referencing another user's folder
    /// (or a nonexistent one) is silently filed under root.
    pub async fn owned_by_user(
        conn: &DatabaseTransaction,
        user_id: i32,
        folder_id: &str,
    ) -> DbResult<bool> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(folder_id))
            .one(conn)
            .await
            .map_err_to_response()
            .map(|row| row.is_some())
    }
}
