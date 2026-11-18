//! On-device artifact store (PARITY B5; mirror of `workers/download.ts`,
//! `opfs-cache.ts`, `opfs-retention.ts`, `environment.ts`). One directory
//! holds `<id>.sqlite` per complete artifact and `<id>.sqlite.tmp` while one
//! downloads. A file is never opened unless complete: it is promoted from
//! its temp name only after its size matches the baked map and it passes
//! the staging check. An integrity failure (short or long body, size
//! mismatch, bad DB) discards the temp; a network failure keeps it so the
//! next attempt resumes with a `Range` request.
//!
//! All file work here is blocking: the app crate runs it through
//! `blocking(..)`. [`download`] is the async convenience that drives a
//! whole download over a [`Transport`] and does the file steps inline (for
//! tests and non-UI callers).

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use super::{
    catalogue::{self, Artifact, ArtifactKind},
    db,
};
use crate::net::{NetError, Request, Response, Transport};

/// Production artifact delivery: the web's `/_quran` proxy on the site
/// origin (`QURAN_ARTIFACT_DELIVERY_BASE` + `SITE.url`).
pub const PRODUCTION_ARTIFACT_BASE: &str = "https://easyquran.fyi/_quran";
/// Bytes per ranged request.
pub const CHUNK_BYTES: u64 = 512 * 1024;
/// Translations unused this long may be pruned (`TTL_MS`).
pub const RETENTION_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// At most this many cached translations (`CAP_COUNT`).
pub const RETENTION_CAP_COUNT: usize = 128;
/// At most this many cached translation bytes (`CAP_BYTES`).
pub const RETENTION_CAP_BYTES: u64 = 256 * 1024 * 1024;

const ACTIVE_SUFFIX: &str = ".sqlite";
const TEMP_SUFFIX: &str = ".sqlite.tmp";
const LAST_USED_FILE: &str = "last-used.json";

/// Where artifacts come from (`resolveQuranArtifactBase`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactBase {
    /// An HTTP base; `<base>/<artifact path>` is fetched.
    Remote(String),
    /// Development: a local copy of the R2 tree (`db/quran` of the
    /// monorepo: `arabic/…`, `translations/…`). Files are read in place,
    /// never copied, never deleted.
    Directory(PathBuf),
}

impl Default for ArtifactBase {
    fn default() -> Self {
        ArtifactBase::Remote(PRODUCTION_ARTIFACT_BASE.to_owned())
    }
}

impl ArtifactBase {
    /// The URL of an artifact (remote bases only).
    pub fn url(&self, artifact: &Artifact) -> Option<String> {
        match self {
            ArtifactBase::Remote(base) => {
                Some(format!("{}/{}", base.trim_end_matches('/'), artifact.path))
            }
            ArtifactBase::Directory(_) => None,
        }
    }

    /// The in-place file of an artifact (directory bases only): the R2 key
    /// without its `tanzil/` bucket prefix.
    pub fn local_file(&self, artifact: &Artifact) -> Option<PathBuf> {
        match self {
            ArtifactBase::Directory(dir) => {
                let rel = artifact
                    .path
                    .strip_prefix("tanzil/")
                    .unwrap_or(&artifact.path);
                Some(dir.join(rel))
            }
            ArtifactBase::Remote(_) => None,
        }
    }
}

/// Why a store operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("unknown artifact {0}")]
    Unknown(String),
    #[error("network: {0}")]
    Net(NetError),
    #[error("http {0}")]
    Http(u16),
    #[error("{id}: size {got} != expected {expected}")]
    SizeMismatch { id: String, expected: u64, got: u64 },
    #[error("{0}: truncated or malformed chunk")]
    Truncated(String),
    #[error("{0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(String),
    #[error("{0} is a development file and is never deleted")]
    DevReadOnly(String),
}

impl StoreError {
    /// A failure after which the partial file is worth keeping (resume).
    pub fn is_resumable(&self) -> bool {
        matches!(self, StoreError::Net(NetError::Offline(_)))
    }
}

fn io(e: std::io::Error) -> StoreError {
    StoreError::Io(e.to_string())
}

/// An artifact's state on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactState {
    Absent,
    /// A temp file of this many bytes waits to resume.
    Partial {
        bytes: u64,
    },
    Present {
        bytes: u64,
    },
}

/// Bytes per artifact on the device.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Usage {
    /// `(id, bytes)` of complete artifacts, sorted by id.
    pub artifacts: Vec<(String, u64)>,
    /// Bytes of temp files waiting to resume.
    pub partial_bytes: u64,
    pub total_bytes: u64,
}

/// Download progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub loaded: u64,
    pub total: u64,
}

