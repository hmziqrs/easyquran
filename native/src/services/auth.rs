//! The auth service: builds the one `eq_core::api::ApiClient` (session
//! transport over [`super::transport::NshTransport`], secrets in
//! [`super::storage::PlatformSecrets`]) and the `AuthMachine`, and says
//! what this install is for device registration. Replaces the starter's
//! mock provider. The app's auth store is `crate::stores::auth`.

use std::{path::PathBuf, rc::Rc, sync::Arc};

use eq_core::{
    api::{ApiClient, ApiConfig, AuthMachine},
    device::{DeviceEnv, FormFactor, Platform},
    kv::{KeyValue, SecretStore},
};

use super::{
    storage::{NshKv, PlatformSecrets, SharedSecrets},
    transport::NshTransport,
};

/// The API this build talks to: `EQ_API_BASE` at build time when set,
/// else the local API (`http://localhost:8888`) in debug builds and
/// production (`https://easyquran.fyi/api`) in release builds.
pub fn api_config() -> ApiConfig {
    if let Some(base) = option_env!("EQ_API_BASE") {
        return ApiConfig::new(base);
    }
    if cfg!(debug_assertions) {
        return ApiConfig::local();
    }
    ApiConfig::production()
}

/// What this install reports at device registration. `phone` stands for
/// every mobile device until the shell reports tablets.
pub fn device_env() -> DeviceEnv {
    let platform = Platform::current().unwrap_or(Platform::Linux);
    let form_factor = match platform {
        Platform::Ios | Platform::Android => FormFactor::Phone,
        Platform::Macos | Platform::Windows | Platform::Linux => FormFactor::Desktop,
    };
    DeviceEnv {
        platform,
        form_factor,
        os_major: os_major(),
        app_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        os_id: os_id(),
    }
}

/// The host OS major version where the platform exposes it natively.
fn os_major() -> Option<i32> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
        i32::try_from(version.majorVersion).ok().filter(|&v| v > 0)
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        None
    }
}

/// The machine id for the continuity hint: `/etc/machine-id` on Linux.
/// Windows (`MachineGuid`, registry) and Android (`ANDROID_ID`, Java) need
/// platform code this crate does not have yet, so they send no hint: the
/// hint is optional and only helps reinstall recognition.
fn os_id() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/machine-id")
            .ok()
            .filter(|id| !id.trim().is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Opens the secret store. Blocking (keychain or file read): run it in
/// `blocking(..)`, then hand the result to [`build`].
pub fn open_secrets(data_dir: Option<PathBuf>) -> Arc<PlatformSecrets> {
    PlatformSecrets::open(data_dir)
}

/// The client and state machine over `secrets`.
pub fn build(secrets: Arc<PlatformSecrets>) -> (Rc<ApiClient>, Rc<AuthMachine>) {
    let secrets: Rc<dyn SecretStore> = Rc::new(SharedSecrets(secrets));
    let client = Rc::new(ApiClient::new(api_config(), Rc::new(NshTransport), secrets));
    let kv: Rc<dyn KeyValue> = Rc::new(NshKv);
    let machine = AuthMachine::new(client.clone(), kv);
    (client, machine)
}
