//! Storage seams (lead-owned). [`KeyValue`] is plain app storage, which the
//! app crate implements over `nsh_core::storage::Storage`
//! (`src/services/storage.rs`). [`SecretStore`] holds secrets (session
//! value, device key) in the platform keystore; its [`KeyStoreKind`] is the
//! `key_store` field of device registration (`contracts/devices.md`).
//! [`MemoryKv`] is the in-memory impl for tests.

use std::{cell::RefCell, collections::BTreeMap};

/// A write that did not land.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KvError {
    /// The backend refused or failed the write.
    #[error("storage write failed: {0}")]
    Write(String),
    /// No backend on this platform.
    #[error("storage unavailable")]
    Unavailable,
}

/// String key/value storage. Single-threaded, as the NSH engine is.
pub trait KeyValue {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str) -> Result<(), KvError>;
    fn remove(&self, key: &str);
    fn keys(&self) -> Vec<String>;
}

/// Where a [`SecretStore`] keeps its secrets, by the backend's wire names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStoreKind {
    /// Secure Enclave / StrongBox-backed Keystore (iOS, Android).
    SecureHardware,
    /// OS keychain / credential store (desktop).
    OsKeystore,
    /// A file in the app's data directory (desktop fallback).
    File,
    /// No usable store: the device registers keyless.
    None,
}

impl KeyStoreKind {
    /// The `key_store` value the backend accepts.
    pub fn as_str(self) -> &'static str {
        match self {
            KeyStoreKind::SecureHardware => "secure_hardware",
            KeyStoreKind::OsKeystore => "os_keystore",
            KeyStoreKind::File => "file",
            KeyStoreKind::None => "none",
        }
    }
}

/// Secret storage. Values never reach logs, traces or analytics.
pub trait SecretStore {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str) -> Result<(), KvError>;
    fn remove(&self, key: &str);
    fn kind(&self) -> KeyStoreKind;
}

/// In-memory [`KeyValue`] for tests.
#[derive(Debug, Default)]
pub struct MemoryKv {
    map: RefCell<BTreeMap<String, String>>,
}

impl MemoryKv {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeyValue for MemoryKv {
    fn get(&self, key: &str) -> Option<String> {
        self.map.borrow().get(key).cloned()
    }

    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        self.map
            .borrow_mut()
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn remove(&self, key: &str) {
        self.map.borrow_mut().remove(key);
    }

    fn keys(&self) -> Vec<String> {
        self.map.borrow().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_kv_roundtrips() {
        let kv = MemoryKv::new();
        kv.set("b", "2").unwrap();
        kv.set("a", "1").unwrap();
        assert_eq!(kv.get("a").as_deref(), Some("1"));
        assert_eq!(kv.keys(), vec!["a", "b"]);
        kv.remove("a");
        assert_eq!(kv.get("a"), None);
    }

    #[test]
    fn key_store_wire_names() {
        assert_eq!(KeyStoreKind::SecureHardware.as_str(), "secure_hardware");
        assert_eq!(KeyStoreKind::OsKeystore.as_str(), "os_keystore");
        assert_eq!(KeyStoreKind::File.as_str(), "file");
        assert_eq!(KeyStoreKind::None.as_str(), "none");
    }
}