/// One download in flight: the pure request planner. Ask it for the next
/// request, hand back the response, write what it returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadJob {
    pub artifact: Artifact,
    pub url: String,
    /// Bytes already in the temp file.
    pub offset: u64,
}

/// What to do with a response's bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkWrite {
    /// Append to the temp file.
    Append(Vec<u8>),
    /// The server sent the whole file (ignored the range): replace the temp.
    Replace(Vec<u8>),
}

impl DownloadJob {
    /// Progress so far.
    pub fn progress(&self) -> Progress {
        Progress {
            loaded: self.offset,
            total: self.artifact.size_bytes,
        }
    }

    /// Whether every byte is in.
    pub fn is_complete(&self) -> bool {
        self.offset >= self.artifact.size_bytes
    }

    /// The next ranged request; `None` when complete.
    pub fn next_request(&self) -> Option<Request> {
        if self.is_complete() {
            return None;
        }
        let end = (self.offset + CHUNK_BYTES).min(self.artifact.size_bytes) - 1;
        Some(
            Request::get(self.url.clone())
                .header("accept-encoding", "identity")
                .header("range", format!("bytes={}-{end}", self.offset)),
        )
    }

    /// Checks a response to [`Self::next_request`] and advances the offset.
    /// Anything but the exact requested bytes (or the exact whole file) is
    /// refused: the caller then discards the temp.
    pub fn accept(&mut self, response: Response) -> Result<ChunkWrite, StoreError> {
        let size = self.artifact.size_bytes;
        let id = self.artifact.id.clone();
        match response.status {
            206 => {
                let end = (self.offset + CHUNK_BYTES).min(size);
                let want = end - self.offset;
                if let Some(range) = response.header("content-range") {
                    let starts_right = range
                        .strip_prefix("bytes ")
                        .and_then(|r| r.split('-').next())
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        == Some(self.offset);
                    if !starts_right {
                        return Err(StoreError::Truncated(id));
                    }
                }
                if response.body.len() as u64 != want {
                    return Err(StoreError::Truncated(id));
                }
                self.offset = end;
                Ok(ChunkWrite::Append(response.body))
            }
            200 => {
                let got = response.body.len() as u64;
                if got != size {
                    return Err(StoreError::SizeMismatch {
                        id,
                        expected: size,
                        got,
                    });
                }
                self.offset = size;
                Ok(ChunkWrite::Replace(response.body))
            }
            status => Err(StoreError::Http(status)),
        }
    }
}

/// The artifact store over one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStore {
    root: PathBuf,
    base: ArtifactBase,
}

