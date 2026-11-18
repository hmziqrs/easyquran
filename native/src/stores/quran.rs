//! Quran data store (PARITY B1, B2, B4, B5): which artifacts are on the
//! device, download progress, storage usage, and async reads that run the
//! blocking `eq_core::quran` calls through `blocking(..)` and write the
//! signals back in `spawn`. Made once by the app's provider and handed
//! down by context; screens read the getters, call the methods.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

use eq_core::{
    clock::{Clock, SystemClock},
    net::Transport,
    quran::{
        catalogue::{self, DEFAULT_READER_SOURCE},
        meta::meta,
        online::{ApiAvailability, QuranApi, api_serves},
        read::{OpenSources, ReadError, ScopeRead, ServedBy, SourceRead, scope_range},
        store::{ArtifactBase, ArtifactStore, ChunkWrite, Progress, StoreError, Usage},
        types::{ArabicSource, RangeText, SourceRef},
        view::group_range,
    },
    routes::ReaderScope,
};
use nsh_core::prelude::*;

use crate::services::files;

/// One download's state for the downloads screen and progress popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Running(Progress),
    /// Stopped; `resumable` when a partial file was kept.
    Failed {
        message: String,
        resumable: bool,
    },
}

struct Shared {
    store: Arc<ArtifactStore>,
    sources: Arc<OpenSources>,
    transport: Rc<dyn Transport>,
    api: QuranApi,
    availability: RefCell<ApiAvailability>,
}

/// The app's Quran data store.
#[derive(Clone, Copy)]
pub struct QuranStore {
    installed: Signal<Vec<String>>,
    downloads: Signal<BTreeMap<String, DownloadStatus>>,
    usage: Signal<Usage>,
    shared: Stored<Rc<Shared>>,
}

fn join_error(e: impl std::fmt::Display) -> String {
    format!("worker: {e}")
}

impl QuranStore {
    /// Makes the store under the current owner: the artifact directory is
    /// `${dataDir}/quran` (development: `EQ_QURAN_DIR` read in place). Reads
    /// the inventory and seeds the bundled default script on first launch.
    pub fn new(transport: Rc<dyn Transport>, api: QuranApi) -> Self {
        let root = files::quran_dir()
            .unwrap_or_else(|| std::env::temp_dir().join("easyquran").join("quran"));
        let base =
            files::dev_quran_dir().map_or_else(ArtifactBase::default, ArtifactBase::Directory);
        let store = Self {
            installed: signal(Vec::new()),
            downloads: signal(BTreeMap::new()),
            usage: signal(Usage::default()),
            shared: stored(Rc::new(Shared {
                store: Arc::new(ArtifactStore::new(root, base)),
                sources: Arc::new(OpenSources::new()),
                transport,
                api,
                availability: RefCell::new(ApiAvailability::default()),
            })),
        };
        store.seed_bundled();
        store
    }

    fn shared(&self) -> Rc<Shared> {
        self.shared.get()
    }

    /// Ids of the artifacts on the device (tracked).
    pub fn installed(&self) -> Vec<String> {
        self.installed.get()
    }

