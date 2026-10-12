//! `reading_session_positions` entity + LWW action (next-features §9.1): one
//! continue-reading anchor per (user, session). Pure LWW — latest position
//! wins, no MAX on ratio/page (§9.3).

use crate::error::{DbResult, DbResultExt};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::bookmark_folder::lww_timestamp;
use super::reading_session::Entity as SessionEntity;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_session_positions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub session_id: String,
    /// Wire-shaped "s:a" convenience copy of surah/ayah.
    pub verse_key: String,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    pub updated_at: DateTimeWithTimeZone,
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

/// Response item for the sync snapshot (camelCase wire format).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionListItem {
    pub session_id: String,
    pub verse_key: String,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    pub updated_at: DateTimeWithTimeZone,
}

impl From<Model> for PositionListItem {
    fn from(m: Model) -> Self {
        Self {
            session_id: m.session_id,
            verse_key: m.verse_key,
            surah: m.surah,
            ayah: m.ayah,
            local_page: m.local_page,
            ratio: m.ratio,
            source_id: m.source_id,
            updated_at: m.updated_at,
        }
    }
}

impl Entity {
    /// Position upsert, pure LWW on `updated_at` (§9.3). Guarded by live
    /// session ownership: a push naming a session this user does not live-own
    /// (foreign id, tombstoned, or absent) is skipped with 0 rows — the EC-52
    /// crafted-id defense, so it lives in the action, not the caller.
    /// Returns 1 = applied, 0 = skipped (guard or stale LWW).
    #[instrument(skip(conn), fields(user_id = user_id, session_id = session_id))]
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_lww(
        conn: &DatabaseTransaction,
        user_id: i32,
        session_id: &str,
        verse_key: &str,
        surah: i32,
        ayah: i32,
        local_page: Option<i32>,
        ratio: Option<f64>,
        source_id: Option<&str>,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        if !SessionEntity::live_owned_by_user(conn, user_id, session_id).await? {
            return Ok(0);
        }
        let ts = lww_timestamp(updated_at);
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO reading_session_positions
                       (user_id, session_id, verse_key, surah, ayah,
                        local_page, ratio, source_id, updated_at)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT (user_id, session_id) DO UPDATE SET
                       verse_key = excluded.verse_key,
                       surah = excluded.surah,
                       ayah = excluded.ayah,
                       local_page = excluded.local_page,
                       ratio = excluded.ratio,
                       source_id = excluded.source_id,
                       updated_at = excluded.updated_at
                   WHERE reading_session_positions.user_id = excluded.user_id
                     AND (julianday(reading_session_positions.updated_at) IS NULL
                          OR julianday(excluded.updated_at)
                             > julianday(reading_session_positions.updated_at))"#,
                [
                    user_id.into(),
                    session_id.into(),
                    verse_key.into(),
                    surah.into(),
                    ayah.into(),
                    local_page.into(),
                    ratio.into(),
                    source_id.into(),
                    ts.into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Snapshot rows, deterministic tie order `(updated_at, session_id)`
    /// (EC-35 — this table's natural tiebreak key is the session id).
    pub async fn list_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::UpdatedAt, Order::Asc)
            .order_by(Column::SessionId, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }
}
