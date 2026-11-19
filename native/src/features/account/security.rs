//! Two-factor setup and disable (`TwoFactorControls.svelte`, the
//! `TwoFactorFlow` of `flows.svelte.ts`, PARITY I3) and the security
//! confirmation (`FreshAuthControls.svelte`, `fresh-auth-client.ts`,
//! PARITY I6). The web writes this copy as English literals, not
//! catalogue messages; they stay literals here (`Msg::Lit`).

use eq_core::{
    api::{
        ApiError, Secret, SignInStep,
        account::is_identity_provider,
        types::{FreshAuthStatus, OAuthProvider, TwoFaSetup},
    },
    i18n::keys,
};
use nsh_core::{element::AnyElement, prelude::*};

use super::{busy_label, needs_fresh_auth, sign_out, start};
use crate::{
    app::auth_store,
    features::auth::{
        form::{
            self, Failure, FormState, Msg, auth_field, copy::VERIFY_EMAIL_NEXT, error_line,
            notice_line, replace_top, six_digit_rule,
        },
        oauth,
    },
    shell::routes::Route,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        collapse::collapsible_section,
        field::{field, password_field, text_field},
        layout::card,
        text::{code, label, muted},
        tokens,
    },
};

mod copy {
    pub const FRESH_AUTH_REQUIRED: &str =
        "Confirm your identity in Security confirmation, then try again.";
    pub const CONFIRMATION_EXPIRED: &str = "Confirmation expired. Confirm your identity in Security confirmation, then start 2FA setup again.";

    pub const VERIFY_HEADING: &str = "Enter your authenticator code";
    pub const VERIFY_BODY: &str =
        "Add this secret to your authenticator app, then enter the 6-digit code it generates.";
    pub const SECRET_LABEL: &str = "Secret (keep private)";
    pub const BACKUP_WARNING: &str =
        "Save these somewhere safe — each code works once and they cannot be shown again.";
    pub const SHOW_BACKUP: &str = "Show backup codes";
    pub const CODE_LABEL: &str = "Authentication code";
    pub const VERIFYING: &str = "Verifying…";
    pub const CONFIRM_ENABLE: &str = "Confirm and enable";
    pub const SETUP_HEADING: &str = "Two-factor authentication";
    pub const SETUP_BODY: &str = "Add a second step at sign-in using an authenticator app.";
    pub const STARTING: &str = "Starting…";
    pub const SET_UP: &str = "Set up 2FA";
    pub const ON_HEADING: &str = "Two-factor authentication is on";
    pub const ON_BODY: &str = "Confirm your identity in Security confirmation first. If you used your authenticator there, wait for its next code before entering it here to disable two-factor authentication.";
    pub const DISABLE_HINT: &str = "Your 6-digit code or a backup code.";
    pub const DISABLING: &str = "Disabling…";
    pub const DISABLE: &str = "Disable 2FA";
    pub const OFF_HEADING: &str = "Two-factor authentication is off";
    pub const OFF_BODY: &str = "Two-factor authentication has been disabled.";

    pub const FRESH_HEADING: &str = "Security confirmation";
    pub const FRESH_BODY: &str = "Confirm your identity before changing sign-in methods or security settings. Each confirmation permits one change within five minutes.";
    pub const FRESH_READY: &str = "Ready for one security change.";
    pub const CURRENT_PASSWORD: &str = "Current password";
    pub const AUTH_OR_BACKUP: &str = "Authenticator or backup code";
    pub const CONFIRMING: &str = "Confirming…";
    pub const CONFIRM_IDENTITY: &str = "Confirm identity";
    pub const PROVIDER_HINT: &str = "Use your linked provider account. If you enabled two-factor authentication, finish that step too.";
    pub const SIGN_IN_AGAIN_BODY: &str =
        "Sign out and sign in again, then return to Account to make your security change.";
    pub const SIGN_IN_AGAIN: &str = "Sign in again";
    pub const CHECK: &str = "Check confirmation";
    pub const CHECK_FAILED: &str = "Couldn't check confirmation. Try again.";
    pub const CONFIRM_FAILED: &str = "Confirmation failed. Try again.";
    pub const TOO_MANY: &str = "Too many attempts. Wait, then try again.";
    pub const CHECK_CREDENTIALS: &str = "Check your password and authentication code.";
    pub const PROVIDER_FAILED: &str = "Sign-in couldn't start. Try again.";
}

