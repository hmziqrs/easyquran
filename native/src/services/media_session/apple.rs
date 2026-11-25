//! Now Playing and remote commands (`MediaPlayer.framework`), and on iOS the
//! audio session. Loaded and messaged dynamically, as `notifications` does.
//! Everything runs on the main queue, in the order it was asked.

use std::{
    ffi::CStr,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use block2::RcBlock;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSArray, NSString};

use super::{Command, Labels, NowPlaying, Sink};

static SINK: OnceLock<Sink> = OnceLock::new();
/// What was last shown plays (the interruption observer's question).
static PLAYING: AtomicBool = AtomicBool::new(false);

/// The seconds of the web's `seekbackward` and `seekforward`.
const SEEK_SECONDS: f64 = 10.0;

fn send(command: Command) {
    if let Some(sink) = SINK.get() {
        sink(command);
    }
}

/// `MediaPlayer.framework`'s handle.
fn framework() -> Option<usize> {
    static LOADED: OnceLock<usize> = OnceLock::new();
    let handle = *LOADED.get_or_init(|| {
        let path: &CStr = c"/System/Library/Frameworks/MediaPlayer.framework/MediaPlayer";
        // SAFETY: dynamically loads the system framework; handle stays open.
        unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY) as usize }
    });
    (handle != 0).then_some(handle)
}

/// The framework's `NSString` constant `name`.
fn constant(name: &CStr) -> Option<Retained<NSString>> {
    let handle = framework()? as *mut libc::c_void;
    // SAFETY: each `name` is a documented `NSString * const` of the
    // framework, which is never unloaded.
    unsafe {
        let symbol = libc::dlsym(handle, name.as_ptr()).cast::<*mut NSString>();
        Retained::retain(*symbol.as_ref()?)
    }
}

/// Runs `work` on the main queue.
fn on_main(work: impl Fn() + Send + 'static) {
    let Some(class) = AnyClass::get(c"NSOperationQueue") else {
        return;
    };
    let block = RcBlock::new(work);
    // SAFETY: documented Foundation API, callable from any thread; the
    // queue copies the block, whose captures are `Send`.
    unsafe {
        let queue: Retained<AnyObject> = msg_send![class, mainQueue];
        let () = msg_send![&*queue, addOperationWithBlock: &*block];
    }
}

fn info_center() -> Option<Retained<AnyObject>> {
    framework()?;
    let class = AnyClass::get(c"MPNowPlayingInfoCenter")?;
    // SAFETY: documented class method.
    Some(unsafe { msg_send![class, defaultCenter] })
}

fn command_center() -> Option<Retained<AnyObject>> {
    framework()?;
    let class = AnyClass::get(c"MPRemoteCommandCenter")?;
    // SAFETY: documented class method.
    Some(unsafe { msg_send![class, sharedCommandCenter] })
}

/// Answers `remote` (an `MPRemoteCommand`) with `command`.
fn handle(remote: &AnyObject, command: Command) {
    let handler = RcBlock::new(move |_: *mut AnyObject| -> isize {
        send(command);
        // MPRemoteCommandHandlerStatusSuccess.
        0
    });
    // SAFETY: documented method; the command keeps the block and the target
    // it answers for the app's life.
    unsafe {
        let _: Option<Retained<AnyObject>> = msg_send![remote, addTargetWithHandler: &*handler];
    }
}

/// A skip command with the web's interval.
fn handle_skip(remote: &AnyObject, command: Command) {
    let seconds = number(SEEK_SECONDS);
    if let Some(seconds) = seconds {
        let intervals = NSArray::from_retained_slice(&[seconds]);
        // SAFETY: documented property of MPSkipIntervalCommand.
        let () = unsafe { msg_send![remote, setPreferredIntervals: &*intervals] };
    }
    handle(remote, command);
}

fn number(value: f64) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSNumber")?;
    // SAFETY: documented class method.
    Some(unsafe { msg_send![class, numberWithDouble: value] })
}

pub fn install(sink: Sink, _: &Labels) {
    if SINK.set(sink).is_err() {
        return;
    }
    on_main(|| {
        let Some(center) = command_center() else {
            return;
        };
        // SAFETY: documented properties of MPRemoteCommandCenter.
        unsafe {
            let play: Retained<AnyObject> = msg_send![&*center, playCommand];
            handle(&play, Command::Play);
            let pause: Retained<AnyObject> = msg_send![&*center, pauseCommand];
            handle(&pause, Command::Pause);
            let toggle: Retained<AnyObject> = msg_send![&*center, togglePlayPauseCommand];
            handle(&toggle, Command::Toggle);
            let previous: Retained<AnyObject> = msg_send![&*center, previousTrackCommand];
            handle(&previous, Command::Previous);
            let next: Retained<AnyObject> = msg_send![&*center, nextTrackCommand];
            handle(&next, Command::Next);
            let backward: Retained<AnyObject> = msg_send![&*center, skipBackwardCommand];
            handle_skip(&backward, Command::SeekBackward);
            let forward: Retained<AnyObject> = msg_send![&*center, skipForwardCommand];
            handle_skip(&forward, Command::SeekForward);
        }
        #[cfg(target_os = "ios")]
        session::observe();
    });
}