fn file_len(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

impl ArtifactStore {
    /// A store in `root` (created on first write), fetching from `base`.
    pub fn new(root: impl Into<PathBuf>, base: ArtifactBase) -> Self {
        Self {
            root: root.into(),
            base,
        }
    }

    /// Its directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Its artifact base.
    pub fn base(&self) -> &ArtifactBase {
        &self.base
    }

    fn artifact(id: &str) -> Result<Artifact, StoreError> {
        catalogue::artifact(id).ok_or_else(|| StoreError::Unknown(id.to_owned()))
    }

    fn active_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}{ACTIVE_SUFFIX}"))
    }

    fn temp_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}{TEMP_SUFFIX}"))
    }

    /// The development file of `artifact` when it is there at the baked size.
    fn dev_file(&self, artifact: &Artifact) -> Option<PathBuf> {
        let path = self.base.local_file(artifact)?;
        (file_len(&path) == Some(artifact.size_bytes)).then_some(path)
    }

    /// The complete file to open for `id`, or `None`: only a file at the
    /// baked size is ever returned.
    pub fn path_for(&self, id: &str) -> Option<PathBuf> {
        let artifact = catalogue::artifact(id)?;
        if let Some(dev) = self.dev_file(&artifact) {
            return Some(dev);
        }
        let path = self.active_path(id);
        (file_len(&path) == Some(artifact.size_bytes)).then_some(path)
    }

    /// The state of `id`.
    pub fn state(&self, id: &str) -> ArtifactState {
        if let Some(path) = self.path_for(id) {
            return ArtifactState::Present {
                bytes: file_len(&path).unwrap_or(0),
            };
        }
        match file_len(&self.temp_path(id)) {
            Some(bytes) => ArtifactState::Partial { bytes },
            None => ArtifactState::Absent,
        }
    }

    /// Starts or resumes a download of `id`. `Ok(None)` when it is already
    /// present (or the base is a directory that holds it). A temp file
    /// larger than the artifact is discarded.
    pub fn begin(&self, id: &str) -> Result<Option<DownloadJob>, StoreError> {
        let artifact = Self::artifact(id)?;
        if self.path_for(id).is_some() {
            return Ok(None);
        }
        let Some(url) = self.base.url(&artifact) else {
            return Err(StoreError::Invalid(format!(
                "{id}: not in the development directory"
            )));
        };
        fs::create_dir_all(&self.root).map_err(io)?;
        let temp = self.temp_path(id);
        let mut offset = file_len(&temp).unwrap_or(0);
        if offset > artifact.size_bytes {
            fs::remove_file(&temp).map_err(io)?;
            offset = 0;
        }
        Ok(Some(DownloadJob {
            artifact,
            url,
            offset,
        }))
    }

    /// Writes an accepted chunk into the temp file.
    pub fn write_chunk(&self, job: &DownloadJob, chunk: &ChunkWrite) -> Result<(), StoreError> {
        let temp = self.temp_path(&job.artifact.id);
        match chunk {
            ChunkWrite::Append(bytes) => {
                let mut f = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&temp)
                    .map_err(io)?;
                f.write_all(bytes).map_err(io)?;
                f.sync_data().map_err(io)
            }
            ChunkWrite::Replace(bytes) => {
                let mut f = fs::File::create(&temp).map_err(io)?;
                f.write_all(bytes).map_err(io)?;
                f.sync_all().map_err(io)
            }
        }
    }

    /// Promotes a complete temp: exact size, staging check, then rename.
    /// On any failure the temp is discarded.
    pub fn finish(&self, job: &DownloadJob) -> Result<PathBuf, StoreError> {
        let id = &job.artifact.id;
        let result = self.promote(id, job.artifact.size_bytes);
        if result.is_err() {
            self.discard(id);
        }
        result
    }

    fn promote(&self, id: &str, size: u64) -> Result<PathBuf, StoreError> {
        let temp = self.temp_path(id);
        let got = file_len(&temp).unwrap_or(0);
        if got != size {
            return Err(StoreError::SizeMismatch {
                id: id.to_owned(),
                expected: size,
                got,
            });
        }
        db::validate_artifact(&temp, id).map_err(|e| StoreError::Invalid(e.to_string()))?;
        let active = self.active_path(id);
        fs::rename(&temp, &active).map_err(io)?;
        if file_len(&active) != Some(size) {
            let _ = fs::remove_file(&active);
            return Err(StoreError::Invalid(format!(
                "{id}: active file missing after switch"
            )));
        }
        Ok(active)
    }

    /// Drops the temp of `id` (failed or cancelled download).
    pub fn discard(&self, id: &str) {
        let _ = fs::remove_file(self.temp_path(id));
    }

    /// Installs `id` from bytes in hand (the bundled default script on first
    /// launch). Same checks as a download; no-op when already present.
    pub fn seed(&self, id: &str, bytes: &[u8]) -> Result<PathBuf, StoreError> {
        let artifact = Self::artifact(id)?;
        if let Some(path) = self.path_for(id) {
            return Ok(path);
        }
        if bytes.len() as u64 != artifact.size_bytes {
            return Err(StoreError::SizeMismatch {
                id: id.to_owned(),
                expected: artifact.size_bytes,
                got: bytes.len() as u64,
            });
        }
        fs::create_dir_all(&self.root).map_err(io)?;
        let job = DownloadJob {
            url: String::new(),
            offset: 0,
            artifact,
        };
        self.write_chunk(&job, &ChunkWrite::Replace(bytes.to_vec()))?;
        self.finish(&job)
    }

    /// Deletes a complete artifact (and any temp). Development files are
    /// never touched.
    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let artifact = Self::artifact(id)?;
        if self.dev_file(&artifact).is_some() {
            return Err(StoreError::DevReadOnly(id.to_owned()));
        }
        self.discard(id);
        match fs::remove_file(self.active_path(id)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
        let mut used = self.read_last_used();
        if used.remove(id).is_some() {
            self.write_last_used(&used)?;
        }
        Ok(())
    }

    /// Removes every temp file (`sweepAbandonedTemps`): call when no
    /// download is running and resume is not wanted.
    pub fn sweep_temps(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(TEMP_SUFFIX) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    /// Ids with a complete file in the store directory (development files
    /// excluded), sorted.
    pub fn installed(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let id = name.strip_suffix(ACTIVE_SUFFIX)?.to_owned();
                let artifact = catalogue::artifact(&id)?;
                (file_len(&e.path()) == Some(artifact.size_bytes)).then_some(id)
            })
            .collect();
        ids.sort();
        ids
    }

    /// Bytes per artifact and in total.
    pub fn usage(&self) -> Usage {
        let artifacts: Vec<(String, u64)> = self
            .installed()
            .into_iter()
            .filter_map(|id| Some((id.clone(), file_len(&self.active_path(&id))?)))
            .collect();
        let partial_bytes = fs::read_dir(&self.root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_name().to_string_lossy().ends_with(TEMP_SUFFIX))
                    .filter_map(|e| file_len(&e.path()))
                    .sum()
            })
            .unwrap_or(0);
        let total_bytes = artifacts.iter().map(|(_, b)| b).sum::<u64>() + partial_bytes;
        Usage {
            artifacts,
            partial_bytes,
            total_bytes,
        }
    }

    fn read_last_used(&self) -> BTreeMap<String, i64> {
        fs::read(self.root.join(LAST_USED_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_last_used(&self, map: &BTreeMap<String, i64>) -> Result<(), StoreError> {
        fs::create_dir_all(&self.root).map_err(io)?;
        let tmp = self.root.join(format!("{LAST_USED_FILE}.tmp"));
        fs::write(&tmp, serde_json::to_vec(map).unwrap_or_default()).map_err(io)?;
        fs::rename(tmp, self.root.join(LAST_USED_FILE)).map_err(io)
    }

    /// Records a use of `id` at `now_ms` (`stampLastUsed`).
    pub fn stamp_used(&self, id: &str, now_ms: i64) -> Result<(), StoreError> {
        let mut map = self.read_last_used();
        map.insert(id.to_owned(), now_ms);
        self.write_last_used(&map)
    }

    /// Evicts translations past the retention rules (`pruneTranslations`):
    /// oldest-used first, until within count, bytes and TTL. Arabic sources
    /// and `pinned` ids are never evicted. Returns the evicted ids.
    pub fn prune(&self, now_ms: i64, pinned: &[String]) -> Vec<String> {
        let used = self.read_last_used();
        let candidates: Vec<(String, u64)> = self
            .usage()
            .artifacts
            .into_iter()
            .filter(|(id, _)| {
                catalogue::artifact(id).is_some_and(|a| a.kind == ArtifactKind::Translation)
            })
            .collect();
        let evict = compute_evictions(&candidates, &used, now_ms, pinned);
        evict
            .into_iter()
            .filter(|id| self.delete(id).is_ok())
            .collect()
    }
}

/// Which candidates to evict (`computeEvictions`).
pub fn compute_evictions(
    candidates: &[(String, u64)],
    last_used: &BTreeMap<String, i64>,
    now_ms: i64,
    pinned: &[String],
) -> Vec<String> {
    let cutoff = now_ms - RETENTION_TTL_MS;
    let mut ranked: Vec<(&str, u64, i64)> = candidates
        .iter()
        .filter(|(id, _)| !pinned.contains(id))
        .map(|(id, size)| (id.as_str(), *size, last_used.get(id).copied().unwrap_or(0)))
        .collect();
    ranked.sort_by_key(|r| r.2);
    let mut total: u64 = ranked.iter().map(|r| r.1).sum();
    let mut evicted = Vec::new();
    for (id, size, used) in &ranked {
        let remaining = ranked.len() - evicted.len();
        if remaining <= RETENTION_CAP_COUNT && total <= RETENTION_CAP_BYTES && *used >= cutoff {
            break;
        }
        evicted.push((*id).to_owned());
        total -= size;
    }
    evicted
}

/// Downloads `id` into `store` over `transport`, chunk by chunk, resuming a
/// kept temp. File steps run inline (blocking): UI callers use the stepwise
/// API ([`ArtifactStore::begin`], [`DownloadJob`], [`ArtifactStore::write_chunk`],
/// [`ArtifactStore::finish`]) with the file steps off the UI thread.
pub async fn download(
    store: &ArtifactStore,
    transport: &dyn Transport,
    id: &str,
    mut on_progress: impl FnMut(Progress),
) -> Result<PathBuf, StoreError> {
    let Some(mut job) = store.begin(id)? else {
        return store
            .path_for(id)
            .ok_or_else(|| StoreError::Unknown(id.to_owned()));
    };
    on_progress(job.progress());
    while let Some(request) = job.next_request() {
        let response = match transport.send(request).await {
            Ok(r) => r,
            Err(e) => {
                let err = StoreError::Net(e);
                if !err.is_resumable() {
                    store.discard(id);
                }
                return Err(err);
            }
        };
        let chunk = match job.accept(response) {
            Ok(c) => c,
            Err(e) => {
                store.discard(id);
                return Err(e);
            }
        };
        if let Err(e) = store.write_chunk(&job, &chunk) {
            store.discard(id);
            return Err(e);
        }
        on_progress(job.progress());
    }
    store.finish(&job)
}