fn lit(s: &'static str) -> impl Fn() -> SharedString + Copy + 'static {
    move || SharedString::from(s)
}

fn sub_heading(s: &'static str) -> impl IntoElement {
    text(s).text_base().font_weight(FontWeight::MEDIUM)
}

fn heading_block(title: &'static str, body: &'static str) -> impl IntoElement {
    view()
        .flex_col()
        .gap_1()
        .child(sub_heading(title))
        .child(muted(body))
}

// --- two-factor ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Idle,
    Verify,
    Enabled,
    Disabled,
}

#[derive(Clone, Copy)]
struct TwoFactor {
    step: Signal<Step>,
    setup: Signal<Option<TwoFaSetup>>,
    pending: Signal<bool>,
    error: Signal<Option<Msg>>,
    verify: FormState,
    verify_code: Signal<String>,
    disable: FormState,
    disable_code: Signal<String>,
    backup_open: Signal<bool>,
}

/// The shared refusals of the 2FA calls, in the web's order; `None` when
/// the call's own rule decides.
fn common_failure(err: &ApiError) -> Option<Msg> {
    if err.status() == 0 && matches!(err, ApiError::Net(_)) {
        return Some(Msg::Lit(form::copy::NETWORK_ERROR));
    }
    if needs_fresh_auth(err) {
        return Some(Msg::Lit(copy::FRESH_AUTH_REQUIRED));
    }
    let verified_only = err.http().is_some_and(|h| h.is_verification_required());
    if err.status() == 403 || verified_only {
        return Some(Msg::Lit(VERIFY_EMAIL_NEXT));
    }
    None
}

fn message_of(failure: Failure) -> Msg {
    match failure {
        Failure::Message(msg) => msg,
        Failure::Fields(_) | Failure::Banned(_) => Msg::Lit(form::copy::GENERIC_TRY_AGAIN),
    }
}

/// A code refusal: the server's field error, else "Invalid authentication
/// code." for 400/401/403, else the generic class message.
fn code_failure(state: FormState, error: Signal<Option<Msg>>, err: &ApiError) {
    match form::failure(err, &["code"]) {
        Failure::Fields(map) => state.server.set(map),
        _ if matches!(err.status(), 400 | 401 | 403) => {
            state
                .server
                .set([("code".to_owned(), form::copy::INVALID_TOTP.to_owned())].into());
            error.set(None);
        }
        other => error.set(Some(message_of(other))),
    }
}

impl TwoFactor {
    fn new() -> Self {
        Self {
            step: signal(Step::Idle),
            setup: signal(None),
            pending: signal(false),
            error: signal(None),
            verify: FormState::new(),
            verify_code: signal(String::new()),
            disable: FormState::new(),
            disable_code: signal(String::new()),
            backup_open: signal(false),
        }
    }

    fn begin(self) -> bool {
        if self.pending.get_untracked() {
            return false;
        }
        self.pending.set(true);
        self.error.set(None);
        true
    }

    fn start_setup(self) {
        let Some(client) = auth_store().client() else {
            return;
        };
        if !self.begin() {
            return;
        }
        spawn(async move {
            let result = client.twofa_setup(None).await;
            let _ = self.pending.try_set(false);
            match result {
                Ok(data) => {
                    self.verify.reset();
                    self.verify_code.set(String::new());
                    self.backup_open.set(false);
                    self.setup.set(Some(data));
                    self.step.set(Step::Verify);
                }
                Err(err) => {
                    let msg = common_failure(&err)
                        .unwrap_or_else(|| message_of(form::failure(&err, &[])));
                    let _ = self.error.try_set(Some(msg));
                }
            }
        });
    }

