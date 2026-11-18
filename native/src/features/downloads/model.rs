//! Download state shared by the downloads screen, the settings storage
//! section and the progress popup: names, sizes, row states, the offline
//! pack, run counting and the popup's dismissal (web
//! `download-progress.ts`, `download-dismissal.svelte.ts`,
//! `offline-store.svelte.ts`).
//!
//! The offline pack on native is the Arabic text of every enabled script,
//! stored as its SQLite artifact (the web stores route responses); the
//! translations the reader opened are already artifacts on the device.

use eq_core::{
    i18n::{Key, keys},
    quran::{
        catalogue::{self, DEFAULT_READER_SOURCE},
        read::is_script_enabled,
        store::{Progress, Usage},
        types::ArabicSource,
    },
};
use nsh_core::prelude::*;

use crate::stores::{
    locale::tr,
    quran::{DownloadStatus, QuranStore},
    reader::{ReaderStore, data_environment},
};

/// `formatBytes`: "—" for nothing, else B / KB / MB / GB / TB with one
/// decimal.
pub fn format_bytes(n: u64) -> String {
    const UNITS: [(f64, &str); 4] = [
        (1024.0 * 1024.0 * 1024.0 * 1024.0, "TB"),
        (1024.0 * 1024.0 * 1024.0, "GB"),
        (1024.0 * 1024.0, "MB"),
        (1024.0, "KB"),
    ];
    if n == 0 {
        return "—".to_owned();
    }
    let value = n as f64;
    for (size, unit) in UNITS {
        if value >= size {
            return format!("{:.1} {unit}", value / size);
        }
    }
    format!("{n} B")
}

/// The Arabic scripts this build offers, in the web's `QURAN.scripts` order
/// (production gates IndoPak and Tajweed).
pub fn arabic_targets() -> Vec<ArabicSource> {
    let env = data_environment();
    ArabicSource::ALL
        .into_iter()
        .filter(|s| is_script_enabled(*s, env))
        .collect()
}

pub fn is_arabic(id: &str) -> bool {
    ArabicSource::from_id(id).is_some()
}

/// The storage copy's name of an Arabic source (`artifactName`).
fn script_key(source: ArabicSource) -> Key {
    match source {
        ArabicSource::Uthmani => keys::SETTINGS_STORAGE_SCRIPT_UTHMANI,
        ArabicSource::AnnotatedUthmani => keys::SETTINGS_STORAGE_SCRIPT_UTHMANI_ANNOTATED,
        ArabicSource::SimpleClean => keys::SETTINGS_STORAGE_SCRIPT_SIMPLE_CLEAN,
        ArabicSource::IndoPak => keys::SETTINGS_STORAGE_SCRIPT_INDOPAK,
        ArabicSource::Tajweed => keys::SETTINGS_STORAGE_SCRIPT_TAJWEED,
    }
}

/// The popup's script label (`SCRIPT_LABELS`).
pub fn popup_script_key(id: &str) -> Key {
    match ArabicSource::from_id(id) {
        Some(ArabicSource::Uthmani | ArabicSource::AnnotatedUthmani) => {
            keys::READER_DOWNLOAD_SCRIPT_UTHMANI
        }
        Some(ArabicSource::SimpleClean) => keys::READER_DOWNLOAD_SCRIPT_SIMPLE_CLEAN,
        Some(ArabicSource::IndoPak) => keys::READER_DOWNLOAD_SCRIPT_INDOPAK,
        Some(ArabicSource::Tajweed) => keys::READER_DOWNLOAD_SCRIPT_TAJWEED,
        None => keys::READER_DOWNLOAD_SCRIPT_TRANSLATION,
    }
}

/// An artifact's display name: the script's, else the catalogue's, else
/// the id. Reads the locale (call in a binding).
pub fn artifact_name(id: &str) -> String {
    if let Some(source) = ArabicSource::from_id(id) {
        return tr(script_key(source)).to_owned();
    }
    catalogue::translation(id).map_or_else(|| id.to_owned(), |e| e.name.clone())
}

