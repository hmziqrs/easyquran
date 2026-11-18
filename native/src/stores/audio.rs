//! The recitation player (PARITY D2, D4): web `lib/audio/engine.svelte.ts`.
//! One app-wide instance, so playback and the queue survive navigation.
//! Source-agnostic: every file comes from `eq_core::audio::url`, every
//! limit from `eq_core::audio::policy`. Dark while the catalogue lists no
//! recording ([`enabled`]), as on the web.
//!
//! State is three globals (main state, progress, offline busy) read through
//! tracked getters on [`AudioStore`]; the running play (task, player) lives
//! in a thread-local, cancelled as a whole by a generation bump.

use std::{cell::RefCell, sync::OnceLock, time::Duration};

use eq_core::audio::{
    catalogue::{self, AudioCatalogue, AudioRecording},
    offline::CacheResult,
    policy::{DEFAULT_PLAYBACK_RATE, nearest_rate, retry_delay_ms},
    queue::{PlayQueue, QueueItem, QueueSegment, build_queue},
    repeat::{RepeatAction, RepeatPlanInput, RepeatRun, RepeatStep, normalize_plan, plan_steps},
    session::{SavedSession, load_session, save_session},
    url::{AyahRef, is_allowed_audio_url, resolve_ayah_audio},
};
use nsh_core::prelude::*;

use crate::services::{
    audio::{self as player, Outcome, PlayObserver, SharedPlayer},
    storage::NshKv,
};

/// Debug builds only: a recording template to test the player with while
/// no source is licensed (`{surah}`/`{ayah}`, 3-digit). An absolute path
/// under `${dataDir}` plays a local file; an `https://` URL also needs its
/// host in `nsh.json`. Never read in release builds.
pub const DEV_TEMPLATE_ENV: &str = "EQ_AUDIO_DEV_TEMPLATE";
/// Debug builds with [`DEV_TEMPLATE_ENV`]: `S:A` to play at launch.
pub const DEV_AUTOPLAY_ENV: &str = "EQ_AUDIO_DEV_AUTOPLAY";

fn dev_template() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(DEV_TEMPLATE_ENV)
        .ok()
        .filter(|t| !t.trim().is_empty())
}

fn dev_recording(template: String) -> AudioRecording {
    use catalogue::{AudioAttribution, AudioLicence, VerseTiming};
    AudioRecording {
        id: "dev".into(),
        reciter_id: "dev".into(),
        label: "Development".into(),
        url_template: template,
        timing: VerseTiming::FilePerAyah,
        licence: AudioLicence {
            name: "Development".into(),
            url: "https://easyquran.fyi".into(),
        },
        attribution: AudioAttribution {
            en: String::new(),
            ar: String::new(),
            url: "https://easyquran.fyi".into(),
        },
    }
}

/// The catalogue: the baked one, plus the debug-only dev recording.
pub fn catalogue() -> &'static AudioCatalogue {
    static CATALOGUE: OnceLock<AudioCatalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let mut c = catalogue::baked();
        if let Some(template) = dev_template() {
            c.reciters.push(catalogue::AudioReciter {
                id: "dev".into(),
                name: "Development".into(),
            });
            c.recordings.push(dev_recording(template));
        }
        c
    })
}

/// `AUDIO_ENABLED`: gate for every audio surface.
pub fn enabled() -> bool {
    catalogue().enabled()
}

/// `isAllowedAudioUrl`, plus a dev recording's local file in debug builds.
fn playable(url: &str) -> bool {
    if is_allowed_audio_url(url) {
        return true;
    }
    dev_template().is_some() && std::path::Path::new(url).is_absolute()
}

/// `AudioStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioStatus {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Error,
}

/// A range the reader asked the repeat panel to load; `seq` changes on
/// every request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatRequest {
    pub surah: u16,
    pub from: u16,
    pub to: u16,
    pub seq: u64,
}

#[derive(Clone)]
struct State {
    status: AudioStatus,
    recording_id: Option<String>,
    queue: Option<PlayQueue>,
    repeat_step: Option<RepeatStep>,
    rate: f32,
    attempt: u32,
    panel_open: bool,
    repeat_request: Option<RepeatRequest>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            status: AudioStatus::Idle,
            recording_id: None,
            queue: None,
            repeat_step: None,
            rate: DEFAULT_PLAYBACK_RATE,
            attempt: 0,
            panel_open: false,
            repeat_request: None,
        }
    }
}

