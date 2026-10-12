//! `reading_session_progress` entity + MAX-merge action (next-features §9.1):
//! per-(user, session, surah) furthest ayah. NOT plain LWW — `furthest_ayah`
//! merges with MAX so progress never regresses across devices (§9.3, EC-05);
//! only the row's `updated_at` follows LWW.

use crate::error::{DbResult, DbResultExt};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::bookmark_folder::lww_timestamp;
use super::reading_session::Entity as SessionEntity;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_session_progress")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub session_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub surah: i32,
    pub furthest_ayah: i32,
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
pub struct ProgressListItem {
    pub session_id: String,
    pub surah: i32,
    pub furthest_ayah: i32,
    pub updated_at: DateTimeWithTimeZone,
}

impl From<Model> for ProgressListItem {
    fn from(m: Model) -> Self {
        Self {
            session_id: m.session_id,
            surah: m.surah,
            furthest_ayah: m.furthest_ayah,
            updated_at: m.updated_at,
        }
    }
}

impl Entity {
    /// Progress upsert with the §9.3 special merge: on a newer (LWW-winning)
    /// timestamp the row applies with `furthest_ayah = MAX(stored, incoming)`,
    /// so a stale device reporting 2:50 under 2:100 can never drag progress
    /// backwards (EC-05). An older-timestamp push is skipped whole — LWW
    /// still governs WHO writes, MAX governs WHAT lands. Same live-session
    /// ownership guard as positions (EC-52). Returns 1 = applied, 0 = skipped.
    #[instrument(skip(conn), fields(user_id = user_id, session_id = session_id))]
    pub async fn upsert_max(
        conn: &DatabaseTransaction,
        user_id: i32,
        session_id: &str,
        surah: i32,
        furthest_ayah: i32,
        updated_at: DateTimeWithTimeZone,
    ) -> DbResult<u64> {
        if !SessionEntity::live_owned_by_user(conn, user_id, session_id).await? {
            return Ok(0);
        }
        let ts = lww_timestamp(updated_at);
        let res = conn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"INSERT INTO reading_session_progress
                       (user_id, session_id, surah, furthest_ayah, updated_at)
                   VALUES (?, ?, ?, ?, ?)
                   ON CONFLICT (user_id, session_id, surah) DO UPDATE SET
                       furthest_ayah = MAX(reading_session_progress.furthest_ayah,
                                           excluded.furthest_ayah),
                       updated_at = excluded.updated_at
                   WHERE reading_session_progress.user_id = excluded.user_id
                     AND (julianday(reading_session_progress.updated_at) IS NULL
                          OR julianday(excluded.updated_at)
                             > julianday(reading_session_progress.updated_at))"#,
                [user_id.into(), session_id.into(), surah.into(), furthest_ayah.into(), ts.into()],
            ))
            .await
            .map_err_to_response()?;
        Ok(res.rows_affected())
    }

    /// Snapshot rows, deterministic tie order `(updated_at, surah)` (EC-35).
    pub async fn list_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by(Column::UpdatedAt, Order::Asc)
            .order_by(Column::Surah, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }
}
