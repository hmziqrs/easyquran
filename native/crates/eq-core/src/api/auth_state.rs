//! The auth state machine (web `auth-state.svelte.ts`, `flows.svelte.ts`,
//! `purge-hook.ts`, `account-purge.ts`): signed out, pending TOTP,
//! unverified, signed in, banned; the transitions other stores hook into;
//! and the account purge rules.
//!
//! Plain Rust over [`ApiClient`]: the app's auth store mirrors
//! [`AuthPhase`] into a signal through [`AuthMachine::subscribe`].

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use super::{
    ApiClient, ApiError, BanNotice, Secret, SessionProbe,
    types::{SignInOutcome, UserProfile},
};
use crate::kv::{KeyValue, SecretStore};

/// Where the account stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthPhase {
    /// Not known yet (boot, before the first probe).
    Unknown,
    /// No session. `session_expired`: it was lost mid-use (expiry, remote
    /// sign-out, revoke), so the "sign in to resume sync" notice shows.
    SignedOut { session_expired: bool },
    /// Password (or provider, passkey) accepted; a TOTP code comes next.
    PendingTotp,
    /// Signed in, email not verified: verified-only features wait.
    Unverified(UserProfile),
    /// Signed in.
    SignedIn(UserProfile),
    /// The account is banned.
    Banned(BanNotice),
}

impl AuthPhase {
    /// The signed-in account, verified or not.
    pub fn user(&self) -> Option<&UserProfile> {
        match self {
            AuthPhase::SignedIn(u) | AuthPhase::Unverified(u) => Some(u),
            _ => None,
        }
    }

    fn for_user(user: UserProfile) -> Self {
        if user.is_verified {
            return AuthPhase::SignedIn(user);
        }
        AuthPhase::Unverified(user)
    }
}

/// An auth edge (web `AuthTransitionKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Login,
    Logout,
    OAuth,
    Passkey,
    SessionRotated,
    TwoFaVerify,
    TwoFaDisable,
    CurrentSessionTerminated,
}

impl TransitionKind {
    /// Edges that change whose data is on screen: account stores suspend
    /// or reload (`ACCOUNT_TRANSITION_KINDS`).
    pub fn changes_account(self) -> bool {
        matches!(
            self,
            TransitionKind::Login
                | TransitionKind::OAuth
                | TransitionKind::Passkey
                | TransitionKind::Logout
                | TransitionKind::CurrentSessionTerminated
        )
    }

    /// Sign-in edges: device registration follows.
    pub fn is_sign_in(self) -> bool {
        matches!(
            self,
            TransitionKind::Login | TransitionKind::OAuth | TransitionKind::Passkey
        )
    }
}

/// A transition as hooks see it. `user_id`: the account it concerns (the
/// new one on sign-in, the old one on sign-out and session loss).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub kind: TransitionKind,
    pub user_id: Option<i32>,
}

/// Where a sign-in step left the flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInStep {
    /// Signed in.
    Done(UserProfile),
    /// Enter the TOTP code next ([`AuthMachine::submit_totp`]).
    TotpRequired,
}

/// Device-local records of one account (`ACCOUNT_KEY_PREFIXES`): each is
/// `<prefix><user id>` (or under `<prefix><user id>.`). Device-only data
/// (reader position, settings, Quran caches) is not listed and stays.
pub const ACCOUNT_KEY_PREFIXES: &[&str] = &[
    "easyquran.preferences.",
    "easyquran.goals.",
    "easyquran.notes.",
    "easyquran.sessions.",
    "easyquran.bookmarks.user.",
    "easyquran.bookmarks.last-folder.",
    "easyquran.sync.clock.",
    "easyquran.device.registered.",
    "easyquran.device.id.",
    "easyquran.analytics.queue.",
];

