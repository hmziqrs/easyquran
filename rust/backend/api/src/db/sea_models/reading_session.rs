//! `reading_sessions` entity + LWW actions (next-features §9.1). Flat module,
//! not a `mod.rs` dir: the three sibling tables live in three sibling files
//! (`reading_session_position.rs`, `reading_session_progress.rs`), one
//! `Model` per file — same split as bookmark/bookmark_folder without three
//! near-empty directory shells.

use crate::error::{DbResult, DbResultExt};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::bookmark_folder::lww_timestamp;

/// Per-user live-session quota (§9.1; settled at 50 in §11.12). Upserting a
/// NEW live session past the cap is refused (surfaced via the round's
/// `skipped` count); updating an already-live id never grows the set.
const LIVE_SESSION_QUOTA: u64 = 50;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_sessions")]
pub struct Model {
    /// Composite PK (user_id, id): the same client-minted id under two
    /// accounts is TWO sessions (EC-52), never a collision.
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    /// "daily" | "weekly" | "free" | "shared". `shared` is a seam kind; the
    /// sharedWith list itself is local-only, never a column.
    pub kind: String,
    pub translation_id: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub archived_at: Option<DateTimeWithTimeZone>,
    /// Tombstone marker (§9.3): a deleted session keeps refusing stale
    /// position/progress pushes until the purge job drops it.
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

/// Response item for the sync snapshot (camelCase wire format; user_id and
/// the tombstone marker never leave the server).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub translation_id: Option<String>,
    pub archived_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl From<Model> for SessionListItem {
    fn from(m: Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            kind: m.kind,
            translation_id: m.translation_id,
            archived_at: m.archived_at,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

impl Entity {
    /// Last-writer-wins session upsert, same contract as
    /// `bookmark_folder::upsert_lww`: DO UPDATE only on a strictly newer
    /// timestamp (julianday() ranks legacy and canonical TEXT by instant;
    /// corrupt NULL counts as oldest), a newer upsert clears the tombstone
    /// (intentional resurrection), and a foreign-account id collision is a
    /// no-op. The quota gates only NEW live rows. Returns rows affected
    /// (1 = applied, 0 = skipped by LWW, ownership, or quota).
    #[instrument(skip(conn), fields(user_id = user_id, session_id = id))]
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        name: &str,
        kind: &str,
        translation_id: Option<&str>,
        archived_at: Option<&str>,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);
        let live = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()?;
        if live == 0 && Self::count_live_for_user(conn, user_id).await? >= LIVE_SESSION_QUOTA {
            return Ok(0);
        }
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO reading_sessions (id, user_id, name, kind, translation_id,
                                                 created_at, updated_at, archived_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT (user_id, id) DO UPDATE SET
                       name = excluded.name,
                       kind = excluded.kind,
                       translation_id = excluded.translation_id,
                       archived_at = excluded.archived_at,
                       updated_at = excluded.updated_at,
                       deleted_at = NULL
                   WHERE reading_sessions.user_id = excluded.user_id
                     AND (julianday(reading_sessions.updated_at) IS NULL
                          OR julianday(excluded.updated_at) > julianday(reading_sessions.updated_at))"#,
                [
                    id.into(),
                    user_id.into(),
                    name.into(),
                    kind.into(),
                    translation_id.into(),
                    ts.clone().into(),
                    ts.clone().into(),
                    archived_at.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Last-writer-wins delete = TOMBSTONE (§9.3), refused when the stored row
    /// is strictly newer. An applied delete also drops the session's position
    /// and progress rows and re-homes its session-scoped bookmarks to global
    /// (`session_id = ''`) in the SAME transaction (§9.1): a scoped bookmark
    /// whose key collides with a live global rival becomes a tombstone so the
    /// partial unique index stays satisfiable and the global row survives
    /// (EC-53). Returns 1 = applied, 0 = skipped (absent/foreign/newer).
    #[instrument(skip(conn), fields(user_id = user_id, session_id = id))]
    pub async fn delete_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        let ts = lww_timestamp(updated_at);
        let removed = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"UPDATE reading_sessions SET deleted_at = ?, updated_at = ?
                   WHERE user_id = ? AND id = ?
                     AND (julianday(updated_at) IS NULL
                          OR julianday(updated_at) <= julianday(?))"#,
                [ts.clone().into(), ts.clone().into(), user_id.into(), id.into(), ts.clone().into()],
            ))
            .await
            .map_err_to_response()?;
        if removed.rows_affected() == 0 {
            return Ok(0);
        }
        // Positions/progress have no identity worth protecting — the live-
        // session guard in their upserts is what keeps stale replays out
        // after the tombstone.
        for stmt in [
            "DELETE FROM reading_session_positions WHERE user_id = ? AND session_id = ?",
            "DELETE FROM reading_session_progress WHERE user_id = ? AND session_id = ?",
        ] {
            conn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                stmt,
                [user_id.into(), id.into()],
            ))
            .await
            .map_err_to_response()?;
        }
        // Colliding scoped rows die as tombstones (the global row stays);
        // non-colliding ones re-home. Both scoped to THIS user + session.
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET deleted_at = ?, updated_at = ?
               WHERE user_id = ? AND session_id = ? AND deleted_at IS NULL
                 AND EXISTS (SELECT 1 FROM bookmarks g
                             WHERE g.user_id = bookmarks.user_id
                               AND g.session_id = '' AND g.deleted_at IS NULL
                               AND g.surah = bookmarks.surah AND g.ayah = bookmarks.ayah
                               AND g.end_surah = bookmarks.end_surah
                               AND g.end_ayah = bookmarks.end_ayah)"#,
            [ts.clone().into(), ts.clone().into(), user_id.into(), id.into()],
        ))
        .await
        .map_err_to_response()?;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"UPDATE bookmarks SET session_id = ''
               WHERE user_id = ? AND session_id = ? AND deleted_at IS NULL"#,
            [user_id.into(), id.into()],
        ))
        .await
        .map_err_to_response()?;
        Ok(1)
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

    /// Live-session ownership guard for position/progress upserts (EC-52): a
    /// push naming a session this user does not live-own (foreign, deleted,
    /// or absent) must be skipped, never written.
    pub async fn live_owned_by_user(
        conn: &DatabaseTransaction,
        user_id: i32,
        session_id: &str,
    ) -> DbResult<bool> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(session_id))
            .filter(Column::DeletedAt.is_null())
            .one(conn)
            .await
            .map_err_to_response()
            .map(|row| row.is_some())
    }

    /// Live (non-tombstoned) session count for the per-user quota check.
    pub async fn count_live_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()
    }
}
