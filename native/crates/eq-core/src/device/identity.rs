//! Who this install is (contracts/devices.md D3, web `identity.ts`): the
//! platform labels the backend accepts, the per-account install id, and the
//! registration shape check (a copy of the backend's
//! `device_v1::validator::check_registration_shape`).

use crate::{
    api::types::DeviceRegistration,
    kv::{KeyStoreKind, KeyValue},
};

/// The OS this build runs on, by the backend's `platform` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Ios,
    Android,
    Macos,
    Windows,
    Linux,
}

impl Platform {
    /// The platform this binary was built for; `None` for any other target.
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "ios") {
            return Some(Platform::Ios);
        }
        if cfg!(target_os = "android") {
            return Some(Platform::Android);
        }
        if cfg!(target_os = "macos") {
            return Some(Platform::Macos);
        }
        if cfg!(target_os = "windows") {
            return Some(Platform::Windows);
        }
        if cfg!(target_os = "linux") {
            return Some(Platform::Linux);
        }
        None
    }

    /// The `platform` value.
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Android => "android",
            Platform::Macos => "macos",
            Platform::Windows => "windows",
            Platform::Linux => "linux",
        }
    }

    /// Whether this platform sends a continuity hint (Android, Windows,
    /// Linux; Apple platforms never do).
    pub fn sends_hint(self) -> bool {
        matches!(
            self,
            Platform::Android | Platform::Windows | Platform::Linux
        )
    }
}

/// The `client` label for a platform. The backend accepts `web`,
/// `react_native` and `gpui` only, so this NSH app registers as
/// `react_native` on iOS/Android and `gpui` on desktop. A backend change
/// adding an `nsh` client label is proposed (PARITY.md, "Proposed backend
/// changes"); when it lands, only this function changes.
pub fn client_label(platform: Platform) -> &'static str {
    match platform {
        Platform::Ios | Platform::Android => "react_native",
        Platform::Macos | Platform::Windows | Platform::Linux => "gpui",
    }
}

/// The `key_store` this install may claim for `kind` under its client
/// label; `None` when the label does not accept it (the device then
/// registers keyless). React Native: `secure_hardware` or `none`. GPUI:
/// `os_keystore`, `file` or `none`.
pub fn key_store_label(platform: Platform, kind: KeyStoreKind) -> Option<KeyStoreKind> {
    let accepted = match client_label(platform) {
        "react_native" => matches!(kind, KeyStoreKind::SecureHardware | KeyStoreKind::None),
        _ => matches!(
            kind,
            KeyStoreKind::OsKeystore | KeyStoreKind::File | KeyStoreKind::None
        ),
    };
    accepted.then_some(kind)
}

/// `form_factor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFactor {
    Phone,
    Tablet,
    Desktop,
}

impl FormFactor {
    pub fn as_str(self) -> &'static str {
        match self {
            FormFactor::Phone => "phone",
            FormFactor::Tablet => "tablet",
            FormFactor::Desktop => "desktop",
        }
    }
}

/// What the app knows about this install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEnv {
    pub platform: Platform,
    pub form_factor: FormFactor,
    /// 1 to 999.
    pub os_major: Option<i32>,
    /// 1 to 32 characters.
    pub app_version: Option<String>,
    /// The raw OS machine id for the continuity hint (`/etc/machine-id`,
    /// `MachineGuid`, `ANDROID_ID`), on platforms that send one. Never
    /// sent: only its HMAC under the account salt leaves the device.
    pub os_id: Option<String>,
}

const ID_PREFIX: &str = "easyquran.device.id.";

/// Client-minted id rule: 1 to 64 of `[A-Za-z0-9._-]`.
pub fn valid_device_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// A new install id (UUID v7).
pub fn new_device_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// The install id for account `owner`, minted and kept on first use. A
/// minor account (A34) gets a fresh random id that is never stored or
/// registered.
pub fn device_id_for(kv: &dyn KeyValue, owner: i32, is_minor: bool) -> String {
    if is_minor {
        return uuid::Uuid::new_v4().to_string();
    }
    let key = format!("{ID_PREFIX}{owner}");
    if let Some(id) = kv.get(&key).filter(|id| valid_device_id(id)) {
        return id;
    }
    let minted = new_device_id();
    // A failed write mints again next time; registration adopts either.
    let _ = kv.set(&key, &minted);
    minted
}

