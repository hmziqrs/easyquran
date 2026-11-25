//! FirebaseMessaging on iOS, through the Objective-C runtime: nothing links
//! against the SDK, so this compiles without it and finds its classes only
//! when the owner's build carries them (linked whole, as nothing names them
//! at link time).
//!
//! Firebase is configured on first use, from build-time values
//! (`EQ_FIREBASE_*`: NSH puts no files at the bundle's root, where
//! `GoogleService-Info.plist` would be read), so an app that is signed out or
//! was refused notifications starts no Firebase code. Its app delegate proxy
//! and auto-init are off (`Cargo.toml`, `[package.metadata.nsh]`): the APNs
//! token comes from `nsh-extra` and is set here, and the SDK makes an FCM
//! token only when asked.

use std::{
    ffi::{CStr, c_void},
    ptr,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use block2::RcBlock;
use futures_channel::oneshot;
use nsh_extra::push::DeviceToken;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::{NSError, NSString};

use super::{DeleteGuard, Event};

/// `FIRMessagingAPNSTokenTypeUnknown`: the SDK reads the environment from
/// the provisioning profile.
const APNS_TOKEN_TYPE_UNKNOWN: isize = 0;

/// `FIRMessagingRegistrationTokenRefreshedNotification`; its object is the
/// token, nil once deleted.
const TOKEN_REFRESHED: &str = "com.firebase.messaging.notif.fcm-token-refreshed";

/// Firebase is configured ([`messaging`]).
static STARTED: AtomicBool = AtomicBool::new(false);

/// The Firebase iOS app's values (`GoogleService-Info.plist` of the Firebase
/// console), at build time.
struct Config {
    /// `GOOGLE_APP_ID`.
    app_id: &'static str,
    /// `GCM_SENDER_ID`.
    sender_id: &'static str,
    /// `API_KEY`.
    api_key: &'static str,
    /// `PROJECT_ID`.
    project_id: &'static str,
}

fn config() -> Option<Config> {
    let set = |value: Option<&'static str>| value.filter(|v| !v.is_empty());
    Some(Config {
        app_id: set(option_env!("EQ_FIREBASE_IOS_APP_ID"))?,
        sender_id: set(option_env!("EQ_FIREBASE_SENDER_ID"))?,
        api_key: set(option_env!("EQ_FIREBASE_IOS_API_KEY"))?,
        project_id: set(option_env!("EQ_FIREBASE_PROJECT_ID"))?,
    })
}

fn class(name: &CStr) -> Option<&'static AnyClass> {
    AnyClass::get(name)
}

/// The SDK is in the app, and the build has its values.
pub(super) fn configured() -> bool {
    config().is_some()
        && [c"FIRApp", c"FIROptions", c"FIRMessaging"]
            .into_iter()
            .all(|name| class(name).is_some())
}

pub(super) fn install() {}

/// `[FIRMessaging messaging]`, Firebase configured first. On the main thread.
fn messaging() -> Option<Retained<AnyObject>> {
    start()?;
    // SAFETY: the SDK's documented class method, with the app configured.
    unsafe { msg_send![class(c"FIRMessaging")?, messaging] }
}

/// Configures Firebase, once. On the main thread.
fn start() -> Option<()> {
    if STARTED.load(Ordering::Acquire) {
        return Some(());
    }
    let config = config()?;
    let (app, options) = (class(c"FIRApp")?, class(c"FIROptions")?);
    // SAFETY: the SDK's documented initializer, properties and class method,
    // with strings; the result of `alloc` goes to its initializer alone.
    unsafe {
        let options: Retained<AnyObject> = {
            let allocated: *mut AnyObject = msg_send![options, alloc];
            let made: *mut AnyObject = msg_send![
                allocated,
                initWithGoogleAppID: &*NSString::from_str(config.app_id),
                GCMSenderID: &*NSString::from_str(config.sender_id)
            ];
            Retained::from_raw(made)?
        };
        let () = msg_send![&*options, setAPIKey: &*NSString::from_str(config.api_key)];
        let () = msg_send![&*options, setProjectID: &*NSString::from_str(config.project_id)];
        let () = msg_send![app, configureWithOptions: &*options];
    }
    STARTED.store(true, Ordering::Release);
    hear_refreshes();
    Some(())
}

