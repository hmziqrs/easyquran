//! `eq_core::kv::KeyValue` over `nsh_core::storage::Storage`, and the
//! platform `eq_core::kv::SecretStore` for the session value and device key
//! (PARITY I1, I8; contracts/devices.md D3).
//!
//! NSH 0.5.1 has no secure storage of its own (no Keychain/Keystore API in
//! `nsh-core`, `nsh-base` or the platform crates; the only "Keychain" name
//! is a test-fake example in `nsh-core/src/host_channel.rs`). What each
//! platform gets here:
//!
//! | Platform | Where secrets live | `key_store` claimed |
//! | --- | --- | --- |
//! | macOS | Keychain (`keyring`, apple-native) | `os_keystore` |
//! | Windows | Credential Manager (`keyring`, windows-native) | `os_keystore` |
//! | Linux | `secrets.json`, mode 0600, in the app data dir | `file` |
//! | iOS | Keychain (`keyring`, apple-native) | `none`: a software key in the Keychain is not Secure Enclave hardware, and `react_native` only accepts `secure_hardware` or `none` |
//! | Android | `secrets.json`, mode 0600, app-private data dir | `none`: no Keystore access without JNI/Java code |
//!
//! A desktop whose keychain refuses (locked, denied) falls back to the file
//! and claims `file`. No data dir and no keychain: memory only, `none`.
//!
//! All secrets sit in one blob (one keychain item or one file), loaded once
//! by [`PlatformSecrets::open`] (blocking: call it inside `blocking(..)`)
//! and written in order by a writer thread, so no keychain or file call
//! runs on the engine thread. Values never reach a log.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

use eq_core::kv::{KeyStoreKind, KeyValue, KvError, SecretStore};
use nsh_core::storage::Storage;

/// [`KeyValue`] over the app's NSH storage (in memory after first load;
/// the backend writes off the engine thread).
#[derive(Debug, Default, Clone, Copy)]
pub struct NshKv;

