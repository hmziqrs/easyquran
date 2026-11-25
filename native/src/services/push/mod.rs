//! Remote push (PARITY M2): `nsh-extra`'s `push` joined to Firebase Cloud
//! Messaging, under `stores::push`.
//!
//! The server sends through FCM only (`rust/backend/api/crates/rux-fcm`), so
//! the one token it may hold is an **FCM registration token**. `nsh-extra`
//! gives iOS's *APNs* device token, which looks valid to the server but FCM
//! cannot send to: it goes to the Firebase SDK and nowhere else. The [`binding`]
//! turns it into an FCM token, and [`request_token`] refuses anything shaped
//! like an APNs token.
//!
//! - `binding` is the Firebase SDK's side: `ios` (FirebaseMessaging over the
//!   Objective-C runtime) and `android` (firebase-messaging over JNI, with
//!   `FcmService.java`). Both are behind the Cargo feature `fcm`, off by
//!   default, and neither needs the SDK to compile. Everywhere else, and
//!   where the SDK or its configuration is missing, it is `none`.
//! - [`host`] is the `FirebaseHost` the messaging facade calls: the binding,
//!   or the pure tier that fails every call.
//! - Without a configured binding nothing registers the device: no provider,
//!   no permission asked, no token. [`install`] then only hears presses, as
//!   `nsh-extra` owns iOS's one handler of the notifications NSH did not show
//!   and a press of the scheduled reminder (`services::notifications`) comes
//!   to it too.
//!
//! A push shows only the fixed public text and opens the account page (web
//! `push-privacy.ts`, `service-worker.ts`): the payload is read for one
//! thing, whether a push service sent the notification at all (the same
//! observer hears local reminders), and never leaves this module. No token
//! is logged.

use std::sync::{
    Arc, Mutex, MutexGuard, Once,
    atomic::{AtomicBool, Ordering},
};

use futures_channel::{mpsc, oneshot};
use nsh_core::JsonValue;
use nsh_extra::push::{ForegroundPresentation, PushEvent, RemoteNotification};
use nsh_firebase_core::{
    error::{FirebaseError, HOST_ERROR, UNAVAILABLE},
    host::{FirebaseHost, HostCallFuture, HostChannel, NativeHost, PureHost},
};
use nsh_firebase_messaging::{Messaging, wire};

#[cfg(all(feature = "fcm", target_os = "android"))]
#[path = "android.rs"]
mod binding;
#[cfg(all(feature = "fcm", target_os = "ios", not(target_abi = "macabi")))]
#[path = "ios.rs"]
mod binding;
#[cfg(not(all(
    feature = "fcm",
    any(
        target_os = "android",
        all(target_os = "ios", not(target_abi = "macabi"))
    )
)))]
#[path = "none.rs"]
mod binding;

/// The longest push token the server takes (`device_v1/validator.rs`).
const TOKEN_MAX: usize = 4096;

/// How many events wait for the app's UI (the press that launched the app).
const PENDING_MAX: usize = 16;

/// What the app does something about, on the engine thread.
#[derive(Clone, PartialEq, Eq)]
pub enum Event {
    /// Ask for the token again (a sign-in).
    Refresh,
    /// The push service changed the FCM token.
    Token(String),
    /// A push arrived while the app is in front.
    Message,
    /// The user pressed a notification NSH did not show: a push, or iOS's
    /// scheduled reminder. (NSH reports the presses of its own, the hint's
    /// too: `services::notifications::listen_for_presses`.)
    Press,
}

static INSTALL: Once = Once::new();
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static QUEUE: Mutex<Queue> = Mutex::new(Queue {
    sink: None,
    pending: Vec::new(),
});
static DELETING: Mutex<Deleting> = Mutex::new(Deleting {
    running: 0,
    waiting: Vec::new(),
});

struct Queue {
    sink: Option<mpsc::UnboundedSender<Event>>,
    pending: Vec<Event>,
}

