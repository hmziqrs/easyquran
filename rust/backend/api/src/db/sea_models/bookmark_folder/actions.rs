use crate::error::{DbResult, DbResultExt};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use tracing::instrument;
use uuid::Uuid;

use super::{Column, Entity, Model};

/// Canonical LWW timestamp: UTC + RFC3339 with fixed-width micros ("...Z").
/// Fixed width keeps the TEXT columns lexicographically orderable; the raw
/// SQL comparisons additionally rank via julianday() so a corrupt or foreign
/// shape resolves by instant (a NULL julianday counts as oldest).
pub(crate) fn lww_timestamp(ts: DateTimeWithTimeZone) -> String {
    ts.with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// UUIDv5 namespace for default-folder ids. Mirrored byte-for-byte by the web
/// client (`#lib/bookmarks/schema.ts`), so every device of an account derives
/// the same default folder offline and they converge instead of duplicating.
const DEFAULT_FOLDER_NAMESPACE: Uuid = Uuid::from_u128(0x6f0e4c1a_2b7d_4e59_9a83_5c1d2e3f4a5b);

/// Seed name; the user may rename it (a client upsert always wins over the
/// epoch-stamped seed).
pub const DEFAULT_FOLDER_NAME: &str = "Bookmarks";

/// Seed timestamp: older than any real client write, so a rename always wins.
const SEED_TS: &str = "1970-01-01T00:00:00.000000Z";

/// Live-folder quota per user. A NEW live folder past the cap is refused
/// (surfaced via the round's `skipped` count).
const LIVE_FOLDER_QUOTA: u64 = 200;

/// Deterministic default-folder id for one account.
pub fn default_folder_id(user_id: i32) -> String {
    Uuid::new_v5(
        &DEFAULT_FOLDER_NAMESPACE,
        format!("user:{user_id}").as_bytes(),
    )
    .to_string()
}

impl Entity {
    /// Make sure the account's default folder exists and is live. Runs at the
    /// head of every sync round, before any mutation resolves a folder id.
    pub async fn ensure_default(conn: &DatabaseTransaction, user_id: i32) -> DbResult<()> {
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO bookmark_folders (user_id, id, name, is_default, created_at, updated_at)
               VALUES (?, ?, ?, 1, ?, ?)
               ON CONFLICT (user_id, id) DO UPDATE SET is_default = 1, deleted_at = NULL"#,
            [
                user_id.into(),
                default_folder_id(user_id).into(),
                DEFAULT_FOLDER_NAME.into(),
                SEED_TS.into(),
                SEED_TS.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        Ok(())
    }

    /// Last-writer-wins upsert (name, position). Returns rows affected
    /// (1 = applied, 0 = skipped by LWW or quota). A newer upsert clears
    /// `deleted_at` — resurrection over a tombstone is intentional (undo).
    /// `is_default` is never client-writable.
    #[instrument(skip(conn), fields(user_id = user_id, folder_id = id))]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        name: &str,
        position: Option<i32>,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);
        let exists = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(id))
            .count(conn)
            .await
            .map_err_to_response()?;
        if exists == 0 && Self::count_live_for_user(conn, user_id).await? >= LIVE_FOLDER_QUOTA {
            return Ok(0);
        }
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO bookmark_folders (user_id, id, name, is_default, position,
                                                created_at, updated_at)
                   VALUES (?, ?, ?, 0, ?, ?, ?)
                   ON CONFLICT (user_id, id) DO UPDATE SET
                       name = excluded.name,
                       position = excluded.position,
                       updated_at = excluded.updated_at,
                       deleted_at = NULL
                   WHERE julianday(bookmark_folders.updated_at) IS NULL
                      OR julianday(excluded.updated_at) > julianday(bookmark_folders.updated_at)"#,
                [
                    user_id.into(),
                    id.into(),
                    name.into(),
                    position.into(),
                    ts.clone().into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins folder delete = TOMBSTONE, cascading to its bookmarks.
    /// The default folder is never deletable (skip). Bookmarks written into
    /// the folder AFTER the delete instant (another device's newer intent) are
    /// re-homed to the default folder when that key is free there; everything
    /// else in the folder is tombstoned with the delete instant. Returns 1 when
    /// the folder tombstone applied, 0 when skipped (absent, default, newer).
    #[instrument(skip(conn), fields(user_id = user_id, folder_id = id))]
    pub async fn delete_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let default_id = default_folder_id(user_id);
        if id == default_id {
            return Ok(0);
        }
        let ts = lww_timestamp(updated_at);
        let removed = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"UPDATE bookmark_folders SET deleted_at = ?, updated_at = ?
                   WHERE user_id = ? AND id = ?
                     AND (julianday(updated_at) IS NULL
                          OR julianday(updated_at) <= julianday(?))"#,
                [
                    ts.clone().into(),
                    ts.clone().into(),
                    user_id.into(),
                    id.into(),
                    ts.clone().into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        if removed.rows_affected() == 0 {
            return Ok(0);
        }
        // Newer live rows survive into the default folder unless their key is
        // already live there.
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET folder_id = ?
               WHERE user_id = ? AND folder_id = ? AND deleted_at IS NULL
                 AND julianday(updated_at) > julianday(?)
                 AND NOT EXISTS (
                     SELECT 1 FROM bookmarks d
                     WHERE d.user_id = bookmarks.user_id AND d.folder_id = ?
                       AND d.surah = bookmarks.surah AND d.ayah = bookmarks.ayah
                       AND d.end_surah = bookmarks.end_surah AND d.end_ayah = bookmarks.end_ayah
                       AND d.deleted_at IS NULL)"#,
            [
                default_id.clone().into(),
                user_id.into(),
                id.into(),
                ts.clone().into(),
                default_id.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET deleted_at = ?, updated_at = ?
               WHERE user_id = ? AND folder_id = ? AND deleted_at IS NULL"#,
            [ts.clone().into(), ts.into(), user_id.into(), id.into()],
        ))
        .await
        .map_err_to_response()?;
        Ok(1)
    }

    /// Live rows, default folder first, then `(created_at, id)` — a
    /// deterministic order every device converges on (clients apply the
    /// manual `position` on top).
    pub async fn list_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .order_by(Column::IsDefault, Order::Desc)
            .order_by(Column::CreatedAt, Order::Asc)
            .order_by(Column::Id, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }

    /// Ownership-resolved live folder lookup: a tombstoned or foreign folder
    /// resolves as missing.
    pub async fn live_for_user(
        conn: &DatabaseTransaction,
        user_id: i32,
        folder_id: &str,
    ) -> DbResult<bool> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(folder_id))
            .filter(Column::DeletedAt.is_null())
            .one(conn)
            .await
            .map_err_to_response()
            .map(|row| row.is_some())
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
