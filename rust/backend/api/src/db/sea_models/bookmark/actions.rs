use crate::error::{DbResult, DbResultExt};
use chrono::Utc;
use eq_sync_core::{canonical_json, compare_stamps, merge_bookmark, parse_stamp};
use sea_orm::{
    entity::prelude::*, DatabaseBackend, DatabaseTransaction, Order, QueryOrder, Statement,
};
use serde_json::{json, Value};
use std::cmp::Ordering;
use tracing::instrument;

use super::super::{bookmark_folder, sync_counter};
use super::{Column, Entity, Model};

/// Per-user live-bookmark quota. A new live row past the cap comes back `capped`
/// and the client keeps it dirty; updating an already-live id is always allowed.
pub const LIVE_BOOKMARK_QUOTA: u64 = 10_000;
pub const BOOKMARKS_DOMAIN: &str = "bookmarks";

/// One bookmark write: the live key is (folder, surah, ayah, end_surah,
/// end_ayah); `source_id` is context metadata, never a key part.
pub struct BookmarkWrite<'a> {
    pub id: &'a str,
    pub stamp: &'a str,
    pub folder_id: &'a str,
    pub source_id: Option<&'a str>,
    pub surah: i32,
    pub ayah: i32,
    pub end_surah: i32,
    pub end_ayah: i32,
    pub deleted: bool,
    pub updated_at: &'a str,
}

pub enum BookmarkOutcome {
    Applied(Box<Model>),
    Superseded(Box<Model>),
    Capped,
    SupersededBelowFloor,
}

fn verse_key(surah: i32, ayah: i32, end_surah: i32, end_ayah: i32) -> String {
    format!("{surah}:{ayah}-{end_surah}:{end_ayah}")
}

fn parse_key(key: &str) -> Option<(i32, i32, i32, i32)> {
    let (start, end) = key.split_once('-')?;
    let (surah, ayah) = start.split_once(':')?;
    let (end_surah, end_ayah) = end.split_once(':')?;
    Some((
        surah.parse().ok()?,
        ayah.parse().ok()?,
        end_surah.parse().ok()?,
        end_ayah.parse().ok()?,
    ))
}

/// The stored row as a merge register (contracts/sync.md).
pub fn registers_of(row: &Model) -> Value {
    json!({
        "id": row.id,
        "register": { "stamp": row.stamp, "value": {
            "context": { "sourceId": row.source_id },
            "deleted": row.deleted_at.is_some(),
            "folderId": row.folder_id,
            "key": verse_key(row.surah, row.ayah, row.end_surah, row.end_ayah),
        }},
    })
}

fn registers_of_write(w: &BookmarkWrite<'_>) -> Value {
    json!({
        "id": w.id,
        "register": { "stamp": w.stamp, "value": {
            "context": { "sourceId": w.source_id },
            "deleted": w.deleted,
            "folderId": w.folder_id,
            "key": verse_key(w.surah, w.ayah, w.end_surah, w.end_ayah),
        }},
    })
}

/// Later row outranks by stamp, then canonical bytes.
fn outranks(a: &Value, b: &Value) -> bool {
    let stamp = |row: &Value| row["register"]["stamp"].as_str().map(str::to_string);
    let (x, y) = (stamp(a), stamp(b));
    match compare_stamps(x.as_deref(), y.as_deref()) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => canonical_json(a).as_bytes() > canonical_json(b).as_bytes(),
    }
}

impl Entity {
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

