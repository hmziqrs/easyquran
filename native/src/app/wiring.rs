//! Auth ↔ sync ↔ device, after the web's `purge-hook.ts` and
//! `account-purge.ts`:
//!
//! - Sync posts go through the auth store's `ApiClient` (D6 session
//!   transport: `x-eq-session`, CSRF, rotation, 401 → session lost).
//! - Signed in (sign-in edge, account switch, or a restored session):
//!   `SyncStore::sign_in` folds the signed-out rows into the account and
//!   kicks a round. Device registration runs from the auth store itself.
//! - Signed out (log out, session expired / revoked / lost, banned): sync
//!   `sign_out`, which quarantines the account's rows (dirty ones wait in
//!   its file for the next sign-in, never destroyed).
//! - Account deleted: sign out, then `purge` its file, dirty rows included.
//! - The server says the session is another account (sync 403 mismatch):
//!   auth re-probes the session, then sync clears the flag.

use std::rc::Rc;

use eq_core::{device::identity::device_id_for, kv::KeyValue};
use nsh_core::prelude::*;

use crate::stores::{auth::AuthStore, sync::SyncStore};

/// Connects `auth` and `sync`. Call once, under the root's owner.
pub fn connect(auth: AuthStore, sync: SyncStore, kv: Rc<dyn KeyValue>) {
    // The sync transport, once the API client exists.
    effect(move || {
        if !auth.ready() {
            return;
        }
        if let Some(client) = untracked(|| auth.client()) {
            sync.set_transport(client);
        }
    });

    // The account on screen drives the sync store's account.
    let applied: Stored<Option<i32>> = stored(None);
    effect(move || {
        let user = auth.user();
        let ready = auth.ready() && sync.ready();
        if !ready {
            return;
        }
        let current = user.as_ref().map(|u| u.id);
        if applied.get() == current {
            return;
        }
        applied.with(|a| *a = current);
        let Some(user) = user else {
            untracked(|| sync.sign_out());
            return;
        };
        let device_id = device_id_for(kv.as_ref(), user.id, user.is_minor);
        untracked(|| {
            sync.sign_in(i64::from(user.id), device_id, |result| {
                if let Err(e) = result {
                    log::warn!("sync sign-in failed: {e}");
                }
            });
        });
    });

    // Account deletion: its local file goes, dirty rows included.
    effect(move || {
        let Some(user_id) = auth.deleted_account() else {
            return;
        };
        untracked(|| {
            applied.with(|a| *a = None);
            sync.sign_out();
            sync.purge(i64::from(user_id));
        });
    });

    // A sync round found the session belongs to another account.
    let probing: Stored<bool> = stored(false);
    effect(move || {
        if !sync.status().session_lost || probing.get() {
            return;
        }
        probing.with(|p| *p = true);
        spawn(async move {
            let _ = auth.refresh_user().await;
            sync.clear_session_lost();
            probing.with(|p| *p = false);
        });
    });
}