/// Reports the tokens the SDK changes on its own.
fn hear_refreshes() {
    let Some(center) = class(c"NSNotificationCenter") else {
        return;
    };
    let heard = RcBlock::new(|notification: *mut AnyObject| {
        // SAFETY: the notification the center passes; its object is the
        // token (a string) or nil.
        let token: Option<Retained<NSString>> = unsafe { msg_send![notification, object] };
        if let Some(token) = token {
            super::emit(Event::Token(token.to_string()));
        }
    });
    // SAFETY: the documented way to observe; the observer stays for the
    // process's life, so its handle is not kept.
    unsafe {
        let center: Retained<AnyObject> = msg_send![center, defaultCenter];
        let _: Retained<AnyObject> = msg_send![
            &*center,
            addObserverForName: &*NSString::from_str(TOKEN_REFRESHED),
            object: ptr::null::<AnyObject>(),
            queue: ptr::null::<AnyObject>(),
            usingBlock: &*heard
        ];
    }
}

/// Gives the SDK the APNs token. On the main thread.
fn set_apns_token(messaging: &AnyObject, token: &DeviceToken) {
    let Some(data) = class(c"NSData") else {
        return;
    };
    let bytes = token.as_bytes();
    // SAFETY: `NSData` copies the bytes; the SDK's documented setter.
    unsafe {
        let data: Retained<AnyObject> = msg_send![
            data,
            dataWithBytes: bytes.as_ptr().cast::<c_void>(),
            length: bytes.len()
        ];
        let () = msg_send![messaging, setAPNSToken: &*data, type: APNS_TOKEN_TYPE_UNKNOWN];
    }
}

/// The system (re)registered the device: an SDK that runs hears the APNs
/// token. One that does not gets it in [`fcm_token`].
pub(super) fn registered(token: &DeviceToken) {
    if !STARTED.load(Ordering::Acquire) {
        return;
    }
    let token = token.clone();
    on_main(move || {
        if let Some(messaging) = messaging() {
            set_apns_token(&messaging, &token);
        }
    });
}

/// The FCM token for the APNs token `nsh-extra` registered.
pub(super) fn fcm_token() -> oneshot::Receiver<Option<String>> {
    let (answer, token) = oneshot::channel();
    on_main(move || {
        let (Some(apns), Some(messaging)) = (nsh_extra::push::token(), messaging()) else {
            return;
        };
        set_apns_token(&messaging, &apns);
        let answer = Mutex::new(Some(answer));
        let done = RcBlock::new(move |token: *mut NSString, _: *mut NSError| {
            // SAFETY: the string the SDK passes, or nil.
            let token = unsafe { token.as_ref() }.map(ToString::to_string);
            if let Some(answer) = answer.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = answer.send(token);
            }
        });
        // SAFETY: the SDK's documented method and its block's signature.
        let () = unsafe { msg_send![&*messaging, tokenWithCompletion: &*done] };
    });
    token
}

pub(super) fn delete_token(guard: DeleteGuard) {
    on_main(move || {
        let Some(messaging) = messaging() else {
            return;
        };
        let guard = Mutex::new(Some(guard));
        let done = RcBlock::new(move |_: *mut NSError| {
            drop(guard.lock().unwrap_or_else(|e| e.into_inner()).take());
        });
        // SAFETY: the SDK's documented method and its block's signature.
        let () = unsafe { msg_send![&*messaging, deleteTokenWithCompletion: &*done] };
    });
}

/// Runs `f` on the main thread, later (a completion block's thread and the
/// engine's are not it).
fn on_main(f: impl FnOnce() + Send + 'static) {
    let Some(queue) = class(c"NSOperationQueue") else {
        return;
    };
    let f = Mutex::new(Some(f));
    let block = RcBlock::new(move || {
        let f = f.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(f) = f {
            f();
        }
    });
    // SAFETY: documented methods; the queue copies the block, whose content
    // is `Send`.
    unsafe {
        let queue: Retained<AnyObject> = msg_send![queue, mainQueue];
        let () = msg_send![&*queue, addOperationWithBlock: &*block];
    }
}
