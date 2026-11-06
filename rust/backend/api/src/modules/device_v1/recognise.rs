//! Recognition ladder (devices.md D3, rules 6 to 8). Everything here stays inside one
//! account: every read and write takes `user_id`. The first rung that matches wins.
//! The returned id is the one the client must keep.
//!
//! Rules of the ladder, in order:
//! 1. Known id, verified key matches, hint matches or absent: that row.
//!    A merged row whose old key comes back with the same hint is a shared hint
//!    (rule 7): both rows get `hint_conflict`, and the old row is restored.
//! 2. The session already holds a live row and the id is unknown or the key was
//!    lost: that row, new key, and the client takes its id.
//! 3. Unknown id, verified key, hint matches a live row with no conflict: a new row
//!    for the new id. The old row is merged into it and retired.
//! 4. Otherwise a new row. Older rows of the same platform, client, browser and
//!    form factor with no live session are retired.
//!
//! A known id that matches none of these is refused with 409. The client then mints
//! a new id and key and registers again, keeping its local data.

use sea_orm::{DbConn, EntityTrait};

use crate::{
    db::sea_models::{
        user_device::{self, Model, NewUserDevice},
        user_session,
    },
    error::{DbResultExt, ErrorCode, ErrorResponse},
};

fn refused() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::DuplicateEntry)
        .with_message("Device key or hint does not match. Register a new device id.")
}

fn removed() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::DuplicateEntry)
        .with_message("Device was removed. Register a new device id.")
}

/// A row the session can prove it holds: live, not merged, not revoked.
fn is_live(row: &Model) -> bool {
    row.retired_at.is_none() && row.revoked_at.is_none() && row.merged_into.is_none()
}

/// Canonical id for this register. `session_row` is the row the current session is
/// bound to, if any. `live_devices` are the ids with a live session for the account.
pub async fn recognise(
    db: &DbConn,
    new: NewUserDevice,
    session_row: Option<Model>,
    live_devices: &[String],
) -> Result<String, ErrorResponse> {
    let session_row = session_row.filter(is_live);
    let known = user_device::Entity::find_row(db, new.user_id, &new.id).await?;
    if let Some(row) = known {
        if row.revoked_at.is_some() {
            return Err(removed());
        }
        return match known_row(db, &new, row, session_row.as_ref()).await? {
            Some(id) => Ok(id),
            None => Err(refused()),
        };
    }
    if let Some(row) = session_row {
        // Rung 2: the session is the proof. The client takes this row's id.
        let id = row.id.clone();
        let mut same = new;
        same.id = id.clone();
        user_device::Entity::register(db, same, Some(row)).await?;
        return Ok(id);
    }
    if let Some(id) = merge_by_hint(db, &new).await? {
        return Ok(id);
    }
    retire_stale_peers(db, &new, live_devices).await?;
    user_device::Entity::register(db, new.clone(), None).await?;
    Ok(new.id)
}

/// Rungs 1 and 2 and rule 7, for an id the account already has. `None` when no rung
/// applies and the register must be refused.
async fn known_row(
    db: &DbConn,
    new: &NewUserDevice,
    row: Model,
    session_row: Option<&Model>,
) -> Result<Option<String>, ErrorResponse> {
    let key_matches = match (&new.key, row.key_thumbprint.as_deref()) {
        (Some(key), Some(stored)) => key.thumbprint == stored,
        _ => false,
    };
    let hints_differ = match (
        new.continuity_hint.as_deref(),
        row.continuity_hint.as_deref(),
    ) {
        (Some(sent), Some(stored)) => sent != stored,
        _ => false,
    };
    let session_holds = session_row.is_some_and(|s| s.id == row.id);

    if row.merged_into.is_some() {
        return restore_shared(db, new, row, key_matches && !hints_differ).await;
    }
    if key_matches {
        if hints_differ {
            // Step 8: a valid key with a different hint is refused.
            return Ok(None);
        }
        return keep(db, new, row).await;
    }
    // A row registered without a key takes this register's key (or none). A row
    // with a stored key moves to a new key only when the session holds it (rung 2).
    if row.key_thumbprint.is_none() || session_holds {
        return keep(db, new, row).await;
    }
    Ok(None)
}

