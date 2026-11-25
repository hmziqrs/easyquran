//! Remote push (PARITY M2, M1 push part): the FCM token, mirroring the
//! web's `lib/firebase/messaging.ts` and `push-privacy.ts`.
//!
//! - Signed in, with notifications granted (the reminders screen asks for
//!   the permission; nothing here prompts), `services::push` gives the FCM
//!   token, asked again each time the app comes to the front (the web does
//!   on `visibilitychange`) and when the service changes it. The token goes
//!   to device registration (`AuthStore::register_device`).
//! - Sign-out deletes the token on the server first (devices.md "Sign-out
//!   on a shared phone"): `AuthStore::sign_out` falls back to
//!   [`current_token`], then [`forget`] drops it here and at FCM. The
//!   permission taken away deletes it too (web `syncPermission`).
//! - A push in front shows only the fixed public text (`publicPushPayload`),
//!   and a press opens the account page (`service-worker.ts`).
//! - Desktop has no push (devices.md "Push matrix"): no token, local
//!   notifications only. So has a build without Firebase: every call here
//!   does nothing.

use std::cell::RefCell;

use nsh_core::{
    notifications::NotificationPermission,
    platform::Lifecycle,
    prelude::{effect, lifecycle, notification_permission, spawn, untracked, use_context},
};

use crate::{
    services::push::{self, Event},
    shell::{nav, routes::Route},
    stores::auth::AuthStore,
};

#[derive(Default)]
struct State {
    /// The signed-in account's store, from the sign-in to [`forget`].
    auth: Option<AuthStore>,
    token: Option<String>,
    /// Counts sign-ins and sign-outs: a token asked for before one is not
    /// the one after it's.
    generation: u64,
    /// The generation a request runs for.
    asking: Option<u64>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(f)
}

/// Whether this build has remote push at all: its platform, and Firebase
/// with its configuration.
pub fn supported() -> bool {
    push::configured()
}

/// The FCM token this install registered, if any.
pub fn current_token() -> Option<String> {
    state(|s| s.token.clone())
}

/// Forgets the token after sign-out deleted it on the server.
pub fn forget() {
    let had = state(|s| {
        s.auth = None;
        s.generation += 1;
        s.token.take().is_some()
    });
    push::delete_token();
    if had {
        crate::features::reminders::push_token_changed();
    }
}

/// After a sign-in (or a restored session): fetches the token when push is
/// granted and registers it. Never prompts; failures leave push off.
pub fn on_signed_in() {
    if !supported() {
        return;
    }
    let Some(auth) = use_context::<AuthStore>() else {
        return;
    };
    state(|s| {
        s.auth = Some(auth);
        s.generation += 1;
    });
    // Not from here: the caller may be a sign-in page about to go, and its
    // tasks with it. [`listen`] lives with the app.
    push::emit(Event::Refresh);
}

/// Follows push for the life of the app. Once, from its root.
pub fn listen() {
    let mut events = push::events();
    spawn(async move {
        while let Ok(event) = events.recv().await {
            handle(event);
        }
    });
    if !supported() {
        return;
    }
    effect(move || {
        if lifecycle() == Lifecycle::Active {
            untracked(refresh);
        }
    });
}

fn handle(event: Event) {
    match event {
        Event::Refresh => refresh(),
        Event::Token(token) => adopt(token),
        Event::Message => hint(),
        Event::Press => nav::open(Route::Account),
    }
}

/// Shows the fixed text for a push that arrived in front; only to a
/// subscribed account, as the web does.
fn hint() {
    if !state(|s| s.auth.is_some() && s.token.is_some()) {
        return;
    }
    spawn(async {
        let _ = crate::services::notifications::show_push_hint().await;
    });
}

/// Asks for the token, if an account is signed in and the user allows
/// notifications. One at a time for each sign-in.
fn refresh() {
    let generation = state(|s| {
        let asked = s.asking == Some(s.generation);
        if asked || s.auth.is_none() {
            return None;
        }
        s.asking = Some(s.generation);
        Some(s.generation)
    });
    let Some(generation) = generation else {
        return;
    };
    spawn(async move {
        match notification_permission().await {
            NotificationPermission::Granted => {
                let token = push::request_token().await;
                let same = state(|s| s.generation == generation);
                if let Some(token) = token.filter(|_| same) {
                    adopt(token);
                }
            }
            NotificationPermission::Denied => withdraw(generation).await,
            _ => {}
        }
        state(|s| {
            if s.asking == Some(generation) {
                s.asking = None;
            }
        });
    });
}

/// Registers `token` when it is a new one of the signed-in account.
fn adopt(token: String) {
    if !push::is_fcm_token(&token) {
        return;
    }
    let auth = state(|s| {
        if s.token.as_deref() == Some(token.as_str()) {
            return None;
        }
        let auth = s.auth?;
        s.token = Some(token.clone());
        Some(auth)
    });
    let Some(auth) = auth else {
        return;
    };
    auth.register_device(Some(token));
    crate::features::reminders::push_token_changed();
}

/// The user took the permission away: the server must stop sending here.
async fn withdraw(generation: u64) {
    let (auth, token) = state(|s| {
        let here = s.generation == generation;
        (s.auth.filter(|_| here), s.token.take_if(|_| here))
    });
    let (Some(auth), Some(token)) = (auth, token) else {
        return;
    };
    crate::features::reminders::push_token_changed();
    if let Some(machine) = auth.machine()
        && let Some(user) = machine.user()
    {
        let _ = machine.client().device_token_delete(user.id, &token).await;
    }
    push::delete_token();
}
