//! `reading_sessions` + `reading_progress`. A session is one continuous reading
//! thread with its cursor inline. Rows merge through `eq-sync-core` register by
//! register (contracts/sync.md); stamps decide, `updated_at` is display only.
//! Progress is account-level and MAX-merged so it never regresses.

use crate::error::{DbResult, DbResultExt};
use chrono::Utc;
use eq_sync_core::{canonical_json, format_stamp, merge_thread};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseBackend, DatabaseTransaction, QueryResult, Statement};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::instrument;

use super::sync_counter;

/// Per-user live-row quota. A NEW thread past the cap is refused (surfaced
/// via the round's `skipped` count); updating an existing id is always fine.
const LIVE_SESSION_QUOTA: u64 = 10_000;
/// Open threads untouched this long turn stale (derived, never stored).
pub const STALE_AFTER_DAYS: i64 = 30;
pub const SESSIONS_DOMAIN: &str = "sessions";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reading_sessions")]
pub struct Model {
    /// Composite PK (user_id, id): the same client-minted id under two
    /// accounts is two threads, never a collision.
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// index | search | bookmark | card | snippet | link
    pub origin: String,
    /// Origin detail, e.g. the card id that opened the thread.
    pub origin_ref: Option<String>,
    /// What the thread reads: `surah:N`, `page:N`, `juz:N`, `hizb:N`,
    /// `snippet:slug`, `range:S:A-S:A`.
    pub scope: String,
    /// `S:A` end of a bounded scope; reaching it completes the thread.
    pub bound_end: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    /// phone | tablet | desktop — the handoff label.
    pub device: String,
    pub started_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub ended_at: Option<DateTimeWithTimeZone>,
    /// completed | restarted | dismissed
    pub end_reason: Option<String>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
    pub identity_stamp: Option<String>,
    pub cursor_stamp: Option<String>,
    pub lifecycle_stamp: Option<String>,
    /// open | ended | deleted
    pub state: String,
    pub device_id: Option<String>,
    pub rev: i64,
    /// Set once a tombstone's content is gone. Such a row ignores further writes.
    pub scrubbed_at: Option<String>,
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

/// Wire row (camelCase). user_id never leaves the server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
    pub id: String,
    pub origin: String,
    pub origin_ref: Option<String>,
    pub scope: String,
    pub bound_end: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    pub local_page: Option<i32>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    pub device: String,
    pub device_id: Option<String>,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
    pub end_reason: Option<String>,
    pub state: String,
    pub identity_stamp: Option<String>,
    pub cursor_stamp: Option<String>,
    pub lifecycle_stamp: Option<String>,
    pub rev: i64,
}

impl SessionListItem {
    fn from_row(row: &QueryResult) -> Result<Self, DbErr> {
        Ok(Self {
            id: row.try_get("", "id")?,
            origin: row.try_get("", "origin")?,
            origin_ref: row.try_get("", "origin_ref")?,
            scope: row.try_get("", "scope")?,
            bound_end: row.try_get("", "bound_end")?,
            surah: row.try_get("", "surah")?,
            ayah: row.try_get("", "ayah")?,
            local_page: row.try_get("", "local_page")?,
            ratio: row.try_get("", "ratio")?,
            source_id: row.try_get("", "source_id")?,
            device: row.try_get("", "device")?,
            device_id: row.try_get("", "device_id")?,
            started_at: row.try_get("", "started_at")?,
            updated_at: row.try_get("", "updated_at")?,
            ended_at: row.try_get("", "ended_at")?,
            end_reason: row.try_get("", "end_reason")?,
            state: row.try_get("", "state")?,
            identity_stamp: row.try_get("", "identity_stamp")?,
            cursor_stamp: row.try_get("", "cursor_stamp")?,
            lifecycle_stamp: row.try_get("", "lifecycle_stamp")?,
            rev: row.try_get("", "rev")?,
        })
    }