/// Seconds into the current ayah and its length.
#[derive(Clone, Copy, Default, PartialEq)]
struct Progress {
    position: f64,
    duration: f64,
}

#[derive(Clone, Copy, Default)]
struct Caching(bool);

fn state() -> Signal<State> {
    global::<State>()
}

fn progress() -> Signal<Progress> {
    global::<Progress>()
}

#[derive(Default)]
struct Runtime {
    owner: Option<Owner>,
    generation: u64,
    task: Option<Spawned<()>>,
    player: Option<SharedPlayer>,
    start_sec: f64,
    hydrated: bool,
}

thread_local! {
    static RT: RefCell<Runtime> = RefCell::new(Runtime::default());
}

fn live(generation: u64) -> bool {
    RT.with(|rt| rt.borrow().generation == generation)
}

fn set_status(status: AudioStatus) {
    state().update(|s| s.status = status);
}

fn status_now() -> AudioStatus {
    state().with_untracked(|s| s.status)
}

/// Stops the active run (queue or repeat) and frees its player.
fn cancel() {
    let (task, player) = RT.with(|rt| {
        let mut rt = rt.borrow_mut();
        rt.generation += 1;
        (rt.task.take(), rt.player.take())
    });
    if let Some(player) = player {
        player.borrow().close();
    }
    if let Some(task) = task {
        task.cancel();
    }
}

/// Starts `run` as the active task, under the dock's owner so it outlives
/// the row or button that started it.
fn start(run: impl FnOnce(u64) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
    cancel();
    let (generation, owner) = RT.with(|rt| {
        let rt = rt.borrow();
        (rt.generation, rt.owner)
    });
    let future = run(generation);
    let task = match owner {
        Some(owner) => owner.run(|| spawn(future)),
        None => spawn(future),
    };
    RT.with(|rt| {
        let mut rt = rt.borrow_mut();
        if rt.generation == generation {
            rt.task = Some(task);
        }
    });
}

fn finish(generation: u64) {
    RT.with(|rt| {
        let mut rt = rt.borrow_mut();
        if rt.generation == generation {
            rt.task = None;
        }
    });
}

fn persist() {
    let saved = state().with_untracked(|s| {
        Some(SavedSession {
            recording_id: s.recording_id.clone()?,
            segments: s.queue.as_ref()?.segments.clone(),
            index: s.queue.as_ref()?.index,
            rate: s.rate,
        })
    });
    if let Some(saved) = saved {
        save_session(&NshKv, &saved);
    }
}

struct Observer;

impl PlayObserver for Observer {
    fn playing(&self) {
        state().update(|s| {
            s.status = AudioStatus::Playing;
            s.attempt = 0;
        });
    }

    fn paused(&self) {
        if status_now() == AudioStatus::Playing {
            set_status(AudioStatus::Paused);
        }
    }

    fn buffering(&self, waiting: bool) {
        match (waiting, status_now()) {
            (true, AudioStatus::Playing) => set_status(AudioStatus::Loading),
            (false, AudioStatus::Loading) => set_status(AudioStatus::Playing),
            _ => {}
        }
    }

    fn progress(&self, position: f64, duration: Option<f64>) {
        progress().update(|p| {
            p.position = position;
            if let Some(d) = duration {
                p.duration = d;
            }
        });
    }
}

/// Thrown by the web when an ayah still fails after every retry.
struct LoadError;

/// `#playItem`: one ayah with retries.
async fn play_item(
    generation: u64,
    recording: &AudioRecording,
    item: AyahRef,
) -> Result<Outcome, LoadError> {
    let mut attempt = 0;
    loop {
        if !live(generation) {
            return Ok(Outcome::Aborted);
        }
        let resolved = resolve_ayah_audio(recording, item, None).ok_or(LoadError)?;
        if !playable(&resolved.url) {
            return Err(LoadError);
        }
        set_status(AudioStatus::Loading);
        let local = player::local_file(&resolved.url).await;
        if !live(generation) {
            return Ok(Outcome::Aborted);
        }
        let rate = state().with_untracked(|s| s.rate);
        progress().set(Progress::default());
        let src = local.unwrap_or_else(|| resolved.url.clone());
        log::info!("audio: playing {}:{}", item.surah, item.ayah);
        let shared = player::open_player(&src, rate);
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.player = Some(shared.clone());
            rt.start_sec = resolved.start_sec;
        });
        let outcome =
            player::play_span(&shared, resolved.start_sec, resolved.end_sec, &Observer).await;
        match outcome {
            Outcome::Ended | Outcome::Aborted => return Ok(outcome),
            Outcome::Blocked => {
                set_status(AudioStatus::Paused);
                return Ok(Outcome::Blocked);
            }
            Outcome::Error => {}
        }
        attempt += 1;
        let wait = retry_delay_ms(attempt).ok_or(LoadError)?;
        state().update(|s| {
            s.attempt = attempt;
            s.status = AudioStatus::Loading;
        });
        timer(Duration::from_millis(wait)).await;
    }
}

