//! The auth store (PARITY I1, I8): a `Copy` store over
//! `eq_core::api::AuthMachine`. Screens read [`AuthStore::phase`] in
//! bindings and call the async methods inside `spawn`. Made once in the
//! root (`AuthStore::new`) and handed down by context.
//!
//! On start it opens the secret store off the engine thread, builds the
//! API client, restores the session (offline keeps the cached account),
//! and registers the device after every sign-in.

use std::rc::Rc;

use eq_core::{
    api::{
        ApiClient, ApiError, AuthMachine, AuthPhase, Secret, SessionProbe, SignInStep,
        TransitionKind,
        auth_state::Transition,
        types::{SignInOutcome, UserProfile},
    },
    clock::SystemClock,
    device::{RegisterOutcome, ensure_device_registered},
};
use nsh_core::prelude::*;

use crate::services::{auth as service, storage::NshKv};

/// Auth state for screens.
#[derive(Clone, Copy)]
pub struct AuthStore {
    phase: Signal<AuthPhase>,
    ready: Signal<bool>,
    machine: Stored<Option<Rc<AuthMachine>>>,
    registering: Stored<bool>,
}

impl AuthStore {
    /// Makes the store and starts it: secrets load in `blocking`, then the
    /// session is restored. Call once, in the root.
    pub fn new() -> Self {
        let store = Self {
            phase: signal(AuthPhase::Unknown),
            ready: signal(false),
            machine: stored(None),
            registering: stored(false),
        };
        spawn(async move { store.start().await });
        store
    }

    async fn start(self) {
        let secrets = open_secrets().await;
        let (_client, machine) = service::build(secrets);
        let phase = self.phase;
        machine.subscribe(move |p| {
            let _ = phase.try_set(p.clone());
        });
        machine.on_transition(move |t: Transition| {
            if t.kind.is_sign_in() {
                self.register_device(None);
            }
        });
        self.machine.with(|m| *m = Some(machine.clone()));
        machine.hydrate().await;
        let _ = self.ready.try_set(true);
        if machine.user().is_some() {
            self.register_device(None);
        }
    }

    /// The phase (tracked).
    pub fn phase(&self) -> AuthPhase {
        self.phase.get()
    }

    /// The signed-in account (tracked).
    pub fn user(&self) -> Option<UserProfile> {
        self.phase.with(|p| p.user().cloned())
    }

    /// Whether the first restore finished (tracked).
    pub fn ready(&self) -> bool {
        self.ready.get()
    }

    /// The state machine, once started.
    pub fn machine(&self) -> Option<Rc<AuthMachine>> {
        self.machine.get()
    }

    /// The API client, once started (other stores call endpoints and
    /// `ApiPost::post_json` through it).
    pub fn client(&self) -> Option<Rc<ApiClient>> {
        self.machine().map(|m| m.client().clone())
    }

    fn require(&self) -> Result<Rc<AuthMachine>, ApiError> {
        self.machine()
            .ok_or_else(|| ApiError::Decode("auth is still starting".into()))
    }

    /// Email and password. `TotpRequired`: call [`AuthStore::submit_totp`].
    pub async fn sign_in(self, email: String, password: Secret) -> Result<SignInStep, ApiError> {
        self.require()?.log_in(&email, &password).await
    }

    /// The TOTP code of a pending sign-in.
    pub async fn submit_totp(self, code: Secret) -> Result<SignInStep, ApiError> {
        self.require()?.submit_totp(&code).await
    }

    /// Applies a sign-in reply from a provider token or passkey flow.
    pub async fn complete_sign_in(
        self,
        result: Result<SignInOutcome, ApiError>,
        kind: TransitionKind,
    ) -> Result<SignInStep, ApiError> {
        self.require()?.complete_sign_in(result, kind).await
    }

    /// Abandons a pending TOTP step.
    pub fn cancel_totp(self) {
        if let Some(m) = self.machine() {
            m.cancel_totp();
        }
    }

    /// Signs out; `push_token` is deleted on the server first.
    pub async fn sign_out(self, push_token: Option<String>) -> Result<(), ApiError> {
        self.require()?.log_out(push_token.as_deref()).await
    }

    /// Re-reads the account (after email verification, profile edits).
    pub async fn refresh_user(self) -> SessionProbe {
        match self.machine() {
            Some(m) => m.refresh_user().await,
            None => SessionProbe::Unknown,
        }
    }

    /// Shows an updated account (2FA on/off, profile, age) without a
    /// sign-in edge.
    pub fn set_user(self, user: UserProfile) {
        if let Some(m) = self.machine() {
            m.set_user(user);
        }
    }

    /// After `user/v1/delete`: wipes this install's records of the account.
    pub fn account_deleted(self, user_id: i32) {
        if let Some(m) = self.machine() {
            m.account_deleted(user_id);
        }
    }

    /// Registers this device when due (sign-in, push token change). One at a
    /// time; failures never block reading or sync.
    pub fn register_device(self, push_token: Option<String>) {
        if self.registering.get() {
            return;
        }
        let Some(machine) = self.machine() else {
            return;
        };
        self.registering.with(|r| *r = true);
        spawn(async move {
            // Runs after the transition publishes the new user.
            if let Some(user) = machine.user() {
                let env = service::device_env();
                let outcome = ensure_device_registered(
                    machine.client(),
                    &NshKv,
                    &SystemClock,
                    &env,
                    &user,
                    push_token.as_deref(),
                )
                .await;
                if let Err(e) = outcome.as_ref() {
                    log::warn!("device registration failed: status {}", e.status());
                }
                if let Ok(RegisterOutcome::Registered { keyed, .. }) = outcome {
                    log::debug!("device registered (keyed: {keyed})");
                }
            }
            self.registering.with(|r| *r = false);
        });
    }
}

impl Default for AuthStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn open_secrets() -> std::sync::Arc<crate::services::storage::PlatformSecrets> {
    let dir = nsh_core::element::data_dir();
    match blocking(move || service::open_secrets(dir)).await {
        Ok(secrets) => secrets,
        Err(_) => std::sync::Arc::new(crate::services::storage::PlatformSecrets::memory()),
    }
}

#[cfg(target_arch = "wasm32")]
async fn open_secrets() -> std::sync::Arc<crate::services::storage::PlatformSecrets> {
    std::sync::Arc::new(crate::services::storage::PlatformSecrets::memory())
}