    /// Whether `id` can be read offline (tracked).
    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.with(|ids| ids.iter().any(|i| i == id))
    }

    /// Downloads in flight or failed, by id (tracked).
    pub fn downloads(&self) -> BTreeMap<String, DownloadStatus> {
        self.downloads.get()
    }

    /// Bytes on the device (tracked).
    pub fn usage(&self) -> Usage {
        self.usage.get()
    }

    /// Re-reads the inventory and usage from disk.
    pub fn refresh(&self) {
        let this = *self;
        let shared = self.shared();
        spawn(async move {
            let store = shared.store.clone();
            let result = blocking(move || {
                let mut ids = store.installed();
                // Development files count as installed, never in usage.
                for a in catalogue::all_artifacts() {
                    if !ids.contains(&a.id) && store.path_for(&a.id).is_some() {
                        ids.push(a.id);
                    }
                }
                (ids, store.usage())
            })
            .await;
            if let Ok((ids, usage)) = result {
                this.installed.set(ids);
                this.usage.set(usage);
            }
        });
    }

    /// Installs the bundled default script (`just quran-bundle`) when the
    /// device has none, then refreshes the inventory.
    pub fn seed_bundled(&self) {
        let this = *self;
        let shared = self.shared();
        spawn(async move {
            let id = DEFAULT_READER_SOURCE.id();
            let artifact = catalogue::arabic_profile(DEFAULT_READER_SOURCE);
            let present = shared.store.path_for(id).is_some();
            if !present {
                let file = artifact
                    .r2_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(artifact.r2_path);
                match files::read_asset(&format!("quran/{file}")).await {
                    Ok(bytes) => {
                        let store = shared.store.clone();
                        let seeded = blocking(move || store.seed(id, &bytes)).await;
                        if let Ok(Err(e)) = seeded {
                            log::warn!("quran seed {id}: {e}");
                        }
                    }
                    Err(e) => log::info!("quran seed {id}: no bundled asset ({e})"),
                }
            }
            this.refresh();
        });
    }

    fn set_status(&self, id: &str, status: Option<DownloadStatus>) {
        self.downloads.update(|map| {
            match status {
                Some(s) => map.insert(id.to_owned(), s),
                None => map.remove(id),
            };
        });
    }

    /// Downloads `id` with progress, resuming a kept partial. A running
    /// download of the same id is not started twice.
    pub fn download(&self, id: &str) {
        let running = self
            .downloads
            .with_untracked(|m| matches!(m.get(id), Some(DownloadStatus::Running(_))));
        if running {
            return;
        }
        let this = *self;
        let id = id.to_owned();
        self.set_status(
            &id,
            Some(DownloadStatus::Running(Progress {
                loaded: 0,
                total: 0,
            })),
        );
        spawn(async move {
            let result = this.run_download(&id).await;
            match result {
                Ok(()) => this.set_status(&id, None),
                Err(e) => this.set_status(
                    &id,
                    Some(DownloadStatus::Failed {
                        resumable: e.is_resumable(),
                        message: e.to_string(),
                    }),
                ),
            }
            this.refresh();
        });
    }

    async fn run_download(&self, id: &str) -> Result<(), StoreError> {
        let shared = self.shared();
        let store = shared.store.clone();
        let owned = id.to_owned();
        let job = blocking(move || store.begin(&owned))
            .await
            .map_err(|e| StoreError::Io(join_error(e)))??;
        let Some(mut job) = job else {
            return Ok(());
        };
        self.set_status(id, Some(DownloadStatus::Running(job.progress())));
        let discard = |shared: &Shared| shared.store.discard(id);
        while let Some(request) = job.next_request() {
            let response = match shared.transport.send(request).await {
                Ok(r) => r,
                Err(e) => {
                    let err = StoreError::Net(e);
                    if !err.is_resumable() {
                        discard(&shared);
                    }
                    return Err(err);
                }
            };
            let chunk: ChunkWrite = match job.accept(response) {
                Ok(c) => c,
                Err(e) => {
                    discard(&shared);
                    return Err(e);
                }
            };
            let (store, j) = (shared.store.clone(), job.clone());
            let written = blocking(move || store.write_chunk(&j, &chunk))
                .await
                .map_err(|e| StoreError::Io(join_error(e)))?;
            if let Err(e) = written {
                discard(&shared);
                return Err(e);
            }
            self.set_status(id, Some(DownloadStatus::Running(job.progress())));
        }
        let (store, j) = (shared.store.clone(), job.clone());
        blocking(move || store.finish(&j))
            .await
            .map_err(|e| StoreError::Io(join_error(e)))?
            .map(|_| ())
    }

    /// Deletes a downloaded artifact (never a development file).
    pub fn delete(&self, id: &str) {
        let this = *self;
        let shared = self.shared();
        let id = id.to_owned();
        shared.sources.forget(&id);
        spawn(async move {
            let store = shared.store.clone();
            let target = id.clone();
            if let Ok(Err(e)) = blocking(move || store.delete(&target)).await {
                log::warn!("quran delete {id}: {e}");
            }
            this.set_status(&id, None);
            this.refresh();
        });
    }

    /// Reads `from..=to` of one source: from the device when installed,
    /// else the read API (through its breaker).
    pub async fn read_source(
        &self,
        source_id: &str,
        from: u32,
        to: u32,
    ) -> Result<(RangeText, ServedBy), ReadError> {
        let shared = self.shared();
        let (store, sources, id) = (
            shared.store.clone(),
            shared.sources.clone(),
            source_id.to_owned(),
        );
        let local = blocking(move || sources.read_local(&store, meta(), &id, from, to))
            .await
            .map_err(|e| ReadError::NotAvailable(join_error(e)))??;
        if let Some(text) = local {
            return Ok((text, ServedBy::Local));
        }
        let online = match SourceRef::from_id(source_id) {
            SourceRef::Arabic(s) => api_serves(s),
            SourceRef::Translation(_) => catalogue::translation(source_id).is_some(),
        };
        if !online {
            return Err(ReadError::NotAvailable(source_id.to_owned()));
        }
        let now = SystemClock.unix_ms();
        let text = shared
            .api
            .read_range_guarded(
                shared.transport.as_ref(),
                &shared.availability,
                now,
                source_id,
                from,
                to,
                meta(),
            )
            .await?;
        Ok((text, ServedBy::Api))
    }

    /// Reads a reader scope: Arabic groups (openers cut, sajdas marked)
    /// plus one lane per translation, each local-first.
    pub async fn read_scope(
        &self,
        scope: ReaderScope,
        arabic: ArabicSource,
        translations: Vec<String>,
    ) -> Result<ScopeRead, ReadError> {
        let (from, to) = scope_range(meta(), &scope).ok_or(ReadError::OutOfRange)?;
        let arabic = match self.read_source(arabic.id(), from, to).await {
            Ok((text, by)) => group_range(&text.ayahs, &text.normalizations, meta())
                .map(|groups| (groups, by))
                .map_err(ReadError::NotAvailable),
            Err(e) => Err(e),
        };
        let mut lanes = Vec::with_capacity(translations.len());
        for id in translations {
            let result = self.read_source(&id, from, to).await;
            lanes.push(SourceRead {
                source_id: id,
                result,
            });
        }
        Ok(ScopeRead {
            from,
            to,
            arabic,
            translations: lanes,
        })
    }
}
