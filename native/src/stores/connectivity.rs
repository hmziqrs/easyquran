//! Connectivity (web `$lib/offline/online.svelte.ts`): whether the device
//! has a live network path, from `nsh-connectivity`'s debounced platform
//! watch. Feeds `SyncStore::set_online` and the tracked [`online`] read.
//!
//! No reachability probe runs (`probe_on_events: false`): the library's
//! probe GETs a third-party URL, and the web's `navigator.onLine` is the
//! same "a path is up" signal. A probe answer of `Unreachable` (none is
//! made here) would still count as offline.

#[cfg(not(target_arch = "wasm32"))]
use nsh_connectivity::{Configuration, Connectivity, NetworkState, Reachability};
use nsh_core::prelude::*;

use crate::stores::sync::SyncStore;

/// The last state seen; `None` until the platform answers (treated as
/// online, so nothing reads "Offline" before it is known).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Online(Option<bool>);

/// Whether the device is online now. Tracked: a binding that reads it
/// runs again when connectivity changes.
pub fn online() -> bool {
    global::<Online>().with(|o| o.0.unwrap_or(true))
}

/// Starts the platform watch under the caller's owner (the app's) and
/// keeps [`online`] and `sync` up to date. A platform without a watch
/// (or a refused permission) stays "online", as before this store.
pub fn start(sync: SyncStore) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        spawn(async move { watch(sync).await });
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = sync;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_online(state: &NetworkState) -> bool {
    state.is_connected && state.is_internet_reachable != Reachability::Unreachable
}

#[cfg(not(target_arch = "wasm32"))]
fn publish(sync: SyncStore, now: bool) {
    let signal = global::<Online>();
    if signal.with_untracked(|o| o.0) == Some(now) {
        return;
    }
    signal.set(Online(Some(now)));
    sync.set_online(now);
}

#[cfg(not(target_arch = "wasm32"))]
async fn watch(sync: SyncStore) {
    let connectivity = Connectivity::new();
    connectivity.configure(Configuration {
        probe_on_events: false,
        ..Configuration::default()
    });
    let mut states = match connectivity.subscribe().await {
        Ok(states) => states,
        Err(e) => {
            log::info!("connectivity watch unavailable: {}", e.code());
            return;
        }
    };
    loop {
        let now = is_online(&states.borrow_and_update());
        publish(sync, now);
        if states.changed().await.is_err() {
            return;
        }
    }
}