    fn confirm(self) {
        let code = self.verify_code.get_untracked().trim().to_owned();
        if !self.verify.validate(&[six_digit_rule(&code)]) {
            return;
        }
        let Some(client) = auth_store().client() else {
            return;
        };
        if !self.begin() {
            return;
        }
        self.verify.begin();
        spawn(async move {
            let result = client.twofa_verify(&Secret::new(code), None).await;
            let _ = self.pending.try_set(false);
            match result {
                Ok(user) => {
                    self.setup.set(None);
                    auth_store().set_user(user);
                    self.step.set(Step::Enabled);
                }
                Err(err) if needs_fresh_auth(&err) => {
                    self.setup.set(None);
                    self.step.set(Step::Idle);
                    self.verify_code.set(String::new());
                    self.error.set(Some(Msg::Lit(copy::CONFIRMATION_EXPIRED)));
                }
                Err(err) => match common_failure(&err) {
                    Some(msg) => self.error.set(Some(msg)),
                    None => code_failure(self.verify, self.error, &err),
                },
            }
        });
    }

    fn turn_off(self) {
        let code = self.disable_code.get_untracked().trim().to_owned();
        if !self.disable.validate(&[backup_rule(&code)]) {
            return;
        }
        let Some(client) = auth_store().client() else {
            return;
        };
        if !self.begin() {
            return;
        }
        self.disable.begin();
        spawn(async move {
            let result = client.twofa_disable(&Secret::new(code)).await;
            let _ = self.pending.try_set(false);
            match result {
                Ok(user) => {
                    self.disable_code.set(String::new());
                    self.disable.reset();
                    auth_store().set_user(user);
                    self.step.set(Step::Disabled);
                }
                Err(err) => match common_failure(&err) {
                    Some(msg) => self.error.set(Some(msg)),
                    None => code_failure(self.disable, self.error, &err),
                },
            }
        });
    }
}

/// `twoFactorDisableSchema`: a TOTP or backup code, 6 to 64 characters.
fn backup_rule(value: &str) -> Option<eq_core::i18n::Key> {
    let value = value.trim();
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_CODE_REQUIRED);
    }
    if !(6..=64).contains(&value.encode_utf16().count()) {
        return Some(keys::AUTH_ERROR_BACKUP_CODE_INVALID);
    }
    None
}

fn setup_button(s: TwoFactor) -> impl IntoElement {
    start(button_with(
        busy_label(
            move || s.pending.get(),
            lit(copy::STARTING),
            lit(copy::SET_UP),
        ),
        ButtonVariant::Primary,
        ButtonSize::Medium,
        move || s.pending.get(),
        None,
        move || s.start_setup(),
    ))
}

fn verify_panel(s: TwoFactor, data: TwoFaSetup) -> AnyElement {
    let code_value = s.verify_code;
    let backup: Vec<String> = data
        .backup_codes
        .iter()
        .map(|c| c.expose().to_owned())
        .collect();
    let backup_block = (!backup.is_empty()).then(|| {
        view()
            .flex_col()
            .gap_1p5()
            .child(label(copy::BACKUP_WARNING))
            .child(collapsible_section(
                copy::SHOW_BACKUP,
                s.backup_open,
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .children(backup.into_iter().map(|c| {
                        view()
                            .min_w(px(120.0))
                            .child(code(SharedString::from(c)))
                            .into_element()
                    })),
            ))
    });
    card()
        .p_5()
        .gap_4()
        .child(heading_block(copy::VERIFY_HEADING, copy::VERIFY_BODY))
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(label(copy::SECRET_LABEL))
                .child(code(SharedString::from(data.secret.expose().to_owned()))),
        )
        .child(backup_block)
        .child(auth_field(
            s.verify,
            "code",
            lit(copy::CODE_LABEL),
            None,
            code_value,
            text_field(code_value, "")
                .keyboard_type(KeyboardType::Number)
                .max_length(6_u32)
                .auto_capitalize(AutoCapitalize::None)
                .auto_correct(false)
                .on_submit(move |_: &str| s.confirm()),
            move || six_digit_rule(&code_value.get()),
        ))
        .child(error_line(s.error))
        .child(start(button_with(
            busy_label(
                move || s.pending.get(),
                lit(copy::VERIFYING),
                lit(copy::CONFIRM_ENABLE),
            ),
            ButtonVariant::Primary,
            ButtonSize::Medium,
            move || s.pending.get(),
            None,
            move || s.confirm(),
        )))
        .into_element()
}

