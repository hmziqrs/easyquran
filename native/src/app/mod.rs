//! App-level composition: the stores, made once in dependency order and
//! provided by context above the shell (so every page the nav stack
//! mounts finds them), then the window assembly around the shell.
//!
//! Order: services (transport, kv, secrets, data dir) → `AuthStore` (API
//! client + `AuthMachine` over them) → `QuranStore` → `SyncStore` (sync
//! through the auth store's `ApiClient`) → [`wiring`] between them. The
//! locale is installed earlier, in `lib.rs`.
//!
//! Screens read stores while building with [`auth_store`], [`quran_store`]
//! and [`sync_store`].

use std::{path::PathBuf, rc::Rc};

use eq_core::{
    api::ApiConfig,
    device::DeviceEnv,
    kv::{KeyValue, SecretStore},
    net::Transport,
    quran::{online::QuranApi, store::ArtifactBase},
    sync::round_status::ClientInfo,
};
use nsh_core::prelude::*;

use crate::{
    services::{auth as auth_service, files, storage::NshKv, transport::NshTransport},
    shell,
    stores::{
        auth::{AuthDeps, AuthStore},
        quran::QuranStore,
        sync::{self as sync_stores, SyncStore},
    },
};

pub mod wiring;

/// Everything the stores are built over. [`Services::platform`] in the
/// app; tests inject a scripted transport, memory stores and a temp dir.
pub struct Services {
    pub transport: Rc<dyn Transport>,
    pub kv: Rc<dyn KeyValue>,
    /// `None`: the platform secret store (Keychain / file), opened off the
    /// engine thread.
    pub secrets: Option<Rc<dyn SecretStore>>,
    /// The app's data directory (`${dataDir}`); `None` keeps the sync DB in
    /// memory and Quran artifacts under the system temp dir.
    pub data_dir: Option<PathBuf>,
    pub api: ApiConfig,
    /// Where Quran artifacts come from (debug: `EQ_QURAN_DIR` in place).
    pub artifact_base: ArtifactBase,
    pub device: DeviceEnv,
}

impl Services {
    /// The real services of this build.
    pub fn platform() -> Self {
        Self {
            transport: Rc::new(NshTransport),
            kv: Rc::new(NshKv),
            secrets: None,
            data_dir: files::app_data_dir(),
            api: auth_service::api_config(),
            artifact_base: files::dev_quran_dir()
                .map_or_else(ArtifactBase::default, ArtifactBase::Directory),
            device: auth_service::device_env(),
        }
    }
}

/// The stores [`provide`] made.
#[derive(Clone, Copy)]
pub struct Stores {
    pub auth: AuthStore,
    pub quran: QuranStore,
    pub sync: SyncStore,
}

/// Makes every store once over `services`, provides each by context to
/// what the current owner builds from now on, and connects them. Call
/// once, above the shell (the root does; tests call it from their root).
pub fn provide(services: Services) -> Stores {
    let Services {
        transport,
        kv,
        secrets,
        data_dir,
        api,
        artifact_base,
        device,
    } = services;

    // Sync's `client.platform`: the OS name device registration sends.
    let client = ClientInfo {
        platform: device.platform.as_str().to_owned(),
        app_version: device
            .app_version
            .clone()
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned()),
    };
    let quran_api = QuranApi::new(format!("{}/quran", api.base()));

    let auth = AuthStore::with(AuthDeps {
        config: api,
        transport: transport.clone(),
        kv: kv.clone(),
        secrets,
        device,
    });
    provide_context(auth);

    let quran_root = data_dir.as_ref().map_or_else(
        || std::env::temp_dir().join("easyquran").join("quran"),
        |d| d.join("quran"),
    );
    let quran = QuranStore::with_root(transport, quran_api, quran_root, artifact_base);
    provide_context(quran);

    let sync = SyncStore::new(client);
    sync.open_at(data_dir.map(|d| d.join("sync")));
    sync.start();
    sync_stores::provide(sync);

    wiring::connect(auth, sync, kv);
    crate::stores::settings::install();
    Stores { auth, quran, sync }
}

/// The auth store. Screens call it while building.
///
/// # Panics
/// Outside the tree [`provide`] built.
pub fn auth_store() -> AuthStore {
    use_context::<AuthStore>().expect("auth store: build under app::provide")
}

/// The Quran data store. Screens call it while building.
///
/// # Panics
/// Outside the tree [`provide`] built.
pub fn quran_store() -> QuranStore {
    use_context::<QuranStore>().expect("quran store: build under app::provide")
}

/// The sync store (local app database + sync engine). Screens call it
/// while building.
///
/// # Panics
/// Outside the tree [`provide`] built.
pub fn sync_store() -> SyncStore {
    sync_stores::sync_store().expect("sync store: build under app::provide")
}

/// The surface's assembly: the stores above everything, then the
/// shell-wide wiring under the root's owner, the shell inside it.
#[component]
pub fn Root() -> impl IntoElement {
    provide(Services::platform());
    shell::install();
    view().flex_1().child(shell::shell())
}
