//! The presses of the notifications NSH did not show, on macOS.
//!
//! Since 0.7 NSH, which owns the notification center's delegate, hands them
//! to the process's foreign handler, never to the app's press listeners. The
//! scheduled reminder is one. Without a handler it still shows and brings the
//! app forward, but the press goes nowhere. There is one handler and a second
//! replaces the first: on iOS nsh-extra's `push` has it.

use std::sync::{Mutex, Once, PoisonError};

use block2::RcBlock;
use futures_channel::oneshot;
use nsh_platform_apple::notifications::{
    ForeignNotification, ForeignNotificationHandler, set_foreign_handler,
};
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};

use super::apple;

/// The presses the handler heard (on the main thread), for
/// [`next_press`] (on the engine's).
struct Presses {
    waiting: Option<oneshot::Sender<()>>,
    queued: usize,
}

static PRESSES: Mutex<Presses> = Mutex::new(Presses {
    waiting: None,
    queued: 0,
});

struct Handler;

impl ForeignNotificationHandler for Handler {
    fn pressed(&self, _notification: &ForeignNotification) {
        let mut presses = PRESSES.lock().unwrap_or_else(PoisonError::into_inner);
        let heard = presses
            .waiting
            .take()
            .is_some_and(|waiting| waiting.send(()).is_ok());
        if !heard {
            presses.queued += 1;
        }
    }
}

/// Sets the process's foreign notification handler, once, on the main
/// thread. The press that started the app waits in NSH until then.
pub(super) fn install() {
    static INSTALLED: Once = Once::new();
    if apple::center().is_none() {
        return;
    }
    INSTALLED.call_once(|| {
        let Some(class) = AnyClass::get(c"NSOperationQueue") else {
            return;
        };
        let set = RcBlock::new(|| set_foreign_handler(Handler));
        // SAFETY: documented: the main queue copies the block and runs it
        // on the main thread.
        unsafe {
            let queue: Retained<AnyObject> = msg_send![class, mainQueue];
            let () = msg_send![&*queue, addOperationWithBlock: &*set];
        }
    });
}

/// The next press of a notification NSH did not show; `false` when another
/// wait replaced this one.
pub(super) async fn next_press() -> bool {
    let receiver = {
        let mut presses = PRESSES.lock().unwrap_or_else(PoisonError::into_inner);
        if presses.queued > 0 {
            presses.queued -= 1;
            return true;
        }
        let (sender, receiver) = oneshot::channel();
        presses.waiting = Some(sender);
        receiver
    };
    receiver.await.is_ok()
}
