use crate::error::{DbResult, DbResultExt};
use chrono::Utc;
use eq_sync_core::{canonical_json, merge_folder, parse_stamp};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use serde_json::{json, Value};
use tracing::instrument;
use uuid::Uuid;

use super::super::{bookmark, sync_counter};
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

/// Seed name; the user may rename it (its NULL stamp ranks below any real write).
pub const DEFAULT_FOLDER_NAME: &str = "Bookmarks";

/// Seed timestamp: older than any real client write, so a rename always wins.
const SEED_TS: &str = "1970-01-01T00:00:00.000000Z";

/// Live-folder quota per user. A new live folder past the cap comes back `capped`;
/// a rename at the cap still works.
const LIVE_FOLDER_QUOTA: u64 = 200;

/// Deterministic default-folder id for one account.
pub fn default_folder_id(user_id: i32) -> String {
    Uuid::new_v5(
        &DEFAULT_FOLDER_NAMESPACE,
        format!("user:{user_id}").as_bytes(),
    )
    .to_string()
}

pub enum FolderOutcome {
    Applied(Box<Model>),
    Superseded(Box<Model>),
    Capped,
    SupersededBelowFloor,
    Incomplete,
}

/// The stored row as merge registers (contracts/sync.md).
pub fn registers_of(row: &Model) -> Value {
    json!({
        "id": row.id,
        "isDefault": row.is_default,
        "name": { "stamp": row.name_stamp, "value": row.name },
        "position": { "stamp": row.position_stamp, "value": row.position },
        "deleted": { "stamp": row.deleted_stamp, "value": row.deleted_at.is_some() },
    })
}

impl Entity {
    /// Make sure the account's default folder exists and is live. Runs at the
    /// head of every sync round, before any mutation resolves a folder id.
    pub async fn ensure_default(conn: &DatabaseTransaction, user_id: i32) -> DbResult<()> {
        let id = default_folder_id(user_id);
        if Self::load(conn, user_id, &id).await?.is_some() {
            return Ok(());
        }
        let rev = sync_counter::next_rev(conn, user_id, bookmark::BOOKMARKS_DOMAIN).await?;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO bookmark_folders (user_id, id, name, is_default, created_at, updated_at, rev)
               VALUES (?, ?, ?, 1, ?, ?, ?)"#,
            [
                user_id.into(),
                id.into(),
                DEFAULT_FOLDER_NAME.into(),
                SEED_TS.into(),
                SEED_TS.into(),
                rev.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        Ok(())
    }

    pub async fn load(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
    ) -> DbResult<Option<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(id))
            .one(conn)
            .await
            .map_err_to_response()
    }

    /// Read, merge register by register, write. A rename and a move on two devices
    /// both survive. `is_default` is never client-writable.
    #[instrument(skip(conn, incoming), fields(user_id = user_id, folder_id = id))]
    pub async fn merge_write(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        incoming: &Value,
        updated_at: &str,
    ) -> DbResult<FolderOutcome> {
        let is_default = id == default_folder_id(user_id);
        let mut write = incoming.clone();
        write["isDefault"] = json!(is_default);
        let existing = Self::load(conn, user_id, id).await?;
        let was_deleted = existing
            .as_ref()
            .is_some_and(|row| row.deleted_at.is_some());
        let merged = match &existing {
            Some(row) => {
                if row.scrubbed_at.is_some() {
                    return Ok(FolderOutcome::Superseded(Box::new(row.clone())));
                }
                let current = registers_of(row);
                sync_counter::absorb_rewritten_replays(&current, &mut write);
                let merged = merge_folder(&current, &write);
                if canonical_json(&merged) == canonical_json(&current) {
                    return Ok(FolderOutcome::Superseded(Box::new(row.clone())));
                }
                merged
            }
            None => {
                if write.get("name").is_none() {
                    return Ok(FolderOutcome::Incomplete);
                }
                let stamp = write["name"]["stamp"].as_str().unwrap_or_default();
                let ms = parse_stamp(stamp).map_or(0, |p| p.ms as i64);
                let floor =
                    sync_counter::floor_ms(conn, user_id, bookmark::BOOKMARKS_DOMAIN).await?;
                if floor > 0 && ms < floor {
                    return Ok(FolderOutcome::SupersededBelowFloor);
                }
                if Self::count_live_for_user(conn, user_id).await? >= LIVE_FOLDER_QUOTA {
                    return Ok(FolderOutcome::Capped);
                }
                write
            }
        };

        let deleted = merged["deleted"]["value"].as_bool().unwrap_or(false);
        let deleted_stamp = merged["deleted"]["stamp"].as_str().map(str::to_string);
        let rev = sync_counter::next_rev(conn, user_id, bookmark::BOOKMARKS_DOMAIN).await?;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO bookmark_folders (user_id, id, name, is_default, position, created_at,
                   updated_at, deleted_at, name_stamp, position_stamp, deleted_stamp, rev)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (user_id, id) DO UPDATE SET
                   name = excluded.name, position = excluded.position,
                   updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                   name_stamp = excluded.name_stamp, position_stamp = excluded.position_stamp,
                   deleted_stamp = excluded.deleted_stamp, rev = excluded.rev"#,
            [
                user_id.into(),
                id.into(),
                merged["name"]["value"].as_str().unwrap_or_default().into(),
                is_default.into(),
                merged["position"]["value"].as_i64().into(),
                updated_at.into(),
                updated_at.into(),
                deleted.then(|| Utc::now().to_rfc3339()).into(),
                merged["name"]["stamp"].as_str().into(),
                merged["position"]["stamp"].as_str().into(),
                deleted_stamp.clone().into(),
                rev.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        if deleted && !was_deleted {
            bookmark::Entity::apply_folder_delete(conn, user_id, id, deleted_stamp.as_deref())
                .await?;
        }
        let stored = Self::load(conn, user_id, id).await?;
        stored
            .map(|row| FolderOutcome::Applied(Box::new(row)))
            .ok_or_else(|| crate::error::ErrorResponse::new(crate::error::ErrorCode::QueryError))
    }

    /// One pull page by `rev`. A full pull sends live rows only.
    pub async fn page_since(
        conn: &DatabaseTransaction,
        user_id: i32,
        after: i64,
        full: bool,
        limit: u64,
    ) -> DbResult<Vec<Model>> {
        let mut query = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Rev.gt(after));
        if full {
            query = query.filter(Column::DeletedAt.is_null());
        }
        sea_orm::QuerySelect::limit(query.order_by(Column::Rev, Order::Asc), limit)
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