fn enabled_panel(s: TwoFactor) -> AnyElement {
    let code_value = s.disable_code;
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .child(heading_block(copy::ON_HEADING, copy::ON_BODY))
        .child(auth_field(
            s.disable,
            "code",
            lit(copy::CODE_LABEL),
            None,
            code_value,
            text_field(code_value, "")
                .max_length(64_u32)
                .auto_capitalize(AutoCapitalize::None)
                .auto_correct(false)
                .on_submit(move |_: &str| s.turn_off()),
            move || backup_rule(&code_value.get()),
        ))
        .child(
            text(copy::DISABLE_HINT)
                .text_xs()
                .text_color(tokens::muted_text),
        )
        .child(error_line(s.error))
        .child(start(button_with(
            busy_label(
                move || s.pending.get(),
                lit(copy::DISABLING),
                lit(copy::DISABLE),
            ),
            ButtonVariant::Ghost,
            ButtonSize::Medium,
            move || s.pending.get(),
            None,
            move || s.turn_off(),
        )))
        .into_element()
}

fn notice_panel(s: TwoFactor, title: &'static str, body: &'static str) -> AnyElement {
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .child(heading_block(title, body))
        .child(error_line(s.error))
        .child(setup_button(s))
        .into_element()
}

/// Which 2FA panel shows (`showSetupCta` / `showVerifyPanel` / …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Unverified,
    Verify,
    Setup,
    Enabled,
    Disabled,
    None,
}

fn panel(verified: bool, enabled: bool, step: Step, has_setup: bool) -> Panel {
    if !verified {
        return Panel::Unverified;
    }
    match step {
        Step::Verify if has_setup => Panel::Verify,
        Step::Idle if !enabled => Panel::Setup,
        Step::Enabled | Step::Idle => Panel::Enabled,
        Step::Disabled => Panel::Disabled,
        Step::Verify => Panel::None,
    }
}

pub(super) fn two_factor() -> AnyElement {
    let auth = auth_store();
    let s = TwoFactor::new();
    let current = computed(move || {
        let (verified, enabled) = auth
            .user()
            .map_or((false, false), |u| (u.is_verified, u.two_fa_enabled));
        panel(
            verified,
            enabled,
            s.step.get(),
            s.setup.with(Option::is_some),
        )
    });
    dynamic(move || match current.get() {
        Panel::Unverified => muted(VERIFY_EMAIL_NEXT).into_element(),
        Panel::Verify => match s.setup.get_untracked() {
            Some(data) => verify_panel(s, data),
            None => view().into_element(),
        },
        Panel::Setup => notice_panel(s, copy::SETUP_HEADING, copy::SETUP_BODY),
        Panel::Enabled => enabled_panel(s),
        Panel::Disabled => notice_panel(s, copy::OFF_HEADING, copy::OFF_BODY),
        Panel::None => view().into_element(),
    })
    .into_element()
}

// --- security confirmation -------------------------------------------------------

