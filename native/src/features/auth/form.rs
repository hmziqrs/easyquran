//! What every auth form shares: the field rules of `web/src/lib/auth/schemas.ts`,
//! the server-error mapping of `flows.svelte.ts` (`credentialFailure`,
//! `classifyAuthError`), the per-form state (`ServerFieldErrors` plus the
//! `revalidateLogic({ mode: "submit", modeAfterSubmission: "change" })`
//! timing), and the form frame (`AuthForm.svelte`, `AuthField.svelte`).

use std::collections::BTreeMap;

use eq_core::{
    api::{ApiError, BanNotice, ErrorKind},
    i18n::{self, Key, keys},
};
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    shell::{nav, routes::Route},
    stores::locale::{locale, t},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{field, field_message, invalid_when},
        layout::{page_body, page_insets},
        text::{muted, page_intro},
        tokens,
    },
};

/// Copy the web writes as English literals in `auth-copy.ts` and the auth
/// pages (no catalogue key exists, so both locales show it, as on web).
// i18n: none (web literals in lib/auth/auth-copy.ts and routes/(auth)/**)
pub mod copy {
    pub const CREDENTIAL_FAILURE: &str = "Email or password is incorrect.";
    pub const RESET_CODE_SENT: &str =
        "If an account exists for that email, a reset code is on its way.";
    pub const ALREADY_SIGNED_IN_RESET: &str =
        "You're already signed in. Sign out first to reset your password.";
    pub const ACCOUNT_EXISTS_RESEND: &str =
        "If that account exists and is unverified, a new verification code has been sent.";
    pub const VERIFY_EMAIL_NEXT: &str =
        "Verify your email to continue. Check your inbox for the code we just sent.";
    pub const TWO_FA_NEXT: &str =
        "Enter the 6-digit code from your authenticator app to finish signing in.";
    pub const GENERIC_TRY_AGAIN: &str = "Something went wrong. Please try again.";
    pub const NETWORK_ERROR: &str = "Network error. Check your connection and try again.";
    pub const RATE_LIMIT: &str = "Too many attempts. Please wait a moment and try again.";
    pub const SIGN_IN_EXPIRED: &str = "Your sign-in session expired. Please start again.";
    pub const INVALID_TOTP: &str = "Invalid authentication code.";
    pub const SESSION_EXPIRED: &str = "Your session expired. Please log in again.";
    pub const RESET_EXPIRED: &str = "Your reset session expired. Please start again.";
    pub const CONFIRM_MISMATCH_SERVER: &str = "Password and confirm password do not match";
}

/// A message on screen: a catalogue key (follows the locale), a web
/// literal, or text the server sent for a field.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Key(Key),
    Lit(&'static str),
    Text(String),
}

impl Msg {
    /// The text in the current locale (tracked).
    pub fn show(&self) -> SharedString {
        match self {
            Msg::Key(key) => SharedString::from(i18n::tr(locale(), *key)),
            Msg::Lit(text) => SharedString::from(*text),
            Msg::Text(text) => SharedString::from(text.clone()),
        }
    }
}

/// How a refused call shows (`credentialFailure` / `classifyAuthError`).
pub enum Failure {
    Banned(BanNotice),
    Fields(BTreeMap<String, String>),
    Message(Msg),
}

/// Classifies `error` against the form's `fields`, in the web's order.
pub fn failure(error: &ApiError, fields: &[&str]) -> Failure {
    match error.classify(fields) {
        ErrorKind::Banned(notice) => Failure::Banned(notice),
        ErrorKind::Field(map) => Failure::Fields(map),
        ErrorKind::Transport => Failure::Message(Msg::Lit(copy::NETWORK_ERROR)),
        ErrorKind::RateLimited {
            retry_after: Some(secs),
        } if secs > 0 => Failure::Message(Msg::Text(format!(
            "Too many attempts. Please try again in {secs}s."
        ))),
        ErrorKind::RateLimited { .. } => Failure::Message(Msg::Lit(copy::RATE_LIMIT)),
        ErrorKind::Server => Failure::Message(Msg::Lit(copy::GENERIC_TRY_AGAIN)),
        ErrorKind::VerifiedOnly => Failure::Message(Msg::Lit(copy::VERIFY_EMAIL_NEXT)),
        ErrorKind::Credential => Failure::Message(Msg::Lit(copy::CREDENTIAL_FAILURE)),
        ErrorKind::Other => Failure::Message(
            error
                .http()
                .and_then(|e| e.message.clone())
                .map_or(Msg::Lit(copy::GENERIC_TRY_AGAIN), Msg::Text),
        ),
    }
}