/// Adopts the id the server kept (rungs 2 and 3 hand back the row's id).
pub fn set_device_id_for(kv: &dyn KeyValue, owner: i32, id: &str) {
    let _ = kv.set(&format!("{ID_PREFIX}{owner}"), id);
}

/// Drops the stored id (minor account, account deletion).
pub fn forget_device_id_for(kv: &dyn KeyValue, owner: i32) {
    kv.remove(&format!("{ID_PREFIX}{owner}"));
}

/// The backend's cross-field rules (`check_registration_shape`), so a body
/// the server would refuse is never sent.
pub fn check_registration_shape(p: &DeviceRegistration) -> Result<(), &'static str> {
    let platform_ok = match p.client.as_str() {
        "web" => p.platform == "web",
        "react_native" => matches!(p.platform.as_str(), "ios" | "android"),
        "gpui" => matches!(p.platform.as_str(), "macos" | "windows" | "linux"),
        _ => false,
    };
    if !platform_ok {
        return Err("platform does not match client");
    }
    if (p.client == "web") != p.browser.is_some() {
        return Err("browser is only sent by the web client");
    }
    let store_ok = match p.client.as_str() {
        "web" => matches!(p.key_store.as_str(), "browser" | "none"),
        "react_native" => matches!(p.key_store.as_str(), "secure_hardware" | "none"),
        "gpui" => matches!(p.key_store.as_str(), "os_keystore" | "file" | "none"),
        _ => false,
    };
    if !store_ok {
        return Err("key_store does not match client");
    }
    let has_key = p.public_key.is_some();
    if (p.key_store == "none") == has_key {
        return Err("key_store and public_key disagree");
    }
    if !has_key {
        if p.nonce.is_none() && p.signature.is_none() {
            return Ok(());
        }
        return Err("nonce and signature need a public_key");
    }
    let (Some(nonce), Some(signature)) = (p.nonce.as_deref(), p.signature.as_deref()) else {
        return Err("a public_key needs a nonce and a signature");
    };
    if super::keys::decode_exact(nonce, 32).is_none() {
        return Err("nonce is not 32 bytes of base64url");
    }
    if super::keys::decode_exact(signature, super::keys::RAW_SIGNATURE_BYTES).is_none() {
        return Err("signature must be 64 raw bytes of base64url");
    }
    match p.continuity_hint.as_deref() {
        Some(hint) if super::keys::decode_exact(hint, 32).is_none() => {
            Err("continuity_hint must be 32 bytes of base64url")
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemoryKv;

    #[test]
    fn labels_follow_the_backend_enums() {
        assert_eq!(client_label(Platform::Ios), "react_native");
        assert_eq!(client_label(Platform::Android), "react_native");
        for p in [Platform::Macos, Platform::Windows, Platform::Linux] {
            assert_eq!(client_label(p), "gpui");
        }
        assert_eq!(
            key_store_label(Platform::Ios, KeyStoreKind::OsKeystore),
            None
        );
        assert_eq!(
            key_store_label(Platform::Linux, KeyStoreKind::SecureHardware),
            None
        );
        assert_eq!(
            key_store_label(Platform::Macos, KeyStoreKind::File),
            Some(KeyStoreKind::File)
        );
        assert!(!Platform::Ios.sends_hint() && !Platform::Macos.sends_hint());
        assert!(Platform::Android.sends_hint() && Platform::Linux.sends_hint());
    }

    #[test]
    fn install_ids_persist_per_account_and_minors_get_none() {
        let kv = MemoryKv::new();
        let a = device_id_for(&kv, 7, false);
        assert!(valid_device_id(&a));
        assert_eq!(device_id_for(&kv, 7, false), a);
        assert_ne!(device_id_for(&kv, 8, false), a);
        let minor = device_id_for(&kv, 9, true);
        assert!(kv.get("easyquran.device.id.9").is_none());
        assert_ne!(device_id_for(&kv, 9, true), minor);
    }
}