#[derive(Clone, Copy)]
struct FreshAuth {
    status: Signal<Option<FreshAuthStatus>>,
    providers: Signal<Vec<OAuthProvider>>,
    password: Signal<String>,
    code: Signal<String>,
    pending: Signal<bool>,
    error: Signal<Option<Msg>>,
    sequence: Stored<u64>,
    logout: Signal<bool>,
}

fn provider_of(name: &str) -> Option<OAuthProvider> {
    oauth::providers()
        .into_iter()
        .find(|p| p.as_str() == name && is_identity_provider(name))
}

impl FreshAuth {
    fn refresh(self) {
        let auth = auth_store();
        let Some(client) = auth.client() else {
            return;
        };
        let current = self.sequence.get() + 1;
        self.sequence.with(|s| *s = current);
        self.error.set(None);
        spawn(async move {
            let status = client.reauth_status().await.ok();
            let identities = client.identities().await.ok();
            if self.sequence.get() != current {
                return;
            }
            let mut linked: Vec<String> = identities
                .map(|s| s.identities.into_iter().map(|i| i.provider).collect())
                .unwrap_or_default();
            if linked.is_empty()
                && let Some(legacy) = auth_store().user().and_then(|u| u.oauth_provider)
                && is_identity_provider(&legacy)
            {
                linked.push(legacy);
            }
            // Only the providers this build can sign in with.
            let providers = linked.iter().filter_map(|p| provider_of(p)).collect();
            let _ = self.providers.try_set(providers);
            if status.is_none() {
                let _ = self.error.try_set(Some(Msg::Lit(copy::CHECK_FAILED)));
            }
            let _ = self.status.try_set(status);
        });
    }

    fn confirm(self) {
        if self.pending.get_untracked() {
            return;
        }
        let auth = auth_store();
        let Some(client) = auth.client() else {
            return;
        };
        let owner = auth.user().map(|u| u.id);
        let password = Secret::new(self.password.get_untracked());
        let code = Secret::new(self.code.get_untracked().trim().to_owned());
        self.pending.set(true);
        self.error.set(None);
        spawn(async move {
            let result = client.reauth(&password, Some(&code)).await;
            let _ = self.pending.try_set(false);
            if owner != auth_store().user().map(|u| u.id) {
                return;
            }
            self.password.set(String::new());
            self.code.set(String::new());
            let failure = match result {
                Ok(true) => None,
                Ok(false) => Some(copy::CONFIRM_FAILED),
                Err(err) if err.status() == 429 => Some(copy::TOO_MANY),
                Err(err) if matches!(err.status(), 400 | 401) => Some(copy::CHECK_CREDENTIALS),
                Err(_) => Some(copy::CONFIRM_FAILED),
            };
            match failure {
                Some(text) => self.error.set(Some(Msg::Lit(text))),
                None => self.refresh(),
            }
        });
    }

    /// `providerSignIn`: signs in with the provider again, which opens a
    /// fresh confirmation window on the server.
    fn provider_sign_in(self, provider: OAuthProvider) {
        if self.pending.get_untracked() {
            return;
        }
        self.pending.set(true);
        self.error.set(None);
        spawn(async move {
            let outcome = oauth::sign_in(provider).await;
            let _ = self.pending.try_set(false);
            match outcome {
                Ok(SignInStep::Done(_)) => self.refresh(),
                Ok(SignInStep::TotpRequired) => replace_top(Route::Totp),
                Err(()) => {
                    let _ = self.error.try_set(Some(Msg::Lit(copy::PROVIDER_FAILED)));
                }
            }
        });
    }
}