fn current_recording() -> Option<AudioRecording> {
    let id = state().with_untracked(|s| s.recording_id.clone())?;
    catalogue().recording_by_id(&id).cloned()
}

/// `#drive`: plays the queue from its index to its end.
async fn drive(generation: u64) {
    loop {
        if !live(generation) {
            return;
        }
        let queue = state().with_untracked(|s| s.queue.clone());
        let recording = current_recording();
        let (Some(queue), Some(recording)) = (queue, recording) else {
            set_status(AudioStatus::Error);
            return;
        };
        let Some(item) = queue.current() else {
            set_status(AudioStatus::Error);
            return;
        };
        let at = AyahRef {
            surah: item.surah,
            ayah: item.ayah,
        };
        match play_item(generation, &recording, at).await {
            Ok(Outcome::Ended) => {}
            Ok(_) => return,
            Err(LoadError) => {
                if live(generation) {
                    set_status(AudioStatus::Error);
                }
                return;
            }
        }
        let Some(next) = queue.step(true) else {
            set_status(AudioStatus::Idle);
            return;
        };
        state().update(|s| s.queue = Some(next));
        persist();
    }
}

fn run_queue() {
    state().update(|s| s.repeat_step = None);
    start(|generation| {
        Box::pin(async move {
            drive(generation).await;
            finish(generation);
        })
    });
}

async fn drive_repeat(generation: u64, recording: AudioRecording, mut run: RepeatRun) {
    loop {
        match run.next_action() {
            RepeatAction::Play(step) => {
                state().update(|s| s.repeat_step = Some(step));
                let at = AyahRef {
                    surah: step.surah,
                    ayah: step.ayah,
                };
                match play_item(generation, &recording, at).await {
                    Ok(Outcome::Ended) => {}
                    Ok(Outcome::Aborted) => return,
                    _ => {
                        if live(generation) {
                            set_status(AudioStatus::Error);
                        }
                        return;
                    }
                }
            }
            RepeatAction::Wait(ms) => timer(Duration::from_millis(ms)).await,
            RepeatAction::Done => {
                set_status(AudioStatus::Idle);
                return;
            }
            RepeatAction::Cancelled => return,
        }
    }
}

/// The player store handle (app-wide; no context needed).
#[derive(Clone, Copy)]
pub struct AudioStore;

pub fn audio_store() -> AudioStore {
    AudioStore
}

impl AudioStore {
    /// Binds the player's tasks to the dock's owner (lives with the shell).
    /// Once, from the dock's body.
    pub fn install(self) {
        let owner = Owner::current();
        RT.with(|rt| rt.borrow_mut().owner = Some(owner));
        if dev_template().is_some() {
            log::info!(
                "audio: dev recording on, data dir {:?}",
                crate::services::files::app_data_dir()
            );
            self.dev_autoplay();
        }
    }

    /// Debug builds with the dev recording: `EQ_AUDIO_DEV_AUTOPLAY=S:A`
    /// plays from that verse to the surah's end at launch (player smoke test).
    fn dev_autoplay(self) {
        let Some((surah, ayah)) = std::env::var(DEV_AUTOPLAY_ENV).ok().and_then(|v| {
            let (s, a) = v.split_once(':')?;
            Some((s.parse::<u16>().ok()?, a.parse::<u16>().ok()?))
        }) else {
            return;
        };
        let count = eq_core::quran::meta::meta()
            .surah(surah)
            .map_or(0, |s| s.ayah_count);
        let segment = QueueSegment {
            surah,
            from: ayah,
            to: count,
        };
        self.play_queue("dev", &[segment]);
    }

    pub fn status(self) -> AudioStatus {
        state().with(|s| s.status)
    }

