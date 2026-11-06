use chrono::Utc;
use sea_orm::{entity::prelude::*, Order, QueryOrder, Set};

use crate::error::{DbResult, DbResultExt};

use super::{
    ActiveModel, Column, Entity, Model, NewUserDevice, DEVICE_RETENTION_CLASS,
    DEVICE_RETENTION_DAYS,
};

fn utc_now() -> DateTimeWithTimeZone {
    Utc::now().fixed_offset()
}

impl Entity {
    pub async fn find_row<C: ConnectionTrait>(
        conn: &C,
        user_id: i32,
        id: &str,
    ) -> DbResult<Option<Model>> {
        Self::find_by_id((user_id, id.to_string()))
            .one(conn)
            .await
            .map_err_to_response()
    }

    /// Rows that are neither retired nor revoked, oldest `last_seen_at` first.
    /// That order is the eviction order. Merged rows are always retired.
    pub async fn live_for_user<C: ConnectionTrait>(conn: &C, user_id: i32) -> DbResult<Vec<Model>> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::RetiredAt.is_null())
            .filter(Column::RevokedAt.is_null())
            .order_by(Column::LastSeenAt, Order::Asc)
            .all(conn)
            .await
            .map_err_to_response()
    }

    /// Insert a new row, or refresh an existing one. A register revives a retired
    /// row (same device, still in use). `created_at` and the name are kept. Key
    /// fields change only when the register carried a verified key; a register
    /// without one keeps the stored key and only updates `key_store`.
    pub async fn register<C: ConnectionTrait>(
        conn: &C,
        new: NewUserDevice,
        existing: Option<Model>,
    ) -> DbResult<Model> {
        let now = utc_now();
        let expires_at = now.timestamp() + DEVICE_RETENTION_DAYS * 86_400;
        match existing {
            Some(row) => {
                let mut am: ActiveModel = row.into();
                am.platform = Set(new.platform);
                am.client = Set(new.client);
                am.form_factor = Set(new.form_factor);
                am.browser = Set(new.browser);
                am.os_major = Set(new.os_major);
                am.app_version = Set(new.app_version);
                am.key_store = Set(new.key_store);
                if let Some(key) = new.key {
                    am.public_key = Set(Some(key.public_key));
                    am.key_thumbprint = Set(Some(key.thumbprint));
                }
                if let Some(hint) = new.continuity_hint {
                    am.continuity_hint = Set(Some(hint));
                }
                am.last_seen_at = Set(now);
                am.retired_at = Set(None);
                am.expires_at = Set(expires_at);
                am.update(conn).await.map_err_to_response()
            }
            None => {
                let (public_key, key_thumbprint) = match new.key {
                    Some(key) => (Some(key.public_key), Some(key.thumbprint)),
                    None => (None, None),
                };
                ActiveModel {
                    user_id: Set(new.user_id),
                    id: Set(new.id),
                    display_name: Set(None),
                    platform: Set(new.platform),
                    client: Set(new.client),
                    form_factor: Set(new.form_factor),
                    browser: Set(new.browser),
                    os_major: Set(new.os_major),
                    app_version: Set(new.app_version),
                    name_stamp: Set(None),
                    merged_into: Set(None),
                    public_key: Set(public_key),
                    key_thumbprint: Set(key_thumbprint),
                    key_store: Set(new.key_store),
                    continuity_hint: Set(new.continuity_hint),
                    hint_conflict: Set(false),
                    merged_key_thumbprint: Set(None),
                    created_at: Set(now),
                    last_seen_at: Set(now),
                    retired_at: Set(None),
                    revoked_at: Set(None),
                    expires_at: Set(expires_at),
                    retention_class: Set(DEVICE_RETENTION_CLASS.to_string()),
                }
                .insert(conn)
                .await
                .map_err_to_response()
            }
        }
    }

    pub async fn set_name<C: ConnectionTrait>(
        conn: &C,
        row: Model,
        name: String,
        stamp: String,
    ) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.display_name = Set(Some(name));
        am.name_stamp = Set(Some(stamp));
        am.update(conn).await.map_err_to_response()
    }

    pub async fn retire<C: ConnectionTrait>(conn: &C, row: Model) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.retired_at = Set(Some(utc_now()));
        am.update(conn).await.map_err_to_response()
    }

    /// Rung 3: this row is the old id of a device that re-registered as `into`.
    /// Retired, so it leaves the list, and it keeps its key for shared-hint recovery.
    pub async fn mark_merged<C: ConnectionTrait>(
        conn: &C,
        row: Model,
        into: &str,
    ) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.merged_into = Set(Some(into.to_string()));
        am.retired_at = Set(Some(utc_now()));
        am.update(conn).await.map_err_to_response()
    }

    /// Shared-hint recovery: the merged row comes back as its own device.
    pub async fn restore_merged<C: ConnectionTrait>(
        conn: &C,
        row: Model,
        conflict: bool,
    ) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.merged_into = Set(None);
        am.retired_at = Set(None);
        if conflict {
            am.hint_conflict = Set(true);
        }
        am.update(conn).await.map_err_to_response()
    }

    pub async fn set_hint_conflict<C: ConnectionTrait>(conn: &C, row: Model) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.hint_conflict = Set(true);
        am.update(conn).await.map_err_to_response()
    }

    pub async fn set_merged_key_thumbprint<C: ConnectionTrait>(
        conn: &C,
        row: Model,
        thumbprint: String,
    ) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.merged_key_thumbprint = Set(Some(thumbprint));
        am.update(conn).await.map_err_to_response()
    }

    pub async fn revoke<C: ConnectionTrait>(conn: &C, row: Model) -> DbResult<Model> {
        let mut am: ActiveModel = row.into();
        am.revoked_at = Set(Some(utc_now()));
        am.update(conn).await.map_err_to_response()
    }
}

/// Ids to retire so the live count fits `cap`. `live` is oldest-idle first. The
/// caller is never chosen, even when it is the oldest row.
pub fn cap_victims(live: &[Model], caller_id: &str, cap: usize) -> Vec<String> {
    let excess = live.len().saturating_sub(cap);
    live.iter()
        .filter(|row| row.id != caller_id)
        .take(excess)
        .map(|row| row.id.clone())
        .collect()
}