/// A translation's language name (the badge).
pub fn artifact_language(id: &str) -> Option<String> {
    catalogue::translation(id).map(|e| e.language.clone())
}

/// Bytes on the device for `id`, else the catalogue's size.
pub fn size_of(id: &str, usage: &Usage) -> u64 {
    usage
        .artifacts
        .iter()
        .find(|(a, _)| a == id)
        .map(|(_, bytes)| *bytes)
        .or_else(|| catalogue::artifact(id).map(|a| a.size_bytes))
        .unwrap_or(0)
}

/// One database row's state (`DownloadRowState`, plus native's failed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Queued,
    Downloading(Progress),
    Ready,
    Missing,
    Failed,
}

impl RowState {
    pub fn key(self) -> Key {
        match self {
            RowState::Queued => keys::DOWNLOADS_STATE_QUEUED,
            RowState::Downloading(_) => keys::DOWNLOADS_STATE_DOWNLOADING,
            RowState::Ready => keys::DOWNLOADS_STATE_READY,
            RowState::Missing | RowState::Failed => keys::DOWNLOADS_STATE_MISSING,
        }
    }
}

/// `id`'s state now (tracked).
pub fn row_state(quran: QuranStore, id: &str) -> RowState {
    match quran.downloads().get(id) {
        Some(DownloadStatus::Running(p)) => RowState::Downloading(*p),
        Some(DownloadStatus::Failed { .. }) => RowState::Failed,
        None if quran.is_installed(id) => RowState::Ready,
        None if pack().with(|p| p.requested) && is_arabic(id) => RowState::Queued,
        None => RowState::Missing,
    }
}

/// The id downloading now, if any (tracked).
pub fn running(quran: QuranStore) -> Option<(String, Progress)> {
    quran.downloads().into_iter().find_map(|(id, s)| match s {
        DownloadStatus::Running(p) => Some((id, p)),
        DownloadStatus::Failed { .. } => None,
    })
}

/// `quranRunFraction`: Arabic scripts count in order (earlier ones in
/// full, the active one by its loaded bytes); a translation on its own.
pub fn quran_run_fraction(id: &str, progress: Progress) -> Option<f32> {
    if !is_arabic(id) {
        if progress.total == 0 {
            return None;
        }
        return Some((progress.loaded as f32 / progress.total as f32).min(1.0));
    }
    let sizes: Vec<(&'static str, u64)> = arabic_targets()
        .into_iter()
        .map(|s| (s.id(), catalogue::arabic_profile(s).size_bytes))
        .collect();
    let total: u64 = sizes.iter().map(|(_, b)| b).sum();
    if total == 0 {
        return None;
    }
    let mut done = 0;
    for (sid, bytes) in sizes {
        if sid == id {
            let here = progress.loaded.min(bytes);
            return Some(((done + here) as f32 / total as f32).min(1.0));
        }
        done += bytes;
    }
    None
}

/// `toPercent`.
pub fn to_percent(fraction: Option<f32>) -> u32 {
    fraction.map_or(0, |f| (f * 100.0).round() as u32)
}

// --- offline pack ---

/// The pack's state (`OfflineStatus`; native has no staging step).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackStatus {
    Off,
    Downloading,
    Active,
    Error,
}

#[derive(Clone, Default)]
pub struct Pack {
    /// The user asked for the pack and it is not complete yet.
    pub requested: bool,
}

pub fn pack() -> Signal<Pack> {
    global::<Pack>()
}

/// The pack's state now (tracked).
pub fn pack_status(quran: QuranStore) -> PackStatus {
    let targets = arabic_targets();
    if targets.iter().all(|s| quran.is_installed(s.id())) {
        return PackStatus::Active;
    }
    if !pack().with(|p| p.requested) {
        return PackStatus::Off;
    }
    let downloads = quran.downloads();
    let failed = targets
        .iter()
        .any(|s| matches!(downloads.get(s.id()), Some(DownloadStatus::Failed { .. })));
    if failed {
        return PackStatus::Error;
    }
    PackStatus::Downloading
}

