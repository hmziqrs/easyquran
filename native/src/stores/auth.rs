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
        ApiClient, ApiConfig, ApiError, AuthMachine, AuthPhase, Secret, SessionProbe, SignInStep,
        TransitionKind,
        auth_state::Transition,
        types::{SignInOutcome, UserProfile},
    },
    clock::SystemClock,
    device::{DeviceEnv, RegisterOutcome, ensure_device_registered},
    kv::{KeyValue, SecretStore},
    net::Transport,
};
use nsh_core::prelude::*;

use crate::services::{
    auth as service,
    storage::{NshKv, SharedSecrets},
    transport::NshTransport,
};

/// What the auth store is built from. The app passes
/// [`AuthDeps::platform`]; tests inject a scripted transport and memory
/// stores (`app::provide`).
pub struct AuthDeps {
    pub config: ApiConfig,
    pub transport: Rc<dyn Transport>,
    pub kv: Rc<dyn KeyValue>,
    /// `None`: the platform secret store, opened off the engine thread.
    pub secrets: Option<Rc<dyn SecretStore>>,
    pub device: DeviceEnv,
}

impl AuthDeps {
    /// The real services: NSH fetch, NSH storage, Keychain/file secrets.
    pub fn platform() -> Self {
        Self {
            config: service::api_config(),
            transport: Rc::new(NshTransport),
            kv: Rc::new(NshKv),
            secrets: None,
            device: service::device_env(),
        }
    }
}

/// Auth state for screens.
#[derive(Clone, Copy)]
pub struct AuthStore {
    phase: Signal<AuthPhase>,
    ready: Signal<bool>,
    deleted: Signal<Option<i32>>,
    machine: Stored<Option<Rc<AuthMachine>>>,
    registering: Stored<bool>,
    /// A registration was asked for while one ran (a push token arrived):
    /// run once more when it ends.
    register_again: Stored<bool>,
    kv: Stored<Rc<dyn KeyValue>>,
    device: Stored<DeviceEnv>,
}

impl AuthStore {
    /// Makes the store over the platform services and starts it. Call
    /// once, in the root (`app::provide` does).
    pub fn new() -> Self {
        Self::with(AuthDeps::platform())
    }

    /// Makes the store over `deps` and starts it: secrets load (in
    /// `blocking` for the platform store), then the session is restored.
    pub fn with(deps: AuthDeps) -> Self {
        let AuthDeps {
            config,
            transport,
            kv,
            secrets,
            device,
        } = deps;
        let store = Self {
            phase: signal(AuthPhase::Unknown),
            ready: signal(false),
            deleted: signal(None),
            machine: stored(None),
            registering: stored(false),
            register_again: stored(false),
            kv: stored(kv),
            device: stored(device),
        };
        spawn(async move { store.start(config, transport, secrets).await });
        store
    }

    async fn start(
        self,
        config: ApiConfig,
        transport: Rc<dyn Transport>,
        secrets: Option<Rc<dyn SecretStore>>,
    ) {
        let secrets: Rc<dyn SecretStore> = match secrets {
            Some(secrets) => secrets,
            None => Rc::new(SharedSecrets(open_secrets().await)),
        };
        let client = Rc::new(ApiClient::new(config, transport, secrets));
        let machine = AuthMachine::new(client, self.kv.get());
        let phase = self.phase;
        machine.subscribe(move |p| {
            let _ = phase.try_set(p.clone());
        });
        machine.on_transition(move |t: Transition| {
            if t.kind.is_sign_in() {
                self.register_device(None);
                crate::stores::push::on_signed_in();
            }
        });
        self.machine.with(|m| *m = Some(machine.clone()));
        machine.hydrate().await;
        let _ = self.ready.try_set(true);
        if machine.user().is_some() {
            self.register_device(None);
            crate::stores::push::on_signed_in();
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
    /// sync through its `SyncPost`/`SyncGet` impls).
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

    /// Signs out; `push_token` (default: this install's FCM token) is
    /// deleted on the server first (devices.md "Sign-out on a shared
    /// phone").
    pub async fn sign_out(self, push_token: Option<String>) -> Result<(), ApiError> {
        let push_token = push_token.or_else(crate::stores::push::current_token);
        let result = self.require()?.log_out(push_token.as_deref()).await;
        crate::stores::push::forget();
        result
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

    /// After `user/v1/delete`: wipes this install's records of the account
    /// (the app root then purges its sync file).
    pub fn account_deleted(self, user_id: i32) {
        if let Some(m) = self.machine() {
            m.account_deleted(user_id);
        }
        self.deleted.set(Some(user_id));
    }

    /// The account last deleted on this install (tracked).
    pub fn deleted_account(&self) -> Option<i32> {
        self.deleted.get()
    }

    /// The key-value store account records live in.
    pub fn kv(&self) -> Rc<dyn KeyValue> {
        self.kv.get()
    }

    /// Registers this device when due (sign-in, push token change). One at a
    /// time (a call during a run runs once more after it); failures never
    /// block reading or sync. `None` sends this install's FCM token, if any.
    pub fn register_device(self, push_token: Option<String>) {
        if self.registering.get() {
            self.register_again.with(|a| *a = true);
            return;
        }
        let push_token = push_token.or_else(crate::stores::push::current_token);
        let Some(machine) = self.machine() else {
            return;
        };
        self.registering.with(|r| *r = true);
        let kv = self.kv.get();
        let env = self.device.get();
        spawn(async move {
            // Runs after the transition publishes the new user.
            if let Some(user) = machine.user() {
                let outcome = ensure_device_registered(
                    machine.client(),
                    kv.as_ref(),
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
            if self.register_again.get() {
                self.register_again.with(|a| *a = false);
                self.register_device(None);
            }
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