/// Account deletion (F7): removes this install's records of `user_id`
/// (the account-keyed values and its device key). Sign-out never calls
/// this: it only hides the account view; dirty rows wait.
pub fn purge_account_local_data(kv: &dyn KeyValue, secrets: &dyn SecretStore, user_id: i32) {
    crate::device::register::delete_device_key(secrets, user_id);
    let keys = kv.keys();
    for prefix in ACCOUNT_KEY_PREFIXES {
        let exact = format!("{prefix}{user_id}");
        let nested = format!("{exact}.");
        for key in keys
            .iter()
            .filter(|k| **k == exact || k.starts_with(&nested))
        {
            kv.remove(key);
        }
    }
}

/// Key of the last signed-in profile, so a signed-in install opens signed
/// in while offline (PARITY I1: "signed-in state kept"). Not a secret.
pub const CACHED_USER_KEY: &str = "easyquran.auth.user";

type Hook = Rc<dyn Fn(Transition)>;
type Listener = Rc<dyn Fn(&AuthPhase)>;

/// The auth state machine.
pub struct AuthMachine {
    client: Rc<ApiClient>,
    kv: Rc<dyn KeyValue>,
    phase: RefCell<AuthPhase>,
    totp: RefCell<Option<Secret>>,
    hooks: RefCell<Vec<Hook>>,
    listeners: RefCell<Vec<Listener>>,
}

impl std::fmt::Debug for AuthMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthMachine")
            .field("phase", &*self.phase.borrow())
            .finish_non_exhaustive()
    }
}

impl AuthMachine {
    /// A machine over `client`. It listens for lost sessions on the client.
    pub fn new(client: Rc<ApiClient>, kv: Rc<dyn KeyValue>) -> Rc<Self> {
        let machine = Rc::new(Self {
            client: client.clone(),
            kv,
            phase: RefCell::new(AuthPhase::Unknown),
            totp: RefCell::new(None),
            hooks: RefCell::new(Vec::new()),
            listeners: RefCell::new(Vec::new()),
        });
        let weak: Weak<Self> = Rc::downgrade(&machine);
        client.on_session_lost(move || {
            if let Some(m) = weak.upgrade() {
                m.terminate(true);
            }
        });
        machine
    }

    /// The client.
    pub fn client(&self) -> &Rc<ApiClient> {
        &self.client
    }

    /// The current phase.
    pub fn phase(&self) -> AuthPhase {
        self.phase.borrow().clone()
    }

    /// The signed-in account.
    pub fn user(&self) -> Option<UserProfile> {
        self.phase.borrow().user().cloned()
    }

    /// Calls `listener` after every phase change.
    pub fn subscribe(&self, listener: impl Fn(&AuthPhase) + 'static) {
        self.listeners.borrow_mut().push(Rc::new(listener));
    }

    /// Registers a transition hook (purge on sign-out, reload on sign-in,
    /// device registration). Hooks run before the new phase is published.
    pub fn on_transition(&self, hook: impl Fn(Transition) + 'static) {
        self.hooks.borrow_mut().push(Rc::new(hook));
    }

    fn set_phase(&self, phase: AuthPhase) {
        if *self.phase.borrow() == phase {
            return;
        }
        match phase.user() {
            Some(user) => self.cache_user(user),
            None => self.kv.remove(CACHED_USER_KEY),
        }
        *self.phase.borrow_mut() = phase.clone();
        let listeners: Vec<Listener> = self.listeners.borrow().clone();
        for listener in listeners {
            listener(&phase);
        }
    }

    fn cache_user(&self, user: &UserProfile) {
        let value = serde_json::json!({
            "id": user.id, "name": user.name, "email": user.email,
            "avatar_id": user.avatar_id, "is_verified": user.is_verified,
            "is_minor": user.is_minor, "role": user.role,
            "two_fa_enabled": user.two_fa_enabled, "oauth_provider": user.oauth_provider,
            "age_declared_at": user.age_declared.then_some("declared"),
        });
        let _ = self.kv.set(CACHED_USER_KEY, &value.to_string());
    }

    fn transition(&self, kind: TransitionKind, user_id: Option<i32>) {
        let hooks: Vec<Hook> = self.hooks.borrow().clone();
        for hook in hooks {
            hook(Transition { kind, user_id });
        }
    }