impl KeyValue for NshKv {
    fn get(&self, key: &str) -> Option<String> {
        Storage::get_item(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        Storage::set_item(key, value).map_err(|full| KvError::Write(full.to_string()))
    }

    fn remove(&self, key: &str) {
        Storage::remove_item(key);
    }

    fn keys(&self) -> Vec<String> {
        Storage::get_all_keys()
    }
}

/// Where the blob is kept.
trait SecretBackend: Send {
    /// The blob, `None` when there is none yet.
    fn load(&self) -> Result<Option<String>, String>;
    /// Keeps `blob`; `None` removes it.
    fn save(&self, blob: Option<&str>) -> Result<(), String>;
}

/// Keychain item `fyi.easyquran.app` / `secrets`.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
struct KeyringBackend {
    entry: keyring::Entry,
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
impl KeyringBackend {
    fn new() -> Result<Self, String> {
        keyring::Entry::new("fyi.easyquran.app", "secrets")
            .map(|entry| Self { entry })
            .map_err(|e| e.to_string())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows"))]
impl SecretBackend for KeyringBackend {
    fn load(&self) -> Result<Option<String>, String> {
        match self.entry.get_password() {
            Ok(blob) => Ok(Some(blob)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, blob: Option<&str>) -> Result<(), String> {
        match blob {
            Some(blob) => self.entry.set_password(blob).map_err(|e| e.to_string()),
            None => match self.entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            },
        }
    }
}

/// `secrets.json` in the app data dir, owner read/write only.
struct FileBackend {
    path: PathBuf,
}

impl FileBackend {
    /// On Unix a file group or others can read is treated as exposed: its
    /// contents are dropped (the device key counts as lost and the server's
    /// rung 2 keeps the device row; the session signs in again) and the
    /// file is rewritten 0600.
    #[cfg(unix)]
    fn exposed(&self) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(&self.path).is_ok_and(|m| m.permissions().mode() & 0o077 != 0)
    }

    #[cfg(not(unix))]
    fn exposed(&self) -> bool {
        false
    }
}

impl SecretBackend for FileBackend {
    fn load(&self) -> Result<Option<String>, String> {
        if !self.path.exists() {
            return Ok(None);
        }
        if self.exposed() {
            let _ = std::fs::remove_file(&self.path);
            return Ok(None);
        }
        std::fs::read_to_string(&self.path)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    fn save(&self, blob: Option<&str>) -> Result<(), String> {
        let Some(blob) = blob else {
            return match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        };
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        write_private(&tmp, blob.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let _ = std::fs::remove_file(path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

/// The platform [`SecretStore`]: a cache in memory, persisted in order by
/// a writer thread.
pub struct PlatformSecrets {
    cache: Mutex<BTreeMap<String, String>>,
    writer: Option<mpsc::Sender<Option<String>>>,
    kind: KeyStoreKind,
}

impl std::fmt::Debug for PlatformSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.cache.lock().map(|c| c.len()).unwrap_or(0);
        f.debug_struct("PlatformSecrets")
            .field("kind", &self.kind)
            .field("entries", &count)
            .finish_non_exhaustive()
    }
}

impl PlatformSecrets {
    /// Opens this platform's store and loads every secret. Blocking (a
    /// keychain or file read): call inside `blocking(..)`. `data_dir` is
    /// `nsh_core::element::data_dir()`.
    pub fn open(data_dir: Option<PathBuf>) -> Arc<Self> {
        let file = data_dir.map(|dir| FileBackend {
            path: dir.join("secrets.json"),
        });
        let (backend, kind): (Option<Box<dyn SecretBackend>>, KeyStoreKind) = choose(file);
        let Some(backend) = backend else {
            return Arc::new(Self::memory());
        };
        let cache = match backend.load() {
            Ok(Some(blob)) => serde_json::from_str(&blob).unwrap_or_default(),
            Ok(None) => BTreeMap::new(),
            Err(e) => {
                log::warn!("secret store unreadable, starting empty: {}", redact(&e));
                BTreeMap::new()
            }
        };
        let (tx, rx) = mpsc::channel::<Option<String>>();
        let spawned = std::thread::Builder::new()
            .name("eq-secrets".into())
            .spawn(move || {
                while let Ok(mut blob) = rx.recv() {
                    // Only the newest blob matters.
                    while let Ok(newer) = rx.try_recv() {
                        blob = newer;
                    }
                    if let Err(e) = backend.save(blob.as_deref()) {
                        log::warn!("secret store write failed: {}", redact(&e));
                    }
                }
            });
        Arc::new(Self {
            cache: Mutex::new(cache),
            writer: spawned.ok().map(|_| tx),
            kind,
        })
    }

    /// A store that keeps nothing past this run and claims `none`.
    pub fn memory() -> Self {
        Self {
            cache: Mutex::new(BTreeMap::new()),
            writer: None,
            kind: KeyStoreKind::None,
        }
    }

    fn persist(&self, cache: &BTreeMap<String, String>) -> Result<(), KvError> {
        let Some(writer) = &self.writer else {
            return Ok(());
        };
        let blob = if cache.is_empty() {
            None
        } else {
            Some(serde_json::to_string(cache).map_err(|e| KvError::Write(e.to_string()))?)
        };
        writer
            .send(blob)
            .map_err(|_| KvError::Write("secret writer stopped".into()))
    }
}

/// Error text from a backend, cut so a value can never ride along.
fn redact(error: &str) -> String {
    error.chars().take(120).collect()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn choose(file: Option<FileBackend>) -> (Option<Box<dyn SecretBackend>>, KeyStoreKind) {
    if let Ok(keyring) = KeyringBackend::new()
        && keyring.load().is_ok()
    {
        return (Some(Box::new(keyring)), KeyStoreKind::OsKeystore);
    }
    match file {
        Some(file) => (Some(Box::new(file)), KeyStoreKind::File),
        None => (None, KeyStoreKind::None),
    }
}

#[cfg(target_os = "ios")]
fn choose(_file: Option<FileBackend>) -> (Option<Box<dyn SecretBackend>>, KeyStoreKind) {
    match KeyringBackend::new() {
        Ok(keyring) => (Some(Box::new(keyring)), KeyStoreKind::None),
        Err(_) => (None, KeyStoreKind::None),
    }
}

#[cfg(target_os = "android")]
fn choose(file: Option<FileBackend>) -> (Option<Box<dyn SecretBackend>>, KeyStoreKind) {
    match file {
        Some(file) => (Some(Box::new(file)), KeyStoreKind::None),
        None => (None, KeyStoreKind::None),
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "ios",
    target_os = "android"
)))]
fn choose(file: Option<FileBackend>) -> (Option<Box<dyn SecretBackend>>, KeyStoreKind) {
    match file {
        Some(file) => (Some(Box::new(file)), KeyStoreKind::File),
        None => (None, KeyStoreKind::None),
    }
}

impl SecretStore for PlatformSecrets {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.lock().ok()?.get(key).cloned()
    }

    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| KvError::Write("secret cache poisoned".into()))?;
        cache.insert(key.to_owned(), value.to_owned());
        self.persist(&cache)
    }

    fn remove(&self, key: &str) {
        let Ok(mut cache) = self.cache.lock() else {
            return;
        };
        if cache.remove(key).is_some() {
            let _ = self.persist(&cache);
        }
    }

    fn kind(&self) -> KeyStoreKind {
        self.kind
    }
}

/// A shared [`PlatformSecrets`] as a [`SecretStore`] (the store is opened
/// on a blocking thread, so it travels as an `Arc`).
#[derive(Debug, Clone)]
pub struct SharedSecrets(pub Arc<PlatformSecrets>);

impl SecretStore for SharedSecrets {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        self.0.set(key, value)
    }
    fn remove(&self, key: &str) {
        self.0.remove(key);
    }
    fn kind(&self) -> KeyStoreKind {
        self.0.kind()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn file_store_is_private_and_drops_an_exposed_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("eq-secrets-{}", std::process::id()));
        let backend = FileBackend {
            path: dir.join("secrets.json"),
        };
        backend.save(Some(r#"{"eq.session":"s"}"#)).unwrap();
        let mode = std::fs::metadata(&backend.path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(backend.load().unwrap().is_some());
        std::fs::set_permissions(&backend.path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(backend.load().unwrap(), None, "exposed file is dropped");
        assert!(!backend.path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn debug_shows_no_values() {
        let store = PlatformSecrets::memory();
        store.set("eq.session", "SESSION-VALUE").unwrap();
        assert!(!format!("{store:?}").contains("SESSION-VALUE"));
    }
}
