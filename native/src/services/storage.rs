//! `eq_core::kv::KeyValue` over `nsh_core::storage::Storage`, and the
//! platform `eq_core::kv::SecretStore` for the session value and device key
//! (PARITY I1, I8; contracts/devices.md D3).
//!
//! Secrets live in `nsh-secure-storage`, one item per key under the store
//! name `fyi.easyquran.app.secrets`. What protects them, per the library's
//! backends (`docs/platforms.md`, `src/options.rs`):
//!
//! | Platform | Store | `key_store` claimed |
//! | --- | --- | --- |
//! | iOS | Keychain, `firstUnlockThisDevice` | `os_keystore`: a Keychain item, not Secure Enclave wrapping (the library's `use_secure_enclave` is not built) |
//! | Android | AES-256-GCM under an AndroidKeyStore key, ciphertext in `no_backup` | `os_keystore`: the library does not require or check StrongBox / TEE backing |
//! | macOS | login Keychain (file keychain; the data protection keychain needs a signed bundle) | `os_keystore` |
//! | Windows | Credential Manager | `os_keystore` |
//! | Linux | Secret Service | `os_keystore` |
//!
//! When the library's store refuses at start (no Secret Service, locked
//! keychain, ...), the secrets fall back to `secrets.json`, mode 0600, in
//! the app data dir, claiming `file`; no data dir either: memory only,
//! `none`. A later run with a working store moves the file into it.
//!
//! First run after the move to the library: the old blob (the `keyring`
//! item `fyi.easyquran.app`/`secrets` on Apple and Windows, or
//! `secrets.json`) is read once, each entry written to the new store, and
//! the old copy deleted once every entry landed.
//!
//! All secrets are loaded once by [`PlatformSecrets::open`] (blocking: call
//! it inside `blocking(..)`) and written in order by a worker thread, so no
//! keychain or file call runs on the engine thread. Values never reach a
//! log.

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