    /// The row as merge registers (contracts/sync.md).
    pub fn registers(&self) -> Value {
        json!({
            "id": self.id,
            "identity": { "stamp": self.identity_stamp, "value": {
                "origin": self.origin, "originRef": self.origin_ref, "scope": self.scope,
                "boundEnd": self.bound_end, "startedAt": self.started_at,
            }},
            "cursor": { "stamp": self.cursor_stamp, "value": {
                "surah": self.surah, "ayah": self.ayah, "localPage": self.local_page,
                "ratio": self.ratio, "sourceId": self.source_id, "device": self.device,
                "deviceId": self.device_id,
            }},
            "lifecycle": { "stamp": self.lifecycle_stamp, "value": {
                "state": self.state, "endedAt": self.ended_at, "endReason": self.end_reason,
            }},
        })
    }

    /// What a pull delivers. A tombstone is id, stamps, state and rev only.
    pub fn to_wire(&self) -> Value {
        if self.state == "deleted" {
            return json!({
                "id": self.id, "state": self.state, "rev": self.rev,
                "identityStamp": self.identity_stamp, "cursorStamp": self.cursor_stamp,
                "lifecycleStamp": self.lifecycle_stamp,
            });
        }
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    pub fn stamps(&self) -> Value {
        json!({
            "identity": self.identity_stamp, "cursor": self.cursor_stamp,
            "lifecycle": self.lifecycle_stamp,
        })
    }
}

/// Per-surah furthest ayah (wire camelCase).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressListItem {
    pub surah: i32,
    pub furthest_ayah: i32,
    pub updated_at: String,
    pub rev: i64,
}

/// Result of writing one row.
pub enum ThreadOutcome {
    /// The merge changed the stored row.
    Applied(Box<SessionListItem>),
    /// Stored row already holds this state or better.
    Superseded(Box<SessionListItem>),
    /// A new row past the live quota. Nothing stored; the client keeps it dirty.
    Capped,
    /// A new row older than the tombstone floor: it may have been deleted long ago.
    SupersededBelowFloor,
    /// A new row with no identity or cursor register cannot be created.
    Incomplete,
}

const SELECT_COLUMNS: &str = "id, origin, origin_ref, scope, bound_end, surah, ayah, local_page, \
     ratio, source_id, device, device_id, started_at, updated_at, ended_at, end_reason, state, \
     identity_stamp, cursor_stamp, lifecycle_stamp, rev";

/// Below this cursor stamp an open thread is stale.
fn stale_cutoff_stamp() -> String {
    let cutoff = Utc::now() - chrono::Duration::days(STALE_AFTER_DAYS);
    format_stamp(cutoff.timestamp_millis().max(0) as u64, 0, "")
}

fn text(register: &Value, field: &str) -> Option<String> {
    register["value"][field].as_str().map(str::to_string)
}

fn stamp(register: &Value) -> Option<String> {
    register["stamp"].as_str().map(str::to_string)
}

fn stamp_ms(register: &Value) -> i64 {
    stamp(register)
        .and_then(|s| eq_sync_core::parse_stamp(&s))
        .map_or(0, |p| p.ms as i64)
}

