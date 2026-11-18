//! The settings document (PARITY J9; web `lib/settings/settings-document.ts`):
//! `{ v, appearance, reading, privacy }` built from the appearance, reader
//! and consent stores, applied back from a sync round or a data import.
//!
//! Sync: one register per setting (`settings.appearance.*`,
//! `settings.reading.*`; `eq_core::store::preferences`). [`install`]
//! captures the current values whenever a setting changes or the account's
//! first full pull lands, and applies the values another device sent
//! (`SyncStore::settings_inbox`). Privacy (consent) never syncs.

use eq_core::{api::AuthPhase, store::preferences};
use nsh_core::prelude::*;
use serde_json::{Value, json};

use crate::{
    app::{auth_store, sync_store},
    stores::{
        appearance::{AppearanceStore, Prefs, SeedKey},
        consent::ConsentStore,
        reader::ReaderStore,
        sync::Area,
    },
};

/// `SETTINGS_DOC_VERSION`.
pub const SETTINGS_DOC_VERSION: i64 = 1;

/// The settings document of the current values (`toSettingsDoc`). Untracked.
pub fn document() -> Value {
    let appearance = AppearanceStore::get();
    let (prefs, dark) = untracked(|| (appearance.prefs(), appearance.is_dark()));
    let appearance_doc =
        serde_json::from_str::<Value>(&prefs.to_json(dark)).unwrap_or_else(|_| json!({}));
    let privacy = untracked(|| ConsentStore::get().current());
    json!({
        "v": SETTINGS_DOC_VERSION,
        "appearance": appearance_doc,
        "reading": ReaderStore::get().reading_doc(),
        "privacy": privacy.to_value(),
    })
}

/// Applies a document's `appearance` as the web's `decodeAppearance` +
/// `prefs.set` do: the custom seeds, instant resume and the legacy accent.
/// The web decoder keeps no `mode`/`palette`, so neither changes here.
fn apply_appearance(raw: &Value) {
    if !raw.is_object() {
        return;
    }
    let incoming = Prefs::from_json(&raw.to_string());
    let store = AppearanceStore::get();
    store.set_legacy_accent(&incoming.legacy_accent);
    for key in SeedKey::ALL {
        store.set_custom(key, incoming.seed(key));
    }
    store.set_instant_resume(incoming.instant_resume);
}

/// `applySyncedSettings`: appearance, then reading. Consent untouched.
pub fn apply_synced(doc: &Value) {
    apply_appearance(&doc["appearance"]);
    if doc["reading"].is_object() {
        ReaderStore::get().apply_reading_doc(&doc["reading"]);
    }
}

/// `applySettingsDoc` (a data import): consent off, then the synced part.
/// The imported `privacy` is never applied.
pub fn apply_document(doc: &Value) {
    ConsentStore::get().deny_all();
    apply_synced(doc);
}

/// Stamps the current settings into their registers (no-op signed out or
/// before the first full pull).
fn capture() {
    let entries = preferences::settings_entries(&document());
    sync_store().write(
        Area::Preferences,
        move |s| preferences::capture(s, Some(&entries)),
        |result| {
            if let Err(e) = result {
                log::debug!("settings capture: {e}");
            }
        },
    );
}

/// Wires settings sync and the consent gates for as long as the caller's
/// owner lives. Once, under the app's provider (the shell's install).
pub fn install() {
    let auth = auth_store();
    let sync = sync_store();
    let consent = ConsentStore::get();
    let appearance = AppearanceStore::get();
    let reader = ReaderStore::get();

    // A34: a minor account holds analytics off; nothing leaves before auth
    // resolves.
    effect(move || {
        let phase = auth.phase();
        let minor = phase.user().is_some_and(|u| u.is_minor);
        let resolved = !matches!(phase, AuthPhase::Unknown);
        untracked(|| {
            consent.set_minor_account(minor);
            if resolved {
                consent.mark_auth_resolved();
            }
        });
    });

    // Values from another device.
    effect(move || {
        if sync.settings_inbox().is_empty() {
            return;
        }
        untracked(|| {
            let taken = sync.take_settings();
            let mut doc = document();
            preferences::apply_settings_entries(&mut doc, &taken);
            apply_synced(&doc);
        });
    });

    // Local changes, and the first full pull.
    effect(move || {
        sync.capture_due();
        appearance.prefs();
        reader.prefs();
        untracked(capture);
    });
}