pub fn update(now: Option<&NowPlaying>) {
    PLAYING.store(now.is_some_and(|n| n.playing), Ordering::Relaxed);
    let now = now.cloned();
    on_main(move || {
        #[cfg(target_os = "ios")]
        session::follow(now.as_ref());
        show(now.as_ref());
    });
}

fn show(now: Option<&NowPlaying>) {
    let Some(center) = info_center() else {
        return;
    };
    let info = now.and_then(dictionary);
    // MPNowPlayingPlaybackState: playing 1, paused 2, stopped 3.
    let state: usize = match now {
        Some(n) if n.playing => 1,
        Some(_) => 2,
        None => 3,
    };
    // SAFETY: documented properties of MPNowPlayingInfoCenter; nil clears
    // the info.
    unsafe {
        let () = msg_send![&*center, setNowPlayingInfo: info.as_deref()];
        let () = msg_send![&*center, setPlaybackState: state];
    }
}

/// The `nowPlayingInfo` of `now`.
fn dictionary(now: &NowPlaying) -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"NSMutableDictionary")?;
    // SAFETY: documented class method.
    let info: Retained<AnyObject> = unsafe { msg_send![class, dictionary] };
    let set = |key: &CStr, value: &AnyObject| {
        if let Some(key) = constant(key) {
            // SAFETY: documented method; the dictionary keeps both.
            let () = unsafe { msg_send![&*info, setObject: value, forKey: &*key] };
        }
    };
    set(
        c"MPMediaItemPropertyTitle",
        &NSString::from_str(&now.metadata.title),
    );
    set(
        c"MPMediaItemPropertyArtist",
        &NSString::from_str(&now.metadata.artist),
    );
    set(
        c"MPMediaItemPropertyAlbumTitle",
        &NSString::from_str(&now.metadata.album),
    );
    if now.duration > 0.0
        && let Some(duration) = number(now.duration)
    {
        set(c"MPMediaItemPropertyPlaybackDuration", &duration);
    }
    if let Some(position) = number(now.position) {
        set(c"MPNowPlayingInfoPropertyElapsedPlaybackTime", &position);
    }
    let rate = if now.playing {
        f64::from(now.rate)
    } else {
        0.0
    };
    if let Some(rate) = number(rate) {
        set(c"MPNowPlayingInfoPropertyPlaybackRate", &rate);
    }
    Some(info)
}

/// The iOS audio session: `playback` while the player is in use, so it
/// sounds with the silent switch on and goes on in the background, then
/// what it was before. NSH's players set no session, its sounds set
/// `ambient` once (nsh-platform-apple `audio.rs` `session()`), its videos
/// and speech set theirs and put back what they found, so it is set again
/// at each play and left alone when it is no longer ours.
#[cfg(target_os = "ios")]
mod session {
    use std::{cell::RefCell, ptr, sync::atomic::Ordering};

    use objc2::runtime::Bool;

    use super::{
        AnyClass, AnyObject, AtomicBool, Command, NSString, NowPlaying, PLAYING, RcBlock, Retained,
        msg_send, send,
    };

    const PLAYBACK: &str = "AVAudioSessionCategoryPlayback";
    const SPOKEN: &str = "AVAudioSessionModeSpokenAudio";
    /// AVAudioSessionSetActiveOptionNotifyOthersOnDeactivation.
    const NOTIFY_OTHERS: usize = 1;

    type Category = (Retained<NSString>, Retained<NSString>, usize);

    thread_local! {
        /// The session before the player took it, on the main thread.
        static BEFORE: RefCell<Option<Category>> = const { RefCell::new(None) };
    }

    /// A pause an interruption (a call) asked for, to undo at its end.
    static INTERRUPTED: AtomicBool = AtomicBool::new(false);

    fn shared() -> Option<Retained<AnyObject>> {
        // AVFoundation is loaded by the player (nsh-platform-apple).
        let class = AnyClass::get(c"AVAudioSession")?;
        // SAFETY: AVAudioSession's documented API.
        Some(unsafe { msg_send![class, sharedInstance] })
    }

    fn category(shared: &AnyObject) -> Category {
        // SAFETY: AVAudioSession's documented getters.
        unsafe {
            (
                msg_send![shared, category],
                msg_send![shared, mode],
                msg_send![shared, categoryOptions],
            )
        }
    }

    fn set_category(shared: &AnyObject, (category, mode, options): &Category) -> bool {
        let mut error: *mut AnyObject = ptr::null_mut();
        // SAFETY: AVAudioSession's documented API; `error` is an out parameter.
        let ok: Bool = unsafe {
            msg_send![
                shared,
                setCategory: &**category,
                mode: &**mode,
                options: *options,
                error: &mut error,
            ]
        };
        ok.as_bool()
    }