    /// Keeps its stamp: at equal stamps a tombstone beats the live copy everywhere.
    pub(crate) async fn tombstone_keeping_stamp(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
    ) -> DbResult<()> {
        let rev = sync_counter::next_rev(conn, user_id, BOOKMARKS_DOMAIN).await?;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE bookmarks SET deleted_at = ?, rev = ? WHERE user_id = ? AND id = ?",
            [
                Utc::now().to_rfc3339().into(),
                rev.into(),
                user_id.into(),
                id.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        Ok(())
    }

    /// Read, merge, resolve the folder and key rules, write. Runs inside the sync
    /// transaction, and rows move only up the lattice, so replay changes nothing.
    #[instrument(skip(conn, w), fields(user_id = user_id, bookmark_id = w.id))]
    pub async fn merge_write(
        conn: &DatabaseTransaction,
        user_id: i32,
        w: &BookmarkWrite<'_>,
    ) -> DbResult<BookmarkOutcome> {
        let existing = Self::load(conn, user_id, w.id).await?;
        let mut incoming = registers_of_write(w);
        let mut merged = match &existing {
            Some(row) => {
                if row.scrubbed_at.is_some() {
                    return Ok(BookmarkOutcome::Superseded(Box::new(row.clone())));
                }
                let current = registers_of(row);
                sync_counter::absorb_rewritten_replays(&current, &mut incoming);
                let merged = merge_bookmark(&current, &incoming);
                if canonical_json(&merged) == canonical_json(&current) {
                    return Ok(BookmarkOutcome::Superseded(Box::new(row.clone())));
                }
                merged
            }
            None => {
                let floor = sync_counter::floor_ms(conn, user_id, BOOKMARKS_DOMAIN).await?;
                let ms = parse_stamp(w.stamp).map_or(0, |p| p.ms as i64);
                if floor > 0 && ms < floor {
                    return Ok(BookmarkOutcome::SupersededBelowFloor);
                }
                if !w.deleted
                    && Self::count_live_for_user(conn, user_id).await? >= LIVE_BOOKMARK_QUOTA
                {
                    return Ok(BookmarkOutcome::Capped);
                }
                incoming
            }
        };

        let value = &merged["register"]["value"];
        let mut deleted = value["deleted"].as_bool().unwrap_or(false);
        let mut folder_id = value["folderId"].as_str().unwrap_or_default().to_string();
        let key = value["key"].as_str().unwrap_or_default().to_string();
        let source_id = value["context"]["sourceId"].as_str().map(str::to_string);
        let stamp = merged["register"]["stamp"].as_str().map(str::to_string);
        let (surah, ayah, end_surah, end_ayah) =
            parse_key(&key).unwrap_or((w.surah, w.ayah, w.end_surah, w.end_ayah));

        // Folder rules: an unknown or foreign id files in the default folder. A
        // deleted folder kills older bookmarks and re-homes newer ones.
        let default_id = bookmark_folder::default_folder_id(user_id);
        match bookmark_folder::Entity::load(conn, user_id, &folder_id).await? {
            None => folder_id = default_id.clone(),
            Some(folder) if folder.deleted_at.is_some() => {
                let order = compare_stamps(stamp.as_deref(), folder.deleted_stamp.as_deref());
                if order == Ordering::Greater {
                    folder_id = default_id.clone();
                } else {
                    deleted = true;
                }
            }
            Some(_) => {}
        }
        merged["register"]["value"]["folderId"] = json!(folder_id);
        merged["register"]["value"]["deleted"] = json!(deleted);

        // Two ids on one key: the lower stamp becomes a tombstone.
        if !deleted {
            let rivals = Self::find()
                .filter(Column::UserId.eq(user_id))
                .filter(Column::FolderId.eq(folder_id.as_str()))
                .filter(Column::Surah.eq(surah))
                .filter(Column::Ayah.eq(ayah))
                .filter(Column::EndSurah.eq(end_surah))
                .filter(Column::EndAyah.eq(end_ayah))
                .filter(Column::Id.ne(w.id))
                .filter(Column::DeletedAt.is_null())
                .all(conn)
                .await
                .map_err_to_response()?;
            for rival in rivals {
                if outranks(&registers_of(&rival), &merged) {
                    deleted = true;
                } else {
                    Self::tombstone_keeping_stamp(conn, user_id, &rival.id).await?;
                }
            }
        }

        let rev = sync_counter::next_rev(conn, user_id, BOOKMARKS_DOMAIN).await?;
        let deleted_at = deleted.then(|| Utc::now().to_rfc3339());
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO bookmarks (user_id, id, folder_id, source_id, surah, ayah,
                   end_surah, end_ayah, created_at, updated_at, deleted_at, stamp, rev)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (user_id, id) DO UPDATE SET
                   folder_id = excluded.folder_id, source_id = excluded.source_id,
                   surah = excluded.surah, ayah = excluded.ayah,
                   end_surah = excluded.end_surah, end_ayah = excluded.end_ayah,
                   updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                   stamp = excluded.stamp, rev = excluded.rev"#,
            [
                user_id.into(),
                w.id.into(),
                folder_id.into(),
                source_id.into(),
                surah.into(),
                ayah.into(),
                end_surah.into(),
                end_ayah.into(),
                w.updated_at.into(),
                w.updated_at.into(),
                deleted_at.into(),
                stamp.into(),
                rev.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        let stored = Self::load(conn, user_id, w.id).await?;
        stored
            .map(|row| BookmarkOutcome::Applied(Box::new(row)))
            .ok_or_else(|| crate::error::ErrorResponse::new(crate::error::ErrorCode::QueryError))
    }

    /// Bookmarks in a folder that just got deleted: older die, newer move to the
    /// default folder, where the key rule still applies.
    pub async fn apply_folder_delete(
        conn: &DatabaseTransaction,
        user_id: i32,
        folder_id: &str,
        delete_stamp: Option<&str>,
    ) -> DbResult<()> {
        let default_id = bookmark_folder::default_folder_id(user_id);
        let rows = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::FolderId.eq(folder_id))
            .filter(Column::DeletedAt.is_null())
            .all(conn)
            .await
            .map_err_to_response()?;
        for row in rows {
            if compare_stamps(row.stamp.as_deref(), delete_stamp) != Ordering::Greater {
                Self::tombstone_keeping_stamp(conn, user_id, &row.id).await?;
                continue;
            }
            let rival = Self::find()
                .filter(Column::UserId.eq(user_id))
                .filter(Column::FolderId.eq(default_id.as_str()))
                .filter(Column::Surah.eq(row.surah))
                .filter(Column::Ayah.eq(row.ayah))
                .filter(Column::EndSurah.eq(row.end_surah))
                .filter(Column::EndAyah.eq(row.end_ayah))
                .filter(Column::DeletedAt.is_null())
                .one(conn)
                .await
                .map_err_to_response()?;
            if let Some(rival) = rival {
                if outranks(&registers_of(&rival), &registers_of(&row)) {
                    Self::tombstone_keeping_stamp(conn, user_id, &row.id).await?;
                    continue;
                }
                Self::tombstone_keeping_stamp(conn, user_id, &rival.id).await?;
            }
            let rev = sync_counter::next_rev(conn, user_id, BOOKMARKS_DOMAIN).await?;
            conn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE bookmarks SET folder_id = ?, rev = ? WHERE user_id = ? AND id = ?",
                [
                    default_id.as_str().into(),
                    rev.into(),
                    user_id.into(),
                    row.id.as_str().into(),
                ],
            ))
            .await
            .map_err_to_response()?;
        }
        Ok(())
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

    /// Live (not tombstoned) rows for one account. Full-pull snapshot guard (A7).
    pub async fn count_live_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::DeletedAt.is_null())
            .count(conn)
            .await
            .map_err_to_response()
    }
}