    /// Playing or loading (the dock's pause face).
    pub fn is_playing(self) -> bool {
        matches!(self.status(), AudioStatus::Playing | AudioStatus::Loading)
    }

    pub fn recording(self) -> Option<AudioRecording> {
        let id = state().with(|s| s.recording_id.clone())?;
        catalogue().recording_by_id(&id).cloned()
    }

    pub fn queue(self) -> Option<PlayQueue> {
        state().with(|s| s.queue.clone())
    }

    /// The ayah at the queue's index.
    pub fn item(self) -> Option<QueueItem> {
        state().with(|s| s.queue.as_ref().and_then(PlayQueue::current))
    }

    /// The verse the player is on (`(surah, ayah)`) while it is not idle;
    /// tracked, for a reader highlight.
    pub fn playing_verse(self) -> Option<(u16, u16)> {
        state().with(|s| {
            if s.status == AudioStatus::Idle {
                return None;
            }
            if let Some(step) = s.repeat_step {
                return Some((step.surah, step.ayah));
            }
            s.queue
                .as_ref()
                .and_then(PlayQueue::current)
                .map(|i| (i.surah, i.ayah))
        })
    }

    pub fn repeat_step(self) -> Option<RepeatStep> {
        state().with(|s| s.repeat_step)
    }

    pub fn position(self) -> f64 {
        progress().with(|p| p.position)
    }

    pub fn duration(self) -> f64 {
        progress().with(|p| p.duration)
    }

    pub fn rate(self) -> f32 {
        state().with(|s| s.rate)
    }

    pub fn attempt(self) -> u32 {
        state().with(|s| s.attempt)
    }

    pub fn caching(self) -> bool {
        global::<Caching>().get().0
    }

    pub fn panel_open(self) -> bool {
        state().with(|s| s.panel_open)
    }

    pub fn repeat_request(self) -> Option<RepeatRequest> {
        state().with(|s| s.repeat_request)
    }

    /// `hydrate`: restores the saved queue paused. Never autoplays. Once.
    pub fn hydrate(self) {
        let first = RT.with(|rt| !std::mem::replace(&mut rt.borrow_mut().hydrated, true));
        if !first {
            return;
        }
        let Some(saved) = load_session(&NshKv) else {
            return;
        };
        if catalogue().recording_by_id(&saved.recording_id).is_none() {
            return;
        }
        let Some(queue) = build_queue(&saved.segments) else {
            return;
        };
        let queue = queue.at(saved.index).unwrap_or(queue);
        state().update(|s| {
            s.recording_id = Some(saved.recording_id);
            s.queue = Some(queue);
            s.rate = nearest_rate(f64::from(saved.rate));
            s.status = AudioStatus::Paused;
        });
    }

    /// `playQueue`: replaces the queue and starts it from its first ayah.
    pub fn play_queue(self, recording_id: &str, segments: &[QueueSegment]) -> bool {
        if catalogue().recording_by_id(recording_id).is_none() {
            return false;
        }
        let Some(queue) = build_queue(segments) else {
            return false;
        };
        state().update(|s| {
            s.recording_id = Some(recording_id.to_owned());
            s.queue = Some(queue);
        });
        persist();
        run_queue();
        true
    }

    pub fn play(self) {
        let resumable = RT.with(|rt| {
            let rt = rt.borrow();
            rt.task.is_some().then(|| rt.player.clone()).flatten()
        });
        if let Some(shared) = resumable
            && status_now() == AudioStatus::Paused
        {
            shared.borrow().play();
            return;
        }
        if state().with_untracked(|s| s.queue.is_some()) {
            run_queue();
        }
    }

    pub fn pause(self) {
        let shared = RT.with(|rt| rt.borrow().player.clone());
        if let Some(shared) = shared {
            shared.borrow().pause();
        }
        if matches!(status_now(), AudioStatus::Playing | AudioStatus::Loading) {
            set_status(AudioStatus::Paused);
        }
    }

    pub fn toggle(self) {
        if matches!(status_now(), AudioStatus::Playing | AudioStatus::Loading) {
            self.pause();
            return;
        }
        self.play();
    }

    fn move_to(self, next: Option<PlayQueue>) {
        let Some(next) = next else {
            return;
        };
        state().update(|s| s.queue = Some(next));
        persist();
        run_queue();
    }

    fn with_queue<R>(self, f: impl FnOnce(&PlayQueue) -> Option<R>) -> Option<R> {
        state().with_untracked(|s| s.queue.as_ref().and_then(f))
    }