impl Entity {
    async fn load<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        id: &str,
    ) -> DbResult<Option<SessionListItem>> {
        let sql =
            format!("SELECT {SELECT_COLUMNS} FROM reading_sessions WHERE user_id = ? AND id = ?");
        let row = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [user_id.into(), id.into()],
            ))
            .await
            .map_err_to_response()?;
        row.as_ref()
            .map(SessionListItem::from_row)
            .transpose()
            .map_err_to_response()
    }

    /// Read, merge, write inside the sync transaction. `incoming` holds only the
    /// registers this client wrote; an absent register changes nothing.
    #[instrument(skip(conn, incoming, updated_at), fields(user_id = user_id, session_id = id))]
    pub async fn merge_write(
        conn: &DatabaseTransaction,
        user_id: i32,
        id: &str,
        incoming: &Value,
        updated_at: &str,
    ) -> DbResult<ThreadOutcome> {
        let existing = Self::load(conn, user_id, id).await?;
        let merged = match &existing {
            Some(row) => {
                if sync_counter::is_scrubbed(conn, "reading_sessions", user_id, id).await? {
                    return Ok(ThreadOutcome::Superseded(Box::new(row.clone())));
                }
                let current = row.registers();
                let mut incoming = incoming.clone();
                sync_counter::absorb_rewritten_replays(&current, &mut incoming);
                let merged = merge_thread(&current, &incoming);
                if canonical_json(&merged) == canonical_json(&current) {
                    return Ok(ThreadOutcome::Superseded(Box::new(row.clone())));
                }
                merged
            }
            None => {
                if incoming.get("identity").is_none() || incoming.get("cursor").is_none() {
                    return Ok(ThreadOutcome::Incomplete);
                }
                let floor = sync_counter::floor_ms(conn, user_id, SESSIONS_DOMAIN).await?;
                if floor > 0 && stamp_ms(&incoming["cursor"]) < floor {
                    return Ok(ThreadOutcome::SupersededBelowFloor);
                }
                if Self::count_live_for_user(conn, user_id).await? >= LIVE_SESSION_QUOTA {
                    return Ok(ThreadOutcome::Capped);
                }
                let mut fresh = incoming.clone();
                if fresh.get("lifecycle").is_none() {
                    fresh["lifecycle"] = json!({ "stamp": null, "value": {
                        "state": "open", "endedAt": null, "endReason": null,
                    }});
                }
                fresh
            }
        };

        let rev = sync_counter::next_rev(conn, user_id, SESSIONS_DOMAIN).await?;
        let (identity, cursor, lifecycle) =
            (&merged["identity"], &merged["cursor"], &merged["lifecycle"]);
        let state = text(lifecycle, "state").unwrap_or_else(|| "open".to_string());
        let deleted_at = (state == "deleted").then(|| Utc::now().to_rfc3339());
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO reading_sessions (user_id, id, origin, origin_ref, scope, bound_end,
                   surah, ayah, local_page, ratio, source_id, device, device_id,
                   started_at, updated_at, ended_at, end_reason, deleted_at, state,
                   identity_stamp, cursor_stamp, lifecycle_stamp, rev)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (user_id, id) DO UPDATE SET
                   origin = excluded.origin, origin_ref = excluded.origin_ref,
                   scope = excluded.scope, bound_end = excluded.bound_end,
                   surah = excluded.surah, ayah = excluded.ayah,
                   local_page = excluded.local_page, ratio = excluded.ratio,
                   source_id = excluded.source_id, device = excluded.device,
                   device_id = excluded.device_id, started_at = excluded.started_at,
                   updated_at = excluded.updated_at, ended_at = excluded.ended_at,
                   end_reason = excluded.end_reason, deleted_at = excluded.deleted_at,
                   state = excluded.state, identity_stamp = excluded.identity_stamp,
                   cursor_stamp = excluded.cursor_stamp,
                   lifecycle_stamp = excluded.lifecycle_stamp, rev = excluded.rev"#,
            [
                user_id.into(),
                id.into(),
                text(identity, "origin").into(),
                text(identity, "originRef").into(),
                text(identity, "scope").into(),
                text(identity, "boundEnd").into(),
                cursor["value"]["surah"].as_i64().unwrap_or(1).into(),
                cursor["value"]["ayah"].as_i64().unwrap_or(1).into(),
                cursor["value"]["localPage"].as_i64().into(),
                cursor["value"]["ratio"].as_f64().into(),
                text(cursor, "sourceId").into(),
                text(cursor, "device").into(),
                text(cursor, "deviceId").into(),
                text(identity, "startedAt").into(),
                updated_at.into(),
                text(lifecycle, "endedAt").into(),
                text(lifecycle, "endReason").into(),
                deleted_at.into(),
                state.into(),
                stamp(identity).into(),
                stamp(cursor).into(),
                stamp(lifecycle).into(),
                rev.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        let stored = Self::load(conn, user_id, id).await?;
        stored
            .map(|row| ThreadOutcome::Applied(Box::new(row)))
            .ok_or_else(|| crate::error::ErrorResponse::new(crate::error::ErrorCode::QueryError))
    }

    /// One page of a pull, in `rev` order. A full pull (`full`) sends open threads
    /// only; history comes from `/archive`, so a large account stays cheap.
    pub async fn page_since<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        after: i64,
        full: bool,
        limit: u64,
    ) -> DbResult<Vec<SessionListItem>> {
        let (filter, values): (&str, Vec<sea_orm::Value>) = if full {
            (
                "AND state = 'open' AND COALESCE(cursor_stamp, '') >= ?",
                vec![user_id.into(), after.into(), stale_cutoff_stamp().into()],
            )
        } else {
            ("", vec![user_id.into(), after.into()])
        };
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM reading_sessions \
             WHERE user_id = ? AND rev > ? {filter} ORDER BY rev ASC LIMIT {limit}"
        );
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                values,
            ))
            .await
            .map_err_to_response()?;
        rows.iter()
            .map(SessionListItem::from_row)
            .collect::<Result<_, _>>()
            .map_err_to_response()
    }

    /// Archive page: threads that ended or went stale, newest first, keyset on
    /// `(cursor_stamp, id)`.
    pub async fn list_archive<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        before: Option<(&str, &str)>,
        limit: u64,
    ) -> DbResult<Vec<SessionListItem>> {
        let mut values: Vec<sea_orm::Value> = vec![user_id.into(), stale_cutoff_stamp().into()];
        let cursor = match before {
            Some((ts, id)) => {
                values.extend([ts.into(), ts.into(), id.into()]);
                "AND (COALESCE(cursor_stamp, '') < ? OR (COALESCE(cursor_stamp, '') = ? AND id < ?))"
            }
            None => "",
        };
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM reading_sessions \
             WHERE user_id = ? AND state != 'deleted' \
               AND (state = 'ended' OR COALESCE(cursor_stamp, '') < ?) {cursor} \
             ORDER BY COALESCE(cursor_stamp, '') DESC, id DESC LIMIT {limit}"
        );
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                values,
            ))
            .await
            .map_err_to_response()?;
        rows.iter()
            .map(SessionListItem::from_row)
            .collect::<Result<_, _>>()
            .map_err_to_response()
    }

    /// MAX-merge one surah's furthest ayah. Returns the new row only if it changed.
    pub async fn upsert_progress(
        conn: &DatabaseTransaction,
        user_id: i32,
        surah: i32,
        furthest_ayah: i32,
        updated_at: &str,
    ) -> DbResult<bool> {
        let current = conn
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT furthest_ayah FROM reading_progress WHERE user_id = ? AND surah = ?",
                [user_id.into(), surah.into()],
            ))
            .await
            .map_err_to_response()?;
        if let Some(row) = current {
            let stored: i32 = row.try_get("", "furthest_ayah").map_err_to_response()?;
            if stored >= furthest_ayah {
                return Ok(false);
            }
        }
        let rev = sync_counter::next_rev(conn, user_id, SESSIONS_DOMAIN).await?;
        conn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"INSERT INTO reading_progress (user_id, surah, furthest_ayah, updated_at, rev)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT (user_id, surah) DO UPDATE SET
                   furthest_ayah = excluded.furthest_ayah,
                   updated_at = excluded.updated_at,
                   rev = excluded.rev"#,
            [
                user_id.into(),
                surah.into(),
                furthest_ayah.into(),
                updated_at.into(),
                rev.into(),
            ],
        ))
        .await
        .map_err_to_response()?;
        Ok(true)
    }

    /// Progress rows past `after`, in `rev` order. All of them on a full pull too.
    pub async fn progress_since<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        after: i64,
        limit: u64,
    ) -> DbResult<Vec<ProgressListItem>> {
        let sql = format!(
            "SELECT surah, furthest_ayah, updated_at, rev FROM reading_progress \
             WHERE user_id = ? AND rev > ? ORDER BY rev ASC LIMIT {limit}"
        );
        let rows = conn
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [user_id.into(), after.into()],
            ))
            .await
            .map_err_to_response()?;
        rows.iter()
            .map(|r| {
                Ok(ProgressListItem {
                    surah: r.try_get("", "surah")?,
                    furthest_ayah: r.try_get("", "furthest_ayah")?,
                    updated_at: r.try_get("", "updated_at")?,
                    rev: r.try_get("", "rev")?,
                })
            })
            .collect::<Result<_, DbErr>>()
            .map_err_to_response()
    }

    async fn count_live_for_user(conn: &DatabaseTransaction, user_id: i32) -> DbResult<u64> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::State.ne("deleted"))
            .count(conn)
            .await
            .map_err_to_response()
    }
}
