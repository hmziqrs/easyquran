//! Consent (PARITY N1, J6): the web's `consent.svelte.ts`, with the same
//! stored shape under the same key (`easyquran.consent`,
//! `{ "analytics": bool, "advertising": bool }`).
//!
//! - Off until the user turns analytics on in Settings.
//! - A signed-in minor account holds analytics and advertising off whatever
//!   is stored (the `minor-gate`), and until auth has resolved nothing may
//!   leave the device ([`ConsentStore::analytics_allowed`]).
//! - Consent is device-local: it never syncs, and an imported settings
//!   document turns it off ([`crate::stores::settings::apply_document`]).
//!
//! One owner: a surface-wide signal (`global`), loaded on the first
//! [`ConsentStore::get`]. [`crate::stores::settings::install`] feeds the
//! auth state in.

use nsh_core::{prelude::*, storage::Storage};
use serde_json::{Value, json};

/// The web's `CONSENT_STORAGE_KEY`.
pub const STORAGE_KEY: &str = "easyquran.consent";

/// The stored flags (`ConsentFlags`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConsentFlags {
    pub analytics: bool,
    pub advertising: bool,
}

impl ConsentFlags {
    /// `decodeConsent`: only a literal `true` turns a flag on.
    pub fn decode(raw: &Value) -> Self {
        Self {
            analytics: raw.get("analytics") == Some(&Value::Bool(true)),
            advertising: raw.get("advertising") == Some(&Value::Bool(true)),
        }
    }

    pub fn to_value(self) -> Value {
        json!({ "analytics": self.analytics, "advertising": self.advertising })
    }
}

#[derive(Clone, Default)]
struct State {
    loaded: bool,
    flags: ConsentFlags,
    minor: bool,
    auth_resolved: bool,
}

/// The consent store: a `Copy` handle over the surface's state. Reads are
/// tracked; writes persist at once.
#[derive(Clone, Copy)]
pub struct ConsentStore {
    state: Signal<State>,
}

impl ConsentStore {
    /// The surface's store; the first call loads the stored flags.
    pub fn get() -> ConsentStore {
        let state = global::<State>();
        if !state.with_untracked(|s| s.loaded) {
            let flags = Storage::get_item(STORAGE_KEY)
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .map(|v| ConsentFlags::decode(&v))
                .unwrap_or_default();
            state.update(|s| {
                s.loaded = true;
                s.flags = flags;
            });
        }
        ConsentStore { state }
    }

    /// The stored preference, as stored (the settings document's
    /// `privacy`). Tracked.
    pub fn current(self) -> ConsentFlags {
        self.state.with(|s| s.flags)
    }

    /// The analytics preference the Settings toggle shows: off for a minor
    /// account. Tracked.
    pub fn analytics(self) -> bool {
        self.state.with(|s| s.flags.analytics && !s.minor)
    }

    /// The advertising preference, off for a minor account. Tracked.
    pub fn advertising(self) -> bool {
        self.state.with(|s| s.flags.advertising && !s.minor)
    }

    /// Whether analytics may leave the device now: preference on, not a
    /// minor, auth resolved. Tracked.
    pub fn analytics_allowed(self) -> bool {
        self.state
            .with(|s| s.flags.analytics && !s.minor && s.auth_resolved)
    }

    /// Whether advertising consent may be granted now. Tracked.
    pub fn advertising_allowed(self) -> bool {
        self.state
            .with(|s| s.flags.advertising && !s.minor && s.auth_resolved)
    }

    /// Whether the signed-in account is a declared minor (the `minor-gate`).
    /// Tracked.
    pub fn minor_gate(self) -> bool {
        self.state.with(|s| s.minor)
    }

    /// Writes the given flags and persists them.
    pub fn set(self, analytics: Option<bool>, advertising: Option<bool>) {
        let mut flags = self.state.with_untracked(|s| s.flags);
        if let Some(on) = analytics {
            flags.analytics = on;
        }
        if let Some(on) = advertising {
            flags.advertising = on;
        }
        if let Err(e) = Storage::set_item(STORAGE_KEY, flags.to_value().to_string()) {
            log::warn!("consent not saved: {e:?}");
        }
        self.state.update(|s| s.flags = flags);
    }

    pub fn set_analytics(self, on: bool) {
        self.set(Some(on), None);
    }

    pub fn allow_all(self) {
        self.set(Some(true), Some(true));
    }

    pub fn deny_all(self) {
        self.set(Some(false), Some(false));
    }

    /// The signed-in account is (not) a declared minor.
    pub fn set_minor_account(self, on: bool) {
        if self.state.with_untracked(|s| s.minor) != on {
            self.state.update(|s| s.minor = on);
        }
    }

    /// Auth status has left "unknown"; latches once set.
    pub fn mark_auth_resolved(self) {
        if !self.state.with_untracked(|s| s.auth_resolved) {
            self.state.update(|s| s.auth_resolved = true);
        }
    }
}