    fn is_ours((category, mode, options): &Category) -> bool {
        category.to_string() == PLAYBACK && mode.to_string() == SPOKEN && *options == 0
    }

    pub(super) fn follow(now: Option<&NowPlaying>) {
        match now {
            Some(now) if now.playing => take(),
            // Paused: kept, or the lock screen's controls would go.
            Some(_) => {}
            None => give_back(),
        }
    }

    fn take() {
        let Some(shared) = shared() else {
            return;
        };
        let current = category(&shared);
        let ours = is_ours(&current);
        BEFORE.with(|b| {
            b.borrow_mut().get_or_insert(current);
        });
        let wanted = (NSString::from_str(PLAYBACK), NSString::from_str(SPOKEN), 0);
        let mut error: *mut AnyObject = ptr::null_mut();
        // SAFETY: AVAudioSession's documented API; `error` is an out parameter.
        let active: Bool = unsafe {
            let set = ours || set_category(&shared, &wanted);
            if !set {
                log::warn!("audio: cannot set the audio session");
            }
            msg_send![&*shared, setActive: Bool::YES, error: &mut error]
        };
        if !active.as_bool() {
            log::warn!("audio: cannot activate the audio session");
        }
    }

    fn give_back() {
        INTERRUPTED.store(false, Ordering::Relaxed);
        let Some(before) = BEFORE.with(|b| b.borrow_mut().take()) else {
            return;
        };
        let Some(shared) = shared() else {
            return;
        };
        if !is_ours(&category(&shared)) {
            return;
        }
        let mut error: *mut AnyObject = ptr::null_mut();
        // SAFETY: AVAudioSession's documented API; `error` is an out parameter.
        unsafe {
            let _: Bool = msg_send![
                &*shared,
                setActive: Bool::NO,
                withOptions: NOTIFY_OTHERS,
                error: &mut error
            ];
        }
        if !set_category(&shared, &before) {
            log::warn!("audio: cannot restore the audio session");
        }
    }

    /// Follows interruptions and unplugged outputs, where the system pauses
    /// the player by itself and NSH reports nothing.
    pub(super) fn observe() {
        add_observer("AVAudioSessionInterruptionNotification", |note| {
            // AVAudioSessionInterruptionTypeBegan is 1, ...Ended 0.
            match user_info(note, "AVAudioSessionInterruptionTypeKey") {
                Some(1) => {
                    INTERRUPTED.store(PLAYING.load(Ordering::Relaxed), Ordering::Relaxed);
                    send(Command::Pause);
                }
                Some(0) => {
                    // AVAudioSessionInterruptionOptionShouldResume.
                    let resume =
                        user_info(note, "AVAudioSessionInterruptionOptionKey").unwrap_or(0) & 1;
                    if INTERRUPTED.swap(false, Ordering::Relaxed) && resume == 1 {
                        send(Command::Play);
                    }
                }
                _ => {}
            }
        });
        add_observer("AVAudioSessionRouteChangeNotification", |note| {
            // AVAudioSessionRouteChangeReasonOldDeviceUnavailable.
            if user_info(note, "AVAudioSessionRouteChangeReasonKey") == Some(2) {
                send(Command::Pause);
            }
        });
    }

    fn add_observer(name: &str, handler: impl Fn(&AnyObject) + 'static) {
        let (Some(centers), Some(queues)) = (
            AnyClass::get(c"NSNotificationCenter"),
            AnyClass::get(c"NSOperationQueue"),
        ) else {
            return;
        };
        let block = RcBlock::new(move |note: *mut AnyObject| {
            // SAFETY: the center passes a valid notification.
            if let Some(note) = unsafe { note.as_ref() } {
                handler(note);
            }
        });
        // SAFETY: documented Foundation API, delivered on the main queue; the
        // center keeps the block and its token for the app's life.
        unsafe {
            let center: Retained<AnyObject> = msg_send![centers, defaultCenter];
            let main: Retained<AnyObject> = msg_send![queues, mainQueue];
            let _: Option<Retained<AnyObject>> = msg_send![
                &*center,
                addObserverForName: &*NSString::from_str(name),
                object: ptr::null::<AnyObject>(),
                queue: &*main,
                usingBlock: &*block
            ];
        }
    }

    /// The number under `key` in a notification's `userInfo`.
    fn user_info(note: &AnyObject, key: &str) -> Option<usize> {
        // SAFETY: documented getters; the session's keys hold NSNumbers.
        unsafe {
            let info: Option<Retained<AnyObject>> = msg_send![note, userInfo];
            let value: Option<Retained<AnyObject>> =
                msg_send![&*info?, objectForKey: &*NSString::from_str(key)];
            Some(msg_send![&*value?, unsignedIntegerValue])
        }
    }
}