/// One form's state: whether it was submitted (rules show from then on),
/// the server's field errors (dropped per field on edit), the pending
/// flag, the form-level error and the success notice.
#[derive(Clone, Copy)]
pub struct FormState {
    pub submitted: Signal<bool>,
    pub server: Signal<BTreeMap<String, String>>,
    pub pending: Signal<bool>,
    pub error: Signal<Option<Msg>>,
    pub notice: Signal<Option<Msg>>,
}

impl Default for FormState {
    fn default() -> Self {
        Self::new()
    }
}

impl FormState {
    pub fn new() -> Self {
        Self {
            submitted: signal(false),
            server: signal(BTreeMap::new()),
            pending: signal(false),
            error: signal(None),
            notice: signal(None),
        }
    }

    /// Clears the last attempt's messages before a new one.
    pub fn begin(self) {
        self.server.set(BTreeMap::new());
        self.error.set(None);
        self.notice.set(None);
    }

    /// Back to untouched (a step change, `form.reset()`).
    pub fn reset(self) {
        self.submitted.set(false);
        self.pending.set(false);
        self.begin();
    }

    /// Shows `failure`; a ban becomes the generic error (forms with a
    /// banned screen handle it before calling this).
    pub fn fail(self, failure: Failure) {
        match failure {
            Failure::Fields(map) => self.server.set(map),
            Failure::Message(msg) => self.error.set(Some(msg)),
            Failure::Banned(_) => self.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN))),
        }
    }

    /// Marks the form submitted; `true` when every rule passes.
    pub fn validate(self, errors: &[Option<Key>]) -> bool {
        self.submitted.set(true);
        errors.iter().all(Option::is_none)
    }
}

/// A labeled input with its rule and its server error: the rule shows
/// once the form was submitted, the server's message until the field is
/// edited. `hint` shows under it while there is no error.
pub fn auth_field(
    form: FormState,
    name: &'static str,
    label: impl IntoReactive<SharedString>,
    hint: Option<Key>,
    value: Signal<String>,
    input: Input,
    rule: impl Fn() -> Option<Key> + Copy + 'static,
) -> AnyElement {
    // `ServerFieldErrors.clearField`: an edit drops this field's server error.
    effect(move || {
        value.with(|_| ());
        if form.server.with_untracked(|m| m.contains_key(name)) {
            form.server.update(|m| {
                m.remove(name);
            });
        }
    });
    let error = move || -> Option<Msg> {
        if form.submitted.get()
            && let Some(key) = rule()
        {
            return Some(Msg::Key(key));
        }
        form.server.with(|m| m.get(name).cloned()).map(Msg::Text)
    };
    let has_error = move || error().is_some();
    view()
        .flex_col()
        .gap_1p5()
        .child(field(
            label,
            invalid_when(input.disabled(move || form.pending.get()), has_error),
        ))
        .child(
            when(has_error, move || {
                field_message(
                    move || error().map(|m| m.show()).unwrap_or_default(),
                    || true,
                )
            })
            .otherwise(move || match hint {
                Some(key) => field_message(t(key), || false).into_element(),
                None => view().into_element(),
            }),
        )
        .into_element()
}

/// The form-level error (`serverError`, `role="alert"`).
pub fn error_line(message: Signal<Option<Msg>>) -> impl IntoElement {
    when(
        move || message.with(Option::is_some),
        move || {
            text(move || message.get().map(|m| m.show()).unwrap_or_default())
                .text_sm()
                .text_color(tokens::danger)
        },
    )
}

/// The success notice (`successNotice`, `role="status"`).
pub fn notice_line(message: Signal<Option<Msg>>) -> impl IntoElement {
    when(
        move || message.with(Option::is_some),
        move || {
            text(move || message.get().map(|m| m.show()).unwrap_or_default())
                .text_sm()
                .text_color(tokens::success)
        },
    )
}

/// The heading and subheading of an auth form.
pub fn heading(
    title: impl Fn() -> SharedString + 'static,
    summary: impl Fn() -> SharedString + 'static,
) -> impl IntoElement {
    page_intro(title, summary)
}

/// The form's one primary action: "Please wait…" while `pending`.
pub fn submit_button(
    label: impl Fn() -> SharedString + 'static,
    pending: Signal<bool>,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let please_wait = t(keys::AUTH_PLEASE_WAIT);
    button_with(
        move || {
            if pending.get() {
                return please_wait();
            }
            label()
        },
        ButtonVariant::Primary,
        ButtonSize::Large,
        move || pending.get(),
        None,
        on_press,
    )
}

/// A quiet in-form action (the footer links, "Use a different account").
pub fn quiet_action(
    label: impl Fn() -> SharedString + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button_with(
        label,
        ButtonVariant::Ghost,
        ButtonSize::Small,
        false,
        None,
        on_press,
    )
}