fn lock<T>(mutex: &'static Mutex<T>) -> MutexGuard<'static, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether this build can receive push: its platform, the `fcm` feature, the
/// Firebase SDK and its configuration.
pub fn configured() -> bool {
    CONFIGURED.load(Ordering::Acquire)
}

/// Joins `nsh-extra`'s push to the binding, once. From the `#[native_module]`
/// entry, before the app launches, so the press that launches it is heard.
pub fn install() {
    INSTALL.call_once(|| {
        nsh_extra::push::on_event(observe);
        if !binding::configured() {
            return;
        }
        CONFIGURED.store(true, Ordering::Release);
        binding::install();
        nsh_extra::push::set_foreground_presentation_decider(Some(Arc::new(
            |notification, shown| present(&notification.payload, shown),
        )));
    });
}

/// How a notification shows in front: a push not at all (the app shows the
/// public text instead), any other as it would.
fn present(payload: &JsonValue, shown: ForegroundPresentation) -> ForegroundPresentation {
    if !from_service(payload) {
        return shown;
    }
    shown.banner(false).list(false).sound(false).badge(false)
}

/// Whether a push service sent the notification with `payload`: APNs puts
/// `aps` in it, FCM's Android SDK `google.message_id`. Other notifications
/// NSH did not show (scheduled reminders) have neither.
fn from_service(payload: &JsonValue) -> bool {
    payload.get("aps").is_some() || payload.get("google.message_id").is_some()
}

/// The module's events, on the thread the platform reports from. A
/// background delivery starts no work here: its completion, untouched,
/// tells the system there was nothing new.
fn observe(event: &PushEvent<'_>) {
    match event {
        PushEvent::Token(token) if configured() => binding::registered(token),
        PushEvent::Notification(notification) if arrived(notification) => {
            emit(Event::Message);
        }
        PushEvent::Press(notification) if opens(notification) => emit(Event::Press),
        _ => {}
    }
}

/// Whether a notification that arrived in front is a push. Android reports
/// only those of the binding, which hands over no payload.
fn arrived(notification: &RemoteNotification) -> bool {
    cfg!(target_os = "android") || from_service(&notification.payload)
}

/// Whether a press opens the account page: all of them on iOS, where the app
/// shows no others (a push, the scheduled reminder). On Android any intent
/// with extras counts as a press, so only a push service's does, and the
/// reminder's carries a link.
fn opens(notification: &RemoteNotification) -> bool {
    cfg!(target_os = "ios") || from_service(&notification.payload)
}

/// Hands `event` to the app's UI, or keeps it until there is one.
pub fn emit(event: Event) {
    let mut queue = lock(&QUEUE);
    let sent = queue
        .sink
        .as_ref()
        .is_some_and(|sink| sink.unbounded_send(event.clone()).is_ok());
    if sent {
        return;
    }
    queue.sink = None;
    if queue.pending.len() < PENDING_MAX {
        queue.pending.push(event);
    }
}

/// The events from now on, those that waited first. A later call takes over
/// from an earlier one.
pub fn events() -> mpsc::UnboundedReceiver<Event> {
    let (sink, events) = mpsc::unbounded();
    let mut queue = lock(&QUEUE);
    for event in queue.pending.drain(..) {
        let _ = sink.unbounded_send(event);
    }
    queue.sink = Some(sink);
    events
}

/// The FCM registration token of this install, or `None`: registers with the
/// system (iOS) or the service (Android), then asks the binding. Prompts for
/// nothing; offline it waits. In a task.
pub async fn request_token() -> Option<String> {
    if !configured() {
        return None;
    }
    settled().await;
    nsh_extra::push::register().await.ok()?;
    let token = Messaging::new(host()).token().await.ok()?;
    is_fcm_token(&token).then_some(token)
}

/// Whether `token` may be registered as an FCM token: the characters the
/// server takes, and not an APNs token (32 or more bytes in hexadecimal),
/// which the server's check would let through.
pub fn is_fcm_token(token: &str) -> bool {
    let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b':' | b'_' | b'-');
    let apns = token.len() >= 64 && token.bytes().all(|b| b.is_ascii_hexdigit());
    !token.is_empty() && token.len() <= TOKEN_MAX && token.bytes().all(allowed) && !apns
}

/// Drops the FCM token at the service (web `deleteFcmToken`), after the
/// server deleted it. Best effort; a later [`request_token`] waits for it.
pub fn delete_token() {
    if !configured() {
        return;
    }
    lock(&DELETING).running += 1;
    binding::delete_token(DeleteGuard);
}

struct Deleting {
    running: usize,
    waiting: Vec<oneshot::Sender<()>>,
}

/// A running deletion: it ends when the binding drops this.
struct DeleteGuard;

impl Drop for DeleteGuard {
    fn drop(&mut self) {
        let mut deleting = lock(&DELETING);
        deleting.running -= 1;
        if deleting.running > 0 {
            return;
        }
        for waiter in deleting.waiting.drain(..) {
            let _ = waiter.send(());
        }
    }
}

/// Resolves once no deletion runs.
async fn settled() {
    let wait = {
        let mut deleting = lock(&DELETING);
        if deleting.running == 0 {
            return;
        }
        let (done, wait) = oneshot::channel();
        deleting.waiting.push(done);
        wait
    };
    let _ = wait.await;
}

/// The host the messaging facade calls.
fn host() -> Arc<dyn FirebaseHost> {
    if !configured() {
        return Arc::new(PureHost);
    }
    Arc::new(NativeHost::new(Arc::new(Channel)))
}

/// The binding as the facade's host channel. The token is all the app asks
/// of it: permission, notifications and the badge are NSH's.
struct Channel;

impl HostChannel for Channel {
    fn call(&self, name: String, _args: JsonValue) -> HostCallFuture {
        if name != wire::TOKEN {
            return Box::pin(async {
                Err(FirebaseError::new(
                    UNAVAILABLE,
                    "the binding has no such handler",
                ))
            });
        }
        let answer = binding::fcm_token();
        Box::pin(async move {
            match answer.await {
                Ok(Some(token)) => Ok(serde_json::json!({ "token": token })),
                _ => Err(FirebaseError::new(HOST_ERROR, "FCM gave no token")),
            }
        })
    }

    fn post(&self, _name: String, _args: JsonValue) -> Result<(), FirebaseError> {
        Err(FirebaseError::new(
            UNAVAILABLE,
            "the binding has no such handler",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_apns_token_is_never_an_fcm_token() {
        assert!(is_fcm_token("dGhpcyBpcyBh_-Zm:APA91bHx"));
        assert!(!is_fcm_token(&"0123456789abcdef".repeat(4)));
        assert!(!is_fcm_token(&"0123456789ABCDEF".repeat(10)));
        assert!(!is_fcm_token(""));
        assert!(!is_fcm_token("line\nbreak"));
    }

    #[test]
    fn only_a_push_service_s_notification_is_a_push() {
        assert!(from_service(&serde_json::json!({ "aps": {} })));
        assert!(from_service(
            &serde_json::json!({ "google.message_id": "0:1" })
        ));
        assert!(!from_service(&serde_json::json!({})));
        assert!(!from_service(&JsonValue::Null));
    }
}
