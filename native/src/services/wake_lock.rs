//! Keeps the screen on while a repeat runs (the web's `holdScreenAwake`,
//! `repeat.ts`). NSH 0.7.1 has no such API, and `nsh-wake-lock` 0.1.0 does
//! not resolve beside it (its `wasm-bindgen` 0.2.130 against nsh-extra's
//! `=0.2.129`), so each platform's is called here. A refusal is silent, as
//! on the web.

#[cfg(any(target_os = "macos", target_os = "ios"))]
use apple as platform;
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "windows",
    target_os = "linux"
)))]
use unsupported as platform;

/// The screen stays on while this lives.
#[must_use = "dropping the guard lets the screen sleep"]
pub struct WakeLock {
    _held: Option<platform::Held>,
}

impl WakeLock {
    /// Takes the lock, where the platform gives one.
    pub fn acquire() -> Self {
        Self {
            _held: platform::acquire(),
        }
    }
}

#[cfg(target_os = "macos")]
mod apple {
    //! An IOKit power assertion.

    use std::ffi::c_void;

    use objc2::rc::Retained;
    use objc2_foundation::NSString;

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        /// The `CFStringRef`s are toll-free bridged `NSString`s.
        fn IOPMAssertionCreateWithName(
            kind: *const c_void,
            level: u32,
            name: *const c_void,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    /// `kIOPMAssertionLevelOn`.
    const LEVEL_ON: u32 = 255;

    pub struct Held(u32);

    pub fn acquire() -> Option<Held> {
        let kind = NSString::from_str("PreventUserIdleDisplaySleep");
        let name = NSString::from_str(crate::APP_NAME);
        let mut id = 0;
        // SAFETY: IOKit's documented signature; both strings outlive the
        // call and `id` is an out parameter.
        let result = unsafe {
            IOPMAssertionCreateWithName(
                Retained::as_ptr(&kind).cast(),
                LEVEL_ON,
                Retained::as_ptr(&name).cast(),
                &raw mut id,
            )
        };
        (result == 0).then_some(Held(id))
    }

    impl Drop for Held {
        fn drop(&mut self) {
            // SAFETY: the id `IOPMAssertionCreateWithName` gave, once.
            unsafe { IOPMAssertionRelease(self.0) };
        }
    }
}

#[cfg(target_os = "ios")]
mod apple {
    //! The application's idle timer, on the main queue.

    use block2::RcBlock;
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, Bool},
    };

    pub struct Held(());

    pub fn acquire() -> Option<Held> {
        idle_timer(false);
        Some(Held(()))
    }

    impl Drop for Held {
        fn drop(&mut self) {
            idle_timer(true);
        }
    }

    fn idle_timer(on: bool) {
        let Some(queues) = AnyClass::get(c"NSOperationQueue") else {
            return;
        };
        let block = RcBlock::new(move || {
            let Some(class) = AnyClass::get(c"UIApplication") else {
                return;
            };
            // SAFETY: UIApplication's documented API, on the main thread.
            unsafe {
                let app: Option<Retained<AnyObject>> = msg_send![class, sharedApplication];
                if let Some(app) = app {
                    let () = msg_send![&*app, setIdleTimerDisabled: Bool::new(!on)];
                }
            }
        });
        // SAFETY: documented Foundation API, callable from any thread; the
        // queue copies the block.
        unsafe {
            let queue: Retained<AnyObject> = msg_send![queues, mainQueue];
            let () = msg_send![&*queue, addOperationWithBlock: &*block];
        }
    }
}

#[cfg(target_os = "android")]
mod platform {
    //! A `PowerManager` screen lock (permission `WAKE_LOCK`): the window flag
    //! that is newer needs the activity, which NSH does not hand out.

    use nsh::jni::objects::{GlobalRef, JValue};

    use crate::services::media_session::android_context;

    /// `PowerManager.SCREEN_BRIGHT_WAKE_LOCK`.
    const SCREEN_BRIGHT: i32 = 0xa;

    pub struct Held(GlobalRef);

    pub fn acquire() -> Option<Held> {
        let taken = nsh::with_env(|env| {
            let service = env.new_string("power")?;
            let manager = env
                .call_method(
                    android_context(),
                    "getSystemService",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                    &[JValue::Object(&service)],
                )?
                .l()?;
            let tag = env.new_string("easyquran:repeat")?;
            let lock = env
                .call_method(
                    &manager,
                    "newWakeLock",
                    "(ILjava/lang/String;)Landroid/os/PowerManager$WakeLock;",
                    &[JValue::Int(SCREEN_BRIGHT), JValue::Object(&tag)],
                )?
                .l()?;
            env.call_method(&lock, "acquire", "()V", &[])?;
            Ok(Held(env.new_global_ref(lock)?))
        });
        taken
            .inspect_err(|e| log::info!("audio: no wake lock: {e}"))
            .ok()
    }

    impl Drop for Held {
        fn drop(&mut self) {
            let released = nsh::with_env(|env| {
                env.call_method(&self.0, "release", "()V", &[])?;
                Ok(())
            });
            if let Err(error) = released {
                log::info!("audio: wake lock: {error}");
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    //! The engine thread's execution state: taken and released there.

    use std::marker::PhantomData;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetThreadExecutionState(state: u32) -> u32;
    }

    const ES_CONTINUOUS: u32 = 0x8000_0000;
    const ES_DISPLAY_REQUIRED: u32 = 0x2;

    /// Not `Send`: the state is its thread's.
    pub struct Held(PhantomData<*const ()>);

    pub fn acquire() -> Option<Held> {
        // SAFETY: a Win32 call without pointers; 0 is a failure.
        let before = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED) };
        (before != 0).then_some(Held(PhantomData))
    }

    impl Drop for Held {
        fn drop(&mut self) {
            // SAFETY: as above.
            unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    //! `org.freedesktop.ScreenSaver` on the session bus, held by a thread:
    //! the lock lasts as long as its connection.

    use std::{
        sync::mpsc::{Receiver, Sender, channel},
        thread,
    };

    use zbus::blocking::Connection;

    const NAME: &str = "org.freedesktop.ScreenSaver";
    const PATH: &str = "/org/freedesktop/ScreenSaver";

    /// Dropping the sender ends the thread, which releases the lock.
    pub struct Held {
        _release: Sender<()>,
    }

    pub fn acquire() -> Option<Held> {
        let (tx, rx) = channel();
        thread::Builder::new()
            .name("eq-wake-lock".to_owned())
            .spawn(move || {
                if let Err(error) = hold(&rx) {
                    log::info!("audio: no wake lock: {error}");
                }
            })
            .ok()?;
        Some(Held { _release: tx })
    }

    fn hold(release: &Receiver<()>) -> zbus::Result<()> {
        let bus = Connection::session()?;
        let reply = bus.call_method(
            Some(NAME),
            PATH,
            Some(NAME),
            "Inhibit",
            &(crate::APP_NAME, crate::APP_NAME),
        )?;
        let cookie: u32 = reply.body().deserialize()?;
        // Until the guard goes.
        let _ = release.recv();
        bus.call_method(Some(NAME), PATH, Some(NAME), "UnInhibit", &cookie)?;
        Ok(())
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "windows",
    target_os = "linux"
)))]
mod unsupported {
    pub struct Held(());

    pub fn acquire() -> Option<Held> {
        None
    }
}