    /// Boot: with no stored session, signed out without a probe (the web's
    /// session-hint gate); with one, the cached profile shows at once, then
    /// a probe settles it. Offline keeps the cached state.
    pub async fn hydrate(&self) -> AuthPhase {
        if !self.client.has_session() {
            self.kv.remove(CACHED_USER_KEY);
            self.set_phase(AuthPhase::SignedOut {
                session_expired: false,
            });
            return self.phase();
        }
        if let Some(user) = self
            .kv
            .get(CACHED_USER_KEY)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .and_then(|v| UserProfile::decode(&v))
        {
            self.set_phase(AuthPhase::for_user(user));
        }
        if let Ok(Some(token)) = self.client.login_totp_pending().await {
            *self.totp.borrow_mut() = Some(token);
            self.set_phase(AuthPhase::PendingTotp);
            return self.phase();
        }
        self.probe().await;
        self.phase()
    }

    /// `GET /user/v1/get` and apply the result (`#applyProbe`). Skipped
    /// while a TOTP step is pending: the anonymous session holds it.
    pub async fn probe(&self) -> SessionProbe {
        if matches!(*self.phase.borrow(), AuthPhase::PendingTotp) {
            return SessionProbe::Unknown;
        }
        match self.client.get_user().await {
            Ok(user) => {
                self.adopt(user.clone(), None);
                SessionProbe::Authenticated(user)
            }
            Err(ApiError::Http(e)) if e.ban_notice().is_some() => {
                self.banned(e.ban_notice().unwrap_or_default());
                SessionProbe::Anonymous
            }
            Err(ApiError::Http(e)) if matches!(e.status, 401 | 403) => {
                // A 401 already ran `terminate` through the client listener.
                let signed_in = self.phase.borrow().user().is_some();
                let signed_out = matches!(*self.phase.borrow(), AuthPhase::SignedOut { .. });
                if signed_in {
                    self.terminate(true);
                }
                if !signed_in && !signed_out {
                    self.set_phase(AuthPhase::SignedOut {
                        session_expired: false,
                    });
                }
                SessionProbe::Anonymous
            }
            Err(_) => {
                if matches!(*self.phase.borrow(), AuthPhase::Unknown) && !self.client.has_session()
                {
                    self.set_phase(AuthPhase::SignedOut {
                        session_expired: false,
                    });
                }
                SessionProbe::Unknown
            }
        }
    }

    /// `refresh_user`: probe again (after verify email, profile edits).
    pub async fn refresh_user(&self) -> SessionProbe {
        self.probe().await
    }

    /// Adopts `user`. `kind`: the sign-in edge that produced it; a probe
    /// that finds another account than the one shown runs `Login` too.
    fn adopt(&self, user: UserProfile, kind: Option<TransitionKind>) {
        let previous = self.phase.borrow().user().map(|u| u.id);
        let edge = kind.or_else(|| {
            previous
                .filter(|id| *id != user.id)
                .map(|_| TransitionKind::Login)
        });
        *self.totp.borrow_mut() = None;
        if let Some(kind) = edge {
            self.transition(kind, Some(user.id));
        }
        self.set_phase(AuthPhase::for_user(user));
    }

    /// Replaces the shown profile after an account edit (2FA, profile,
    /// age declaration) without a sign-in edge.
    pub fn set_user(&self, user: UserProfile) {
        self.set_phase(AuthPhase::for_user(user));
    }

    fn banned(&self, notice: BanNotice) {
        let previous = self.phase.borrow().user().map(|u| u.id);
        if previous.is_some() {
            self.client.clear_session();
            self.transition(TransitionKind::CurrentSessionTerminated, previous);
        }
        *self.totp.borrow_mut() = None;
        self.set_phase(AuthPhase::Banned(notice));
    }

    /// Session lost mid-use (401, revoke, expiry): signed out with the
    /// notice. Dirty sync rows are untouched and wait for the next sign-in.
    fn terminate(&self, notice: bool) {
        let previous = self.phase.borrow().user().map(|u| u.id);
        let Some(user_id) = previous else {
            return;
        };
        self.client.clear_csrf();
        *self.totp.borrow_mut() = None;
        self.transition(TransitionKind::CurrentSessionTerminated, Some(user_id));
        self.set_phase(AuthPhase::SignedOut {
            session_expired: notice,
        });
    }