fn password_form(f: FreshAuth) -> AnyElement {
    let two_fa = move || f.status.with(|s| s.is_some_and(|s| s.two_fa_enabled));
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .child(field(
            lit(copy::CURRENT_PASSWORD),
            password_field(f.password, "")
                .max_length(256_u32)
                .disabled(move || f.pending.get())
                .on_submit(move |_: &str| f.confirm()),
        ))
        .child(when(two_fa, move || {
            field(
                lit(copy::AUTH_OR_BACKUP),
                text_field(f.code, "")
                    .max_length(64_u32)
                    .auto_capitalize(AutoCapitalize::None)
                    .auto_correct(false)
                    .disabled(move || f.pending.get())
                    .on_submit(move |_: &str| f.confirm()),
            )
            .into_element()
        }))
        .child(start(button_with(
            busy_label(
                move || f.pending.get(),
                lit(copy::CONFIRMING),
                lit(copy::CONFIRM_IDENTITY),
            ),
            ButtonVariant::Primary,
            ButtonSize::Medium,
            move || f.pending.get(),
            None,
            move || f.confirm(),
        )))
        .into_element()
}

fn provider_block(f: FreshAuth) -> AnyElement {
    dynamic(move || {
        let providers = f.providers.get();
        if providers.is_empty() {
            let no_password = f.status.with(|s| s.is_some_and(|s| !s.has_password));
            if !no_password {
                return view().into_element();
            }
            return view()
                .flex_col()
                .gap_3()
                .child(muted(copy::SIGN_IN_AGAIN_BODY))
                .child(start(button_with(
                    copy::SIGN_IN_AGAIN,
                    ButtonVariant::Secondary,
                    ButtonSize::Medium,
                    move || f.logout.get(),
                    None,
                    move || sign_out(f.logout),
                )))
                .into_element();
        }
        view()
            .flex_col()
            .gap_2()
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .children(providers.into_iter().map(|p| {
                        button_with(
                            SharedString::from(format!("Confirm with {}", oauth::label(p))),
                            ButtonVariant::Secondary,
                            ButtonSize::Small,
                            move || f.pending.get(),
                            None,
                            move || f.provider_sign_in(p),
                        )
                        .into_element()
                    })),
            )
            .child(
                text(copy::PROVIDER_HINT)
                    .text_xs()
                    .text_color(tokens::muted_text),
            )
            .into_element()
    })
    .into_element()
}

pub(super) fn fresh_auth() -> AnyElement {
    let auth = auth_store();
    let f = FreshAuth {
        status: signal(None),
        providers: signal(Vec::new()),
        password: signal(String::new()),
        code: signal(String::new()),
        pending: signal(false),
        error: signal(None),
        sequence: stored(0),
        logout: signal(false),
    };
    // A new account on this page starts over (`authState.user?.id`).
    let owner = computed(move || auth.user().map(|u| u.id));
    effect(move || {
        let id = owner.get();
        untracked(|| {
            f.sequence.with(|s| *s += 1);
            f.status.set(None);
            f.providers.set(Vec::new());
            f.password.set(String::new());
            f.code.set(String::new());
            if id.is_some() {
                f.refresh();
            }
        });
    });
    let ready = move || f.status.with(|s| s.is_some_and(|s| s.ready));
    let has_password = move || f.status.with(|s| s.is_some_and(|s| s.has_password));
    card()
        .p_5()
        .gap_4()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(crate::features::downloads::rows::heading(
                    copy::FRESH_HEADING,
                ))
                .child(muted(copy::FRESH_BODY)),
        )
        .child(when(ready, || {
            notice_line(signal(Some(Msg::Lit(copy::FRESH_READY)))).into_element()
        }))
        .child(when(has_password, move || password_form(f)))
        .child(provider_block(f))
        .child(error_line(f.error))
        .child(start(button_with(
            copy::CHECK,
            ButtonVariant::Ghost,
            ButtonSize::Small,
            move || f.pending.get(),
            None,
            move || f.refresh(),
        )))
        .into_element()
}

/// The refusal text other account controls show for a fresh-auth refusal.
pub(super) fn fresh_auth_required() -> Msg {
    Msg::Lit(copy::FRESH_AUTH_REQUIRED)
}