/// Installed (and in-flight) bytes over the pack's total, 0-100 (tracked).
pub fn pack_percent(quran: QuranStore) -> u32 {
    let downloads = quran.downloads();
    let mut total = 0;
    let mut have = 0;
    for s in arabic_targets() {
        let bytes = catalogue::arabic_profile(s).size_bytes;
        total += bytes;
        if quran.is_installed(s.id()) {
            have += bytes;
        } else if let Some(DownloadStatus::Running(p)) = downloads.get(s.id()) {
            have += p.loaded.min(bytes);
        }
    }
    if total == 0 {
        return 0;
    }
    ((have as f64 / total as f64) * 100.0).round() as u32
}

/// The pack's toggle (`offline.enable` / `disable`): turn it on (or retry
/// after a failure), or remove the scripts the reader does not use now.
pub fn toggle_pack(quran: QuranStore) {
    match untracked(|| pack_status(quran)) {
        PackStatus::Active => {
            pack().update(|p| p.requested = false);
            let keep = [ReaderStore::get().script().id(), DEFAULT_READER_SOURCE.id()];
            for s in arabic_targets() {
                if !keep.contains(&s.id()) {
                    quran.delete(s.id());
                }
            }
        }
        PackStatus::Error => {
            for s in arabic_targets() {
                let failed = quran
                    .downloads()
                    .get(s.id())
                    .is_some_and(|d| matches!(d, DownloadStatus::Failed { .. }));
                if failed {
                    quran.download(s.id());
                }
            }
        }
        PackStatus::Off | PackStatus::Downloading => {
            pack().update(|p| p.requested = true);
            runs().update(|r| r.pack_run += 1);
        }
    }
}

/// The pack's driver: one script at a time, in order, while the pack is
/// requested; done once every script is on the device. Run from an
/// effect (the download bar's).
pub fn drive_pack(quran: QuranStore) {
    if !pack().with(|p| p.requested) {
        return;
    }
    let targets = arabic_targets();
    if targets.iter().all(|s| quran.is_installed(s.id())) {
        untracked(|| pack().update(|p| p.requested = false));
        return;
    }
    let downloads = quran.downloads();
    let busy = targets.iter().any(|s| downloads.contains_key(s.id()));
    if busy {
        return;
    }
    if let Some(next) = targets.iter().find(|s| !quran.is_installed(s.id())) {
        let id = next.id();
        untracked(|| quran.download(id));
    }
}

// --- runs and dismissal ---

/// Download runs: a run starts when downloading starts from idle.
#[derive(Clone, Default)]
pub struct Runs {
    pub quran_run: u64,
    pub quran_active: bool,
    pub pack_run: u64,
    /// The runs the popup was hidden for (in memory: a new run shows it).
    pub quran_hidden: Option<u64>,
    pub pack_hidden: Option<u64>,
}

pub fn runs() -> Signal<Runs> {
    global::<Runs>()
}

/// Counts a new Quran run when downloading starts. Run from an effect.
pub fn track_runs(quran: QuranStore) {
    let active = running(quran).is_some();
    let was = runs().with_untracked(|r| r.quran_active);
    if active != was {
        untracked(|| {
            runs().update(|r| {
                r.quran_active = active;
                if active {
                    r.quran_run += 1;
                }
            });
        });
    }
}

pub fn quran_hidden() -> bool {
    runs().with(|r| r.quran_hidden == Some(r.quran_run))
}

pub fn pack_hidden() -> bool {
    runs().with(|r| r.pack_hidden == Some(r.pack_run))
}

/// Hides the popup for the current runs (never stops a download).
pub fn hide_popup() {
    runs().update(|r| {
        r.quran_hidden = Some(r.quran_run);
        r.pack_hidden = Some(r.pack_run);
    });
}

/// Shows the popup again.
pub fn show_popup() {
    runs().update(|r| {
        r.quran_hidden = None;
        r.pack_hidden = None;
    });
}
