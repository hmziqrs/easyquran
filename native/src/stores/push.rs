//! Remote push (PARITY M2, M1 push part): the FCM token, mirroring the
//! web's `lib/firebase/messaging.ts` and `push-privacy.ts`.
//!
//! - iOS and Android ask `nsh-firebase-messaging` for the FCM token once the
//!   user is signed in and notifications are granted (the reminders screen
//!   asks for the permission; nothing here prompts). The token goes to
//!   device registration (`AuthStore::register_device`).
//! - Sign-out deletes the token on the server first (devices.md "Sign-out
//!   on a shared phone"): `AuthStore::sign_out` falls back to
//!   [`current_token`], then [`forget`] drops it here.
//! - A foreground FCM message shows only the fixed public text
//!   (`publicPushPayload`): no account or notification details are read
//!   from the payload.
//! - Desktop has no push (devices.md "Push matrix"): no token, local
//!   notifications only.
//!
//! The messaging facade rides the Firebase native tier (`firebase.*` host
//! handlers registered by an embedding host). An NSH app built by `nsh
//! build` has no such host today, so the facade runs on the pure tier,
//! every call fails clearly and no token is ever sent: push stays off
//! until a host channel exists (`native/README.md`, "Firebase").

use std::cell::RefCell;
use std::sync::Arc;

use nsh_core::prelude::*;
use nsh_firebase_core::host::{FirebaseHost, PureHost};
use nsh_firebase_messaging::{Event, Messaging, Permission};

thread_local! {
    static TOKEN: RefCell<Option<String>> = const { RefCell::new(None) };
    static MESSAGING: RefCell<Option<Messaging>> = const { RefCell::new(None) };
}

/// Whether this platform has remote push at all.
pub fn supported() -> bool {
    cfg!(any(target_os = "ios", target_os = "android"))
}

/// The host the messaging facade calls. No NSH API hands a Rust app the
/// embedding host's channel, so this is the pure tier (every call fails
/// with `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`).
fn host() -> Arc<dyn FirebaseHost> {
    Arc::new(PureHost)
}

fn messaging() -> Option<Messaging> {
    if !supported() {
        return None;
    }
    MESSAGING.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let m = Messaging::new(host());
            // A foreground message: the fixed public text only.
            let _ = m.on(Event::Message, |_payload| {
                spawn(async {
                    let _ = crate::services::notifications::show_push_hint().await;
                });
            });
            // A rotated token: register it again.
            let _ = m.on(Event::Token, |payload| {
                let token = payload
                    .get("token")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                spawn(async move { adopt(token) });
            });
            *slot = Some(m);
        }
        slot.clone()
    })
}

/// The FCM token this install registered, if any.
pub fn current_token() -> Option<String> {
    TOKEN.with(|t| t.borrow().clone())
}

/// Forgets the token after sign-out deleted it on the server.
pub fn forget() {
    TOKEN.with(|t| *t.borrow_mut() = None);
}

fn adopt(token: Option<String>) {
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return;
    };
    if current_token().as_deref() == Some(token.as_str()) {
        return;
    }
    TOKEN.with(|t| *t.borrow_mut() = Some(token.clone()));
    crate::app::auth_store().register_device(Some(token));
}

/// After a sign-in (or a restored session): fetch the token when push is
/// granted and register it. Never prompts; failures leave push off.
pub fn on_signed_in() {
    let Some(m) = messaging() else {
        return;
    };
    spawn(async move {
        if !matches!(m.permission().await, Ok(Permission::Granted)) {
            return;
        }
        if let Ok(token) = m.token().await {
            adopt(Some(token));
        }
    });
}