    /// Applies a sign-in reply from any path: TOTP next, or signed in.
    /// A banned or already-signed-in reply is handled here too.
    pub async fn complete_sign_in(
        &self,
        result: Result<SignInOutcome, ApiError>,
        kind: TransitionKind,
    ) -> Result<SignInStep, ApiError> {
        match result {
            Ok(SignInOutcome::TotpRequired(token)) => {
                *self.totp.borrow_mut() = Some(token);
                self.set_phase(AuthPhase::PendingTotp);
                Ok(SignInStep::TotpRequired)
            }
            Ok(SignInOutcome::SignedIn(user)) => {
                self.adopt(user.clone(), Some(kind));
                Ok(SignInStep::Done(user))
            }
            Err(ApiError::Http(e)) if e.is_already_authenticated() => {
                // A stale form after signing in elsewhere: adopt that session.
                match self.client.get_user().await {
                    Ok(user) => {
                        self.adopt(user.clone(), Some(TransitionKind::Login));
                        Ok(SignInStep::Done(user))
                    }
                    Err(_) => Err(ApiError::Http(e)),
                }
            }
            Err(ApiError::Http(e)) if e.ban_notice().is_some() => {
                self.banned(e.ban_notice().unwrap_or_default());
                Err(ApiError::Http(e))
            }
            Err(e) => Err(e),
        }
    }

    /// Email and password.
    pub async fn log_in(&self, email: &str, password: &Secret) -> Result<SignInStep, ApiError> {
        let result = self.client.log_in(email.trim(), password).await;
        self.complete_sign_in(result, TransitionKind::Login).await
    }

    /// The TOTP code of a pending sign-in. With no pending token the flow
    /// starts over (signed out).
    pub async fn submit_totp(&self, code: &Secret) -> Result<SignInStep, ApiError> {
        let token = self.totp.borrow().clone();
        let Some(token) = token else {
            self.set_phase(AuthPhase::SignedOut {
                session_expired: false,
            });
            return Err(ApiError::Decode("no pending sign-in".into()));
        };
        let result = self.client.login_totp(&token, code).await;
        self.complete_sign_in(result, TransitionKind::Login).await
    }

    /// Abandons a pending TOTP step.
    pub fn cancel_totp(&self) {
        *self.totp.borrow_mut() = None;
        if matches!(*self.phase.borrow(), AuthPhase::PendingTotp) {
            self.set_phase(AuthPhase::SignedOut {
                session_expired: false,
            });
        }
    }

    /// Signs out: the push token is deleted first (shared phones), then
    /// `log_out`; the stored session value and CSRF token are cleared,
    /// hooks run `Logout`, and the phase is signed out. Account data stays
    /// on the device (quarantined by the stores), never destroyed.
    pub async fn log_out(&self, push_token: Option<&str>) -> Result<(), ApiError> {
        let previous = self.phase.borrow().user().map(|u| u.id);
        if let (Some(user_id), Some(token)) = (previous, push_token) {
            let _ = self.client.device_token_delete(user_id, token).await;
            crate::device::register::forget_registered_token(self.kv.as_ref(), user_id);
        }
        self.client.log_out().await?;
        *self.totp.borrow_mut() = None;
        self.transition(TransitionKind::Logout, previous);
        self.set_phase(AuthPhase::SignedOut {
            session_expired: false,
        });
        Ok(())
    }

    /// After `user/v1/delete` succeeded: wipe this install's records of the
    /// account and sign out.
    pub fn account_deleted(&self, user_id: i32) {
        self.client.clear_session();
        purge_account_local_data(self.kv.as_ref(), self.client.secrets().as_ref(), user_id);
        self.transition(TransitionKind::Logout, Some(user_id));
        self.set_phase(AuthPhase::SignedOut {
            session_expired: false,
        });
    }
}
