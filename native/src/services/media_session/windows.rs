//! System Media Transport Controls: those of a `MediaPlayer` that plays
//! nothing, which need no window handle.

use std::sync::OnceLock;

use windows::{
    Foundation::{TimeSpan, TypedEventHandler},
    Media::{
        MediaPlaybackStatus, MediaPlaybackType, Playback::MediaPlayer,
        SystemMediaTransportControls, SystemMediaTransportControlsButton as Button,
        SystemMediaTransportControlsButtonPressedEventArgs as Pressed,
        SystemMediaTransportControlsTimelineProperties,
    },
    core::{HSTRING, Ref, Result},
};

use super::{Command, Labels, NowPlaying, Sink};

struct Controls {
    /// The controls are its own: they go with it.
    _player: MediaPlayer,
    controls: SystemMediaTransportControls,
}

static CONTROLS: OnceLock<Controls> = OnceLock::new();

fn command(button: Button) -> Option<Command> {
    match button {
        Button::Play => Some(Command::Play),
        Button::Pause => Some(Command::Pause),
        Button::Previous => Some(Command::Previous),
        Button::Next => Some(Command::Next),
        Button::Rewind => Some(Command::SeekBackward),
        Button::FastForward => Some(Command::SeekForward),
        _ => None,
    }
}

fn make(sink: Sink) -> Result<Controls> {
    let player = MediaPlayer::new()?;
    // Its own handling of the buttons would answer them first.
    player.CommandManager()?.SetIsEnabled(false)?;
    let controls = player.SystemMediaTransportControls()?;
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetIsRewindEnabled(true)?;
    controls.SetIsFastForwardEnabled(true)?;
    controls.ButtonPressed(&TypedEventHandler::new(
        move |_: Ref<'_, SystemMediaTransportControls>, pressed: Ref<'_, Pressed>| {
            if let Some(command) = command(pressed.ok()?.Button()?) {
                sink(command);
            }
            Ok(())
        },
    ))?;
    Ok(Controls {
        _player: player,
        controls,
    })
}

pub fn install(sink: Sink, _: &Labels) {
    if CONTROLS.get().is_some() {
        return;
    }
    match make(sink) {
        Ok(controls) => {
            let _ = CONTROLS.set(controls);
        }
        Err(error) => log::warn!("audio: no media controls: {error}"),
    }
}

pub fn update(now: Option<&NowPlaying>) {
    if let Some(Controls { controls, .. }) = CONTROLS.get()
        && let Err(error) = show(controls, now)
    {
        log::warn!("audio: media controls: {error}");
    }
}

fn show(controls: &SystemMediaTransportControls, now: Option<&NowPlaying>) -> Result<()> {
    let Some(now) = now else {
        controls.SetPlaybackStatus(MediaPlaybackStatus::Closed)?;
        return controls.SetIsEnabled(false);
    };
    controls.SetIsEnabled(true)?;
    controls.SetPlaybackStatus(status(now))?;
    controls.SetPlaybackRate(f64::from(now.rate))?;
    let updater = controls.DisplayUpdater()?;
    updater.SetType(MediaPlaybackType::Music)?;
    let music = updater.MusicProperties()?;
    music.SetTitle(&HSTRING::from(&now.metadata.title))?;
    music.SetArtist(&HSTRING::from(&now.metadata.artist))?;
    music.SetAlbumTitle(&HSTRING::from(&now.metadata.album))?;
    updater.Update()?;
    let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
    timeline.SetStartTime(time(0.0))?;
    timeline.SetEndTime(time(now.duration))?;
    timeline.SetPosition(time(now.position))?;
    controls.UpdateTimelineProperties(&timeline)
}

fn status(now: &NowPlaying) -> MediaPlaybackStatus {
    if now.playing {
        return MediaPlaybackStatus::Playing;
    }
    MediaPlaybackStatus::Paused
}

/// A `TimeSpan`, in its 100 ns units.
fn time(seconds: f64) -> TimeSpan {
    TimeSpan {
        Duration: (seconds * 10_000_000.0) as i64,
    }
}