/// `secrets.json` in the app data dir, owner read/write only: the fallback
/// store, and the old store migrated from.
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

    /// The blob, `None` when there is none.
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

    /// Keeps `blob`; `None` removes it.
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

    /// The entries, empty when there is no readable blob.
    fn entries(&self) -> BTreeMap<String, String> {
        match self.load() {
            Ok(Some(blob)) => parse_blob(&blob),
            Ok(None) => BTreeMap::new(),
            Err(e) => {
                log::warn!("secret file unreadable: {}", redact(&e));
                BTreeMap::new()
            }
        }
    }

    fn persist(&self, cache: &BTreeMap<String, String>) {
        let saved = if cache.is_empty() {
            self.save(None)
        } else {
            serde_json::to_string(cache)
                .map_err(|e| e.to_string())
                .and_then(|blob| self.save(Some(&blob)))
        };
        if let Err(e) = saved {
            log::warn!("secret file write failed: {}", redact(&e));
        }
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

/// The old one-blob format: a JSON object of key to value.
fn parse_blob(blob: &str) -> BTreeMap<String, String> {
    serde_json::from_str(blob).unwrap_or_default()
}

/// Error text from a backend, cut so a value can never ride along.
fn redact(error: &str) -> String {
    error.chars().take(120).collect()
}

/// A write for the worker.
enum Command {
    Set(String, String),
    Remove(String),
}

/// The platform [`SecretStore`]: a cache in memory, persisted in order by
/// a worker thread.
pub struct PlatformSecrets {
    cache: Mutex<BTreeMap<String, String>>,
    writer: Option<mpsc::Sender<Command>>,
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
    /// keychain or file read, waited for here): call inside `blocking(..)`.
    /// `data_dir` is `nsh_core::element::data_dir()`.
    pub fn open(data_dir: Option<PathBuf>) -> Arc<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (loaded_tx, loaded_rx) = mpsc::sync_channel(1);
            let (tx, rx) = mpsc::channel::<Command>();
            let spawned = std::thread::Builder::new()
                .name("eq-secrets".into())
                .spawn(move || worker::run(data_dir, &loaded_tx, &rx));
            if spawned.is_err() {
                return Arc::new(Self::memory());
            }
            let Ok((cache, kind, persists)) = loaded_rx.recv() else {
                return Arc::new(Self::memory());
            };
            Arc::new(Self {
                cache: Mutex::new(cache),
                writer: persists.then_some(tx),
                kind,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = data_dir;
            Arc::new(Self::memory())
        }
    }

    /// A store that keeps nothing past this run and claims `none`.
    pub fn memory() -> Self {
        Self {
            cache: Mutex::new(BTreeMap::new()),
            writer: None,
            kind: KeyStoreKind::None,
        }
    }

    fn send(&self, command: Command) -> Result<(), KvError> {
        let Some(writer) = &self.writer else {
            return Ok(());
        };
        writer
            .send(command)
            .map_err(|_| KvError::Write("secret writer stopped".into()))
    }
}

/// The worker thread: owns the library's store (its handle is not `Send`)
/// on a runtime of its own, whose blocking pool runs the platform calls.
#[cfg(not(target_arch = "wasm32"))]
mod worker {
    use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};

    use eq_core::kv::KeyStoreKind;
    use nsh_secure_storage::{SecureStorage, SecureStorageOptions};

    use super::{Command, FileBackend, parse_blob, redact};

    /// The store name of every item.
    const STORE: &str = "fyi.easyquran.app.secrets";

    /// What the open loaded: the entries, the kind, whether writes persist.
    pub(super) type Loaded = (BTreeMap<String, String>, KeyStoreKind, bool);

    enum Sink {
        Library(SecureStorage),
        File(FileBackend, BTreeMap<String, String>),
    }

    fn options() -> SecureStorageOptions {
        let mut options = SecureStorageOptions::default();
        let platform = &mut options.platform;
        platform.ios.apple.account_name = STORE.to_owned();
        // Background sync reads the session after the first unlock; never
        // in a backup or on another device.
        platform.ios.apple.accessibility =
            nsh_secure_storage::KeychainAccessibility::FirstUnlockThisDevice;
        platform.macos.apple.account_name = STORE.to_owned();
        // The data protection keychain needs a signed bundle with an
        // access group; the login keychain works for every build.
        platform.macos.uses_data_protection_keychain = false;
        platform.windows.account_name = STORE.to_owned();
        platform.linux.account_name = STORE.to_owned();
        options
    }

    pub(super) fn run(
        data_dir: Option<PathBuf>,
        loaded: &mpsc::SyncSender<Loaded>,
        commands: &mpsc::Receiver<Command>,
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let Ok(runtime) = runtime else {
            let _ = loaded.send((BTreeMap::new(), KeyStoreKind::None, false));
            return;
        };
        runtime.block_on(async move {
            let file = data_dir.map(|dir| FileBackend {
                path: dir.join("secrets.json"),
            });
            let Some((mut sink, cache, kind)) = start(file).await else {
                let _ = loaded.send((BTreeMap::new(), KeyStoreKind::None, false));
                return;
            };
            if loaded.send((cache, kind, true)).is_err() {
                return;
            }
            while let Ok(command) = commands.recv() {
                apply(&mut sink, command).await;
            }
        });
    }

    /// Opens the library's store, migrating the old ones into it; the file
    /// when the library refuses; `None` with neither.
    async fn start(
        file: Option<FileBackend>,
    ) -> Option<(Sink, BTreeMap<String, String>, KeyStoreKind)> {
        let store = SecureStorage::new(options());
        match store.read_all(&Default::default()).await {
            Ok(entries) => {
                let mut cache: BTreeMap<String, String> = entries
                    .into_iter()
                    .map(|(k, v)| (k, v.as_str().to_owned()))
                    .collect();
                migrate_legacy(store, file.as_ref(), &mut cache).await;
                Some((Sink::Library(store), cache, KeyStoreKind::OsKeystore))
            }
            Err(e) => {
                log::warn!("secure storage unavailable ({}), using the file", e.code());
                let file = file?;
                let cache = file.entries();
                Some((Sink::File(file, cache.clone()), cache, KeyStoreKind::File))
            }
        }
    }

    async fn apply(sink: &mut Sink, command: Command) {
        match sink {
            Sink::Library(store) => {
                let done = match &command {
                    Command::Set(key, value) => {
                        store.write(key, Some(value), &Default::default()).await
                    }
                    Command::Remove(key) => store.delete(key, &Default::default()).await,
                };
                if let Err(e) = done {
                    log::warn!("secret write failed: {}", e.code());
                }
            }
            Sink::File(file, cache) => {
                match command {
                    Command::Set(key, value) => {
                        cache.insert(key, value);
                    }
                    Command::Remove(key) => {
                        cache.remove(&key);
                    }
                }
                file.persist(cache);
            }
        }
    }

    /// Writes each `old` entry the store lacks into `store` and `cache`.
    /// `true` when every one landed, so the old copy may go.
    pub(super) async fn migrate(
        store: SecureStorage,
        old: BTreeMap<String, String>,
        cache: &mut BTreeMap<String, String>,
    ) -> bool {
        let mut all = true;
        for (key, value) in old {
            if cache.contains_key(&key) {
                continue;
            }
            match store.write(&key, Some(&value), &Default::default()).await {
                Ok(()) => {
                    cache.insert(key, value);
                }
                Err(e) => {
                    log::warn!("secret migration write failed: {}", e.code());
                    all = false;
                }
            }
        }
        all
    }

    /// The old stores: the `keyring` blob (Apple, Windows) and the file.
    async fn migrate_legacy(
        store: SecureStorage,
        file: Option<&FileBackend>,
        cache: &mut BTreeMap<String, String>,
    ) {
        legacy::migrate_keyring(store, cache).await;
        let Some(file) = file else {
            return;
        };
        let old = file.entries();
        if old.is_empty() {
            return;
        }
        if migrate(store, old, cache).await
            && let Err(e) = file.save(None)
        {
            log::warn!("old secret file not removed: {}", redact(&e));
        }
    }

    /// The `keyring` item `fyi.easyquran.app` / `secrets` of earlier
    /// builds. On Apple it is a generic password the library reads by the
    /// same service and account; on Windows a `keyring` credential.
    mod legacy {
        use std::collections::BTreeMap;

        use nsh_secure_storage::SecureStorage;

        #[cfg(target_vendor = "apple")]
        pub(super) async fn migrate_keyring(
            store: SecureStorage,
            cache: &mut BTreeMap<String, String>,
        ) {
            let mut options = nsh_secure_storage::SecureStorageOptions::default();
            options.platform.ios.apple.account_name = "fyi.easyquran.app".to_owned();
            options.platform.macos.apple.account_name = "fyi.easyquran.app".to_owned();
            options.platform.macos.uses_data_protection_keychain = false;
            let old = SecureStorage::new(options);
            let blob = match old.read("secrets", &Default::default()).await {
                Ok(Some(blob)) => blob,
                Ok(None) => return,
                Err(e) => {
                    log::warn!("old keychain item unreadable: {}", e.code());
                    return;
                }
            };
            let entries = super::parse_blob(&blob);
            if super::migrate(store, entries, cache).await
                && let Err(e) = old.delete("secrets", &Default::default()).await
            {
                log::warn!("old keychain item not removed: {}", e.code());
            }
        }

        #[cfg(target_os = "windows")]
        pub(super) async fn migrate_keyring(
            store: SecureStorage,
            cache: &mut BTreeMap<String, String>,
        ) {
            let Ok(entry) = keyring::Entry::new("fyi.easyquran.app", "secrets") else {
                return;
            };
            let read = nsh_core::prelude::blocking(move || {
                let blob = entry.get_password();
                (entry, blob)
            })
            .await;
            let Ok((entry, Ok(blob))) = read else {
                return;
            };
            let entries = super::parse_blob(&blob);
            if super::migrate(store, entries, cache).await {
                let _ = nsh_core::prelude::blocking(move || entry.delete_credential()).await;
            }
        }

        #[cfg(not(any(target_vendor = "apple", target_os = "windows")))]
        pub(super) async fn migrate_keyring(
            _store: SecureStorage,
            _cache: &mut BTreeMap<String, String>,
        ) {
        }
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
        self.send(Command::Set(key.to_owned(), value.to_owned()))
    }

    fn remove(&self, key: &str) {
        let Ok(mut cache) = self.cache.lock() else {
            return;
        };
        if cache.remove(key).is_some() {
            let _ = self.send(Command::Remove(key.to_owned()));
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

    /// Migration copies the old blob's entries the new store lacks, never
    /// overwrites a newer value, and reports success so the old copy goes.
    #[test]
    fn migration_moves_missing_entries_and_keeps_newer_ones() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let store = nsh_secure_storage::SecureStorage::memory();
            store
                .write("eq.session", Some("new"), &Default::default())
                .await
                .unwrap();
            let mut cache = BTreeMap::from([("eq.session".to_owned(), "new".to_owned())]);
            let old = parse_blob(r#"{"eq.session":"old","eq.device.key.7":"k"}"#);
            assert!(worker::migrate(store, old, &mut cache).await);
            assert_eq!(cache["eq.session"], "new");
            assert_eq!(cache["eq.device.key.7"], "k");
            let stored = store
                .read("eq.device.key.7", &Default::default())
                .await
                .unwrap();
            assert_eq!(stored.as_deref().map(String::as_str), Some("k"));
        });
    }
}
