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
    /// That order is the eviction order.
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
    /// row (same device, still in use). `created_at` and the name are kept.
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
                am.last_seen_at = Set(now);
                am.retired_at = Set(None);
                am.expires_at = Set(expires_at);
                am.update(conn).await.map_err_to_response()
            }
            None => ActiveModel {
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
                created_at: Set(now),
                last_seen_at: Set(now),
                retired_at: Set(None),
                revoked_at: Set(None),
                expires_at: Set(expires_at),
                retention_class: Set(DEVICE_RETENTION_CLASS.to_string()),
            }
            .insert(conn)
            .await
            .map_err_to_response(),
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
