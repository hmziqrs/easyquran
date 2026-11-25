//! Playback over NSH audio (PARITY D2, D4): one ayah file through an
//! `AudioPlayer`, and offline files under `${dataDir}/audio` (web
//! `offline.ts`: download, budget, local lookup, clear). The queue and repeat
//! drive live in `stores::audio`.
//!
//! What NSH 0.7.1 has no API for is the app's own: the lock screen and media
//! keys, background playback (`services::media_session`) and the screen kept
//! on during a repeat (`services::wake_lock`).

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use eq_core::{
    audio::{
        offline::{
            self, CacheResult, OFFLINE_AUDIO_DIR, OfflineIndex, offline_file_name, read_index,
            write_index,
        },
        url::is_allowed_audio_url,
    },
    net::{Request, Transport},
};
use nsh_core::{
    audio::{AudioError, AudioEvent, PlayerSettings},
    prelude::*,
};

use crate::services::{files, storage::NshKv, transport::NshTransport};

/// How one ayah play ended (the web engine's `Outcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ended,
    Error,
    /// The platform refused to play (`NotAllowedError`).
    Blocked,
    /// The player closed under it (the run was cancelled).
    Aborted,
}

/// What a play reports while it runs.
pub trait PlayObserver {
    fn playing(&self);
    fn paused(&self);
    fn buffering(&self, waiting: bool);
    /// Seconds into the ayah, and its length when known.
    fn progress(&self, position: f64, duration: Option<f64>);
}

/// A shared player: the drive task polls it, handlers pause and resume it.
pub type SharedPlayer = Rc<RefCell<AudioPlayer>>;

/// Opens a player for `src` at `rate` (nothing loads until played).
pub fn open_player(src: &str, rate: f32) -> SharedPlayer {
    let mut settings = PlayerSettings::default();
    settings.rate = rate;
    Rc::new(RefCell::new(audio_player(src, settings)))
}

async fn next_event(player: &SharedPlayer) -> Option<AudioEvent> {
    // Borrowed only while polled, so handlers can pause between polls.
    std::future::poll_fn(|cx| player.borrow_mut().poll_next(cx)).await
}

/// Plays `player` from `start_sec` to `end_sec` (or its end), reporting to
/// `observer`, until it ends, fails or closes.
pub async fn play_span(
    player: &SharedPlayer,
    start_sec: f64,
    end_sec: Option<f64>,
    observer: &dyn PlayObserver,
) -> Outcome {
    player.borrow().play();
    let mut length: Option<f64> = None;
    while let Some(event) = next_event(player).await {
        match event {
            AudioEvent::Load { duration } => {
                if start_sec > 0.0 {
                    player.borrow().seek(Duration::from_secs_f64(start_sec));
                }
                let end = end_sec.or(duration.map(|d| d.as_secs_f64()));
                length = end.map(|e| (e - start_sec).max(0.0));
                observer.progress(0.0, length);
            }
            AudioEvent::Playing => {
                log::info!("audio: playback started");
                observer.playing();
            }
            AudioEvent::Paused => observer.paused(),
            AudioEvent::Buffer { buffering } => observer.buffering(buffering),
            AudioEvent::Progress { position, duration } => {
                let at = position.as_secs_f64();
                if length.is_none() {
                    let end = end_sec.or(duration.map(|d| d.as_secs_f64()));
                    length = end.map(|e| (e - start_sec).max(0.0));
                }
                observer.progress((at - start_sec).max(0.0), length);
                if end_sec.is_some_and(|end| at >= end) {
                    player.borrow().pause();
                    return Outcome::Ended;
                }
            }
            AudioEvent::Ended => {
                log::info!("audio: ayah ended");
                return Outcome::Ended;
            }
            AudioEvent::Failed { error, message } => {
                log::warn!("audio: player failed ({}): {message}", error.as_str());
                if error == AudioError::NotAllowed {
                    return Outcome::Blocked;
                }
                return Outcome::Error;
            }
            _ => {}
        }
    }
    Outcome::Aborted
}

fn audio_dir() -> Option<PathBuf> {
    files::app_data_dir().map(|dir| dir.join(OFFLINE_AUDIO_DIR))
}

/// `cachedAudioObjectUrl`: the local file for `url` when it was saved and
/// is still on disk.
pub async fn local_file(url: &str) -> Option<String> {
    if !read_index(&NshKv).contains_key(url) {
        return None;
    }
    let path = audio_dir()?.join(offline_file_name(url));
    let found = blocking(move || path.is_file().then_some(path))
        .await
        .ok()
        .flatten()?;
    found.to_str().map(str::to_owned)
}

fn write_file(dir: &Path, name: &str, body: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.part"));
    std::fs::write(&tmp, body)?;
    std::fs::rename(tmp, dir.join(name))
}

/// `cacheAudioFile`: downloads one file into the offline folder, within the
/// budget.
pub async fn cache_file(url: &str) -> CacheResult {
    if !is_allowed_audio_url(url) {
        return CacheResult::Failed;
    }
    let index: OfflineIndex = read_index(&NshKv);
    if index.contains_key(url) {
        return CacheResult::Ok;
    }
    let Some(dir) = audio_dir() else {
        return CacheResult::Failed;
    };
    let response = match NshTransport.send(Request::get(url)).await {
        Ok(res) if (200..300).contains(&res.status) => res,
        Ok(res) => {
            log::info!("audio offline: HTTP {}", res.status);
            return CacheResult::Failed;
        }
        Err(e) => {
            log::info!("audio offline: {e}");
            return CacheResult::Failed;
        }
    };
    let size = response.body.len() as u64;
    if offline::admit(&index, size) != CacheResult::Ok {
        return CacheResult::Limit;
    }
    let name = offline_file_name(url);
    let body = response.body;
    let written = blocking(move || write_file(&dir, &name, &body)).await;
    if !matches!(written, Ok(Ok(()))) {
        return CacheResult::Failed;
    }
    // Re-read: another save may have landed while this one downloaded.
    let mut index = read_index(&NshKv);
    index.insert(url.to_owned(), size);
    if write_index(&NshKv, &index) {
        return CacheResult::Ok;
    }
    CacheResult::Failed
}

/// `removeCachedAudioFiles`: the folder goes, and the index is cleared
/// whatever the disk says.
pub async fn clear_files() {
    if let Some(dir) = audio_dir() {
        let _ = blocking(move || std::fs::remove_dir_all(dir)).await;
    }
    write_index(&NshKv, &OfflineIndex::new());
}