/// A footer line: a muted prompt and its action.
pub fn footer_line(
    prompt: impl Fn() -> SharedString + 'static,
    action: impl IntoElement,
) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(muted(prompt))
        .child(action)
}

/// The auth page frame: a scroll with the content column at a form's
/// measure.
pub fn auth_screen(content: impl IntoElement) -> impl IntoElement {
    scroll()
        .flex_1()
        .content_inset(page_insets(0.0))
        .keyboard_dismiss_mode(KeyboardDismissMode::OnDrag)
        .child(
            view()
                .w_full()
                .items_center()
                .child(page_body().max_w(px(440.0)).gap_4().child(content)),
        )
}

/// Replaces the page on top with `route` (web `goto` from an auth page).
pub fn replace_top(route: Route) {
    nav::pop();
    nav::push(route);
}

/// Where a finished sign-in goes (`postAuthPath`): an unverified account
/// verifies its email first; else back to the page signed in from, or
/// the surah index when the auth page is the only one open.
pub fn after_sign_in(verified: bool) {
    if !verified {
        replace_top(Route::VerifyEmail);
        return;
    }
    if nav::depth() > 1 {
        nav::pop();
        return;
    }
    nav::open(Route::SurahIndex);
}

// --- rules (`schemas.ts`) -----------------------------------------------------

/// Server bounds: `PASSWORD_MIN` / `PASSWORD_MAX`.
const PASSWORD_MIN: usize = 12;
const PASSWORD_MAX: usize = 256;
/// `CODE_LEN` of `email_verification_v1`.
const EMAIL_CODE_LEN: usize = 8;
const NAME_MAX: usize = 120;

/// JS string length (UTF-16 units), as valibot counts.
fn js_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// `^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]+$` (the server's regex).
pub fn is_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    let local_ok = !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'));
    let domain_chars = domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    let Some((host, tld)) = domain.rsplit_once('.') else {
        return false;
    };
    local_ok
        && domain_chars
        && !host.is_empty()
        && !tld.is_empty()
        && tld.chars().all(|c| c.is_ascii_alphabetic())
}

pub fn email_rule(value: &str) -> Option<Key> {
    let value = value.trim();
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_EMAIL_REQUIRED);
    }
    if !is_email(value) {
        return Some(keys::AUTH_ERROR_EMAIL_INVALID);
    }
    None
}

pub fn password_rule(value: &str) -> Option<Key> {
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_PASSWORD_REQUIRED);
    }
    if js_len(value) < PASSWORD_MIN {
        return Some(keys::AUTH_ERROR_PASSWORD_MIN);
    }
    if js_len(value) > PASSWORD_MAX {
        return Some(keys::AUTH_ERROR_PASSWORD_MAX);
    }
    None
}

/// `confirmPasswordSchema` plus the `partialCheck`, which runs only once
/// both fields pass their own rules.
pub fn confirm_rule(password: &str, confirm: &str) -> Option<Key> {
    if confirm.is_empty() {
        return Some(keys::AUTH_ERROR_CONFIRM_REQUIRED);
    }
    if password_rule(password).is_none() && password != confirm {
        return Some(keys::AUTH_ERROR_CONFIRM_MISMATCH);
    }
    None
}

/// A 6-digit TOTP or reset code.
pub fn six_digit_rule(value: &str) -> Option<Key> {
    let value = value.trim();
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_CODE_REQUIRED);
    }
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Some(keys::AUTH_ERROR_CODE_DIGITS);
    }
    None
}

/// The 8-character email verification code.
pub fn email_code_rule(value: &str) -> Option<Key> {
    let value = value.trim();
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_CODE_REQUIRED);
    }
    if js_len(value) != EMAIL_CODE_LEN {
        return Some(keys::AUTH_ERROR_CODE_LENGTH_8);
    }
    None
}

pub fn name_rule(value: &str) -> Option<Key> {
    let value = value.trim();
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_NAME_REQUIRED);
    }
    if js_len(value) > NAME_MAX {
        return Some(keys::AUTH_ERROR_NAME_MAX);
    }
    None
}

/// A real calendar date written `YYYY-MM-DD`, not in the future.
pub fn date_of_birth_rule(value: &str) -> Option<Key> {
    if value.is_empty() {
        return Some(keys::AUTH_ERROR_DOB_REQUIRED);
    }
    if !is_past_date(value) {
        return Some(keys::AUTH_ERROR_DOB_INVALID);
    }
    None
}

fn is_past_date(value: &str) -> bool {
    let shaped = value.len() == 10
        && value.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        });
    if !shaped {
        return false;
    }
    let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let today = i64::try_from(now)
        .ok()
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .map(|d| d.date_naive());
    today.is_some_and(|today| date <= today)
}