    pub fn next(self) {
        self.move_to(self.with_queue(|q| q.step(true)));
    }

    pub fn prev(self) {
        self.move_to(self.with_queue(|q| q.step(false)));
    }

    pub fn next_surah(self) {
        if let Some(i) = self.with_queue(|q| q.surah_boundary(true)) {
            self.jump_to(i);
        }
    }

    pub fn prev_surah(self) {
        if let Some(i) = self.with_queue(|q| q.surah_boundary(false)) {
            self.jump_to(i);
        }
    }

    pub fn jump_to(self, index: usize) {
        self.move_to(self.with_queue(|q| q.at(index)));
    }

    /// Seconds into the current ayah, clamped to its length.
    pub fn seek(self, sec: f64) {
        if !sec.is_finite() {
            return;
        }
        let (shared, start) = RT.with(|rt| {
            let rt = rt.borrow();
            (rt.player.clone(), rt.start_sec)
        });
        let Some(shared) = shared else {
            return;
        };
        let span = progress().with_untracked(|p| p.duration).max(0.0);
        let at = start + sec.clamp(0.0, span);
        shared.borrow().seek(Duration::from_secs_f64(at));
    }

    pub fn set_rate(self, value: f64) {
        let rate = nearest_rate(value);
        state().update(|s| s.rate = rate);
        let shared = RT.with(|rt| rt.borrow().player.clone());
        if let Some(shared) = shared {
            let player = shared.borrow();
            let mut settings = player.settings();
            settings.rate = rate;
            player.set_settings(settings);
        }
        persist();
    }

    pub fn set_panel_open(self, open: bool) {
        state().update(|s| s.panel_open = open);
    }

    /// Opens the full player with the repeat panel set to this range.
    pub fn request_repeat(self, surah: u16, from: u16, to: u16) {
        state().update(|s| {
            let seq = s.repeat_request.map_or(0, |r| r.seq) + 1;
            s.repeat_request = Some(RepeatRequest {
                surah,
                from,
                to,
                seq,
            });
            s.panel_open = true;
        });
    }

    /// Retries the current ayah after a failure.
    pub fn retry(self) {
        if state().with_untracked(|s| s.queue.is_some()) {
            run_queue();
        }
    }

    /// Stops playback and any repeat run; keeps the queue.
    pub fn stop(self) {
        cancel();
        state().update(|s| {
            s.repeat_step = None;
            s.status = if s.queue.is_some() {
                AudioStatus::Paused
            } else {
                AudioStatus::Idle
            };
        });
    }

    /// `startRepeat`: false when the plan is invalid.
    pub fn start_repeat(self, recording_id: &str, input: RepeatPlanInput, ayah_count: u16) -> bool {
        let Some(recording) = catalogue().recording_by_id(recording_id).cloned() else {
            return false;
        };
        let Some(plan) = normalize_plan(input, ayah_count) else {
            return false;
        };
        state().update(|s| s.recording_id = Some(recording.id.clone()));
        let run = RepeatRun::new(plan_steps(&plan), plan.delay_ms);
        start(move |generation| {
            Box::pin(async move {
                drive_repeat(generation, recording, run).await;
                if live(generation) {
                    state().update(|s| s.repeat_step = None);
                }
                finish(generation);
            })
        });
        true
    }

    pub fn stop_repeat(self) {
        if state().with_untracked(|s| s.repeat_step.is_none()) {
            return;
        }
        self.stop();
    }

    /// `cacheQueue`: saves every file of the queue for offline listening.
    pub async fn cache_queue(self) -> CacheResult {
        let recording = current_recording();
        let queue = state().with_untracked(|s| s.queue.clone());
        let (Some(recording), Some(queue)) = (recording, queue) else {
            return CacheResult::Failed;
        };
        let mut urls: Vec<String> = Vec::new();
        for item in &queue.items {
            let at = AyahRef {
                surah: item.surah,
                ayah: item.ayah,
            };
            if let Some(resolved) = resolve_ayah_audio(&recording, at, None)
                && !urls.contains(&resolved.url)
            {
                urls.push(resolved.url);
            }
        }
        global::<Caching>().set(Caching(true));
        let mut result = CacheResult::Ok;
        for url in &urls {
            result = player::cache_file(url).await;
            if result != CacheResult::Ok {
                break;
            }
        }
        global::<Caching>().set(Caching(false));
        result
    }
}
