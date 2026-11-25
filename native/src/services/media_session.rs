//! The system's media controls for the recitation player (PARITY D2, D3):
//! the web's `navigator.mediaSession` (`engine.svelte.ts`
//! `#wireMediaSession`, `#updateMediaSession`). NSH 0.7.1 has no such API,
//! so each platform's is called here:
//!
//! - iOS, macOS: `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`; on
//!   iOS the audio session too, which lets playback go on in the background
//!   (`UIBackgroundModes` in `Cargo.toml`).
//! - Android: `MediaSession`, its notification and a foreground service
//!   (`android/playback/java/fyi/easyquran/media/Playback.java`).
//! - Windows: System Media Transport Controls. Linux: MPRIS.
//!
//! Commands reach the sink on any thread; `stores::audio` answers them with
//! the dock buttons' own actions.

#[cfg_attr(
    any(target_os = "macos", target_os = "ios"),
    path = "media_session/apple.rs"
)]
#[cfg_attr(target_os = "android", path = "media_session/android.rs")]
#[cfg_attr(target_os = "windows", path = "media_session/windows.rs")]
#[cfg_attr(target_os = "linux", path = "media_session/linux.rs")]
#[cfg_attr(
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
        target_os = "windows",
        target_os = "linux"
    )),
    path = "media_session/unsupported.rs"
)]
mod platform;

#[cfg(target_os = "android")]
pub(in crate::services) use platform::context as android_context;

/// What the system asks for: the actions the web sets handlers for, and the
/// toggle a headset's one button sends (the browser resolves that itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Command {
    Play = 0,
    Pause = 1,
    Toggle = 2,
    Previous = 3,
    Next = 4,
    SeekBackward = 5,
    SeekForward = 6,
}

impl Command {
    /// The command numbered `n` (`Playback.java`).
    pub fn from_number(n: i32) -> Option<Self> {
        use Command::{Next, Pause, Play, Previous, SeekBackward, SeekForward, Toggle};
        [
            Play,
            Pause,
            Toggle,
            Previous,
            Next,
            SeekBackward,
            SeekForward,
        ]
        .into_iter()
        .find(|c| *c as i32 == n)
    }
}

/// Takes the system's commands, on any thread.
pub type Sink = Box<dyn Fn(Command) + Send + Sync>;

/// The web's `MediaMetadata`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub title: String,
    pub artist: String,
    pub album: String,
}

/// What plays, as the system shows it. The browser reads all but the
/// metadata off the media element.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NowPlaying {
    pub metadata: Metadata,
    pub playing: bool,
    /// Seconds into the ayah.
    pub position: f64,
    /// Its length, 0 while unknown.
    pub duration: f64,
    pub rate: f32,
}

/// Texts of the controls Android draws from the app's own.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub channel: String,
    pub play: String,
    pub pause: String,
    pub previous: String,
    pub next: String,
}

/// Wires the system's commands to `sink`. Once, at the first play.
pub fn install(sink: Sink, labels: &Labels) {
    platform::install(sink, labels);
}

/// Shows `now`, or nothing.
pub fn update(now: Option<&NowPlaying>) {
    platform::update(now);
}