async fn keep(
    db: &DbConn,
    new: &NewUserDevice,
    row: Model,
) -> Result<Option<String>, ErrorResponse> {
    let id = row.id.clone();
    let mut same = new.clone();
    same.id = id.clone();
    user_device::Entity::register(db, same, Some(row)).await?;
    Ok(Some(id))
}

/// Rule 7. An old key that was merged away is back. If its hint still matches a live
/// row that is not revoked, the two machines share the hint: restore the old row and
/// flag both, so the hint never matches them again. Neither is signed out.
async fn restore_shared(
    db: &DbConn,
    new: &NewUserDevice,
    row: Model,
    key_matches: bool,
) -> Result<Option<String>, ErrorResponse> {
    if !key_matches || new.continuity_hint.is_none() {
        return Ok(None);
    }
    let successor = match row.merged_into.clone() {
        Some(into) => user_device::Entity::find_row(db, new.user_id, &into).await?,
        None => None,
    };
    let shared = successor
        .as_ref()
        .is_some_and(|s| s.revoked_at.is_none() && s.continuity_hint == row.continuity_hint);
    let restored = user_device::Entity::restore_merged(db, row, shared).await?;
    if let (true, Some(other)) = (shared, successor) {
        user_device::Entity::set_hint_conflict(db, other).await?;
    }
    keep(db, new, restored).await
}

/// Rung 3. A new id whose hint matches a live row with no conflict. The new id gets
/// its own row, and the old row is merged into it and retired.
async fn merge_by_hint(db: &DbConn, new: &NewUserDevice) -> Result<Option<String>, ErrorResponse> {
    let (true, Some(hint)) = (new.key.is_some(), new.continuity_hint.as_deref()) else {
        return Ok(None);
    };
    let live = user_device::Entity::live_for_user(db, new.user_id).await?;
    let Some(old) = live.into_iter().find(|row| {
        row.continuity_hint.as_deref() == Some(hint) && !row.hint_conflict && row.id != new.id
    }) else {
        return Ok(None);
    };
    let old_thumbprint = old.key_thumbprint.clone();
    let fresh = user_device::Entity::register(db, new.clone(), None).await?;
    if let Some(thumbprint) = old_thumbprint {
        user_device::Entity::set_merged_key_thumbprint(db, fresh, thumbprint).await?;
    }
    user_device::Entity::mark_merged(db, old, &new.id).await?;
    Ok(Some(new.id.clone()))
}

/// Rung 4 housekeeping. Older rows that look like this device and have no live
/// session stop listing. A row with a live session is another device and stays.
async fn retire_stale_peers(
    db: &DbConn,
    new: &NewUserDevice,
    live_devices: &[String],
) -> Result<(), ErrorResponse> {
    let live = user_device::Entity::live_for_user(db, new.user_id).await?;
    for row in live {
        let same_shape = row.platform == new.platform
            && row.client == new.client
            && row.browser == new.browser
            && row.form_factor == new.form_factor;
        if same_shape && !live_devices.contains(&row.id) {
            user_device::Entity::retire(db, row).await?;
        }
    }
    Ok(())
}

/// The live row the current session is bound to, if the session has one.
pub async fn session_device_row(
    db: &DbConn,
    user_id: i32,
    session_id: Option<i32>,
) -> Result<Option<Model>, ErrorResponse> {
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    let session = <user_session::Entity as EntityTrait>::find_by_id(session_id)
        .one(db)
        .await
        .map_err_to_response()?;
    let Some(session) = session.filter(|s| s.user_id == user_id) else {
        return Ok(None);
    };
    let Some(device_id) = session.device_id else {
        return Ok(None);
    };
    user_device::Entity::find_row(db, user_id, &device_id).await
}
