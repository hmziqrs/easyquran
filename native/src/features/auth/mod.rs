//! Auth flows (PARITY I2, I3, I4, I5, I13, I14). Contract:
//! `pub fn login_page() -> impl IntoElement`,
//! `pub fn register_page() -> impl IntoElement`,
//! `pub fn verify_email_page() -> impl IntoElement`,
//! `pub fn forgot_password_page() -> impl IntoElement`,
//! `pub fn totp_page() -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/(auth)/**`, `routes/auth/[provider]/totp`,
//! `lib/auth/components/{SignInForm,RegisterForm,OAuthButtons,BannedNotice}`
//! and the flows in `lib/auth/flows.svelte.ts`.
//!
//! Passkeys (I5) show where the platform has them (`services::passkeys`),
//! as `OAuthButtons` shows the button only with WebAuthn.
//!
//! Not offered on native, with the web's own rule for each:
//! - Turnstile (I14): the widget is a Cloudflare web script; native sends
//!   no `turnstile_token` (see the report's backend proposal).

pub mod form;
pub mod oauth;
pub mod passkey;

use eq_core::{
    api::{
        ApiError, AuthPhase, BanNotice, Secret, SignInStep, TransitionKind,
        types::{OAuthProvider, RegisterRequest, SignInOutcome},
    },
    i18n::keys,
};
use nsh_core::{element::AnyElement, prelude::*};

use self::form::{
    FormState, Msg, after_sign_in, auth_field, auth_screen, confirm_rule, copy, date_of_birth_rule,
    email_code_rule, email_rule, error_line, failure, footer_line, heading, name_rule, notice_line,
    password_rule, quiet_action, replace_top, six_digit_rule, submit_button,
};
use crate::{
    app::auth_store,
    shell::{nav, routes::Route},
    stores::locale::{t, t_args, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{password_field, text_field},
        layout::{divider, eyebrow},
        text::muted,
        tokens,
    },
};

const LOGIN_FIELDS: &[&str] = &["email", "password"];
const REGISTER_FIELDS: &[&str] = &[
    "name",
    "email",
    "password",
    "confirm_password",
    "date_of_birth",
];

fn static_text(s: &'static str) -> impl Fn() -> SharedString + Copy + 'static {
    move || SharedString::from(s)
}

fn email_input(value: Signal<String>) -> Input {
    text_field(value, tr(keys::AUTH_EMAIL_PLACEHOLDER))
        .keyboard_type(KeyboardType::Email)
        .auto_capitalize(AutoCapitalize::None)
        .auto_correct(false)
}

fn code_input(value: Signal<String>, max: u32) -> Input {
    text_field(value, "")
        .keyboard_type(KeyboardType::Number)
        .max_length(max)
        .auto_capitalize(AutoCapitalize::None)
        .auto_correct(false)
}

// --- sign in ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum SignInView {
    Credentials,
    Totp,
}

/// The TOTP step's state, shared by sign-in and register (`flow.login`).
#[derive(Clone, Copy)]
struct TotpStep {
    form: FormState,
    code: Signal<String>,
}

impl TotpStep {
    fn new() -> Self {
        Self {
            form: FormState::new(),
            code: signal(String::new()),
        }
    }

    /// Enters the step with `TWO_FA_NEXT` showing (`flow.genericError`).
    fn enter(self) {
        self.form.reset();
        self.code.set(String::new());
        self.form.error.set(Some(Msg::Lit(copy::TWO_FA_NEXT)));
    }

    /// `submitTotp`. `on_done` gets the account; `on_expired` runs when no
    /// sign-in is pending any more; `banned` takes a ban.
    fn submit(
        self,
        banned: Signal<Option<BanNotice>>,
        on_done: impl Fn(bool) + 'static,
        on_expired: impl Fn() + 'static,
    ) {
        let form = self.form;
        if form.pending.get_untracked() {
            return;
        }
        if !form.validate(&[six_digit_rule(&self.code.get_untracked())]) {
            return;
        }
        form.begin();
        form.pending.set(true);
        let code = Secret::new(self.code.get_untracked().trim().to_owned());
        spawn(async move {
            let result = auth_store().submit_totp(code).await;
            form.pending.set(false);
            match result {
                Ok(SignInStep::Done(user)) => on_done(user.is_verified),
                Ok(SignInStep::TotpRequired) => {}
                Err(ApiError::Decode(_))
                    if !matches!(auth_store().phase(), AuthPhase::PendingTotp) =>
                {
                    on_expired();
                }
                Err(err) => {
                    let status = err.status();
                    match failure(&err, &["code"]) {
                        form::Failure::Banned(notice) => banned.set(Some(notice)),
                        form::Failure::Fields(map) if map.contains_key("code") => {
                            form.server.set(map);
                        }
                        _ if matches!(status, 400 | 401 | 403 | 422) => {
                            form.server
                                .set([("code".to_owned(), copy::INVALID_TOTP.to_owned())].into());
                        }
                        other => form.fail(other),
                    }
                }
            }
        });
    }

    fn view(
        self,
        on_submit: impl Fn() + Copy + 'static,
        on_cancel: impl Fn() + 'static,
    ) -> AnyElement {
        let code = self.code;
        view()
            .flex_col()
            .gap_4()
            .child(heading(
                t(keys::AUTH_TOTP_HEADING),
                t(keys::AUTH_TOTP_SUBHEADING),
            ))
            .child(auth_field(
                self.form,
                "code",
                t(keys::AUTH_FIELD_AUTHENTICATION_CODE),
                None,
                code,
                code_input(code, 6).on_submit(move |_: &str| on_submit()),
                move || six_digit_rule(&code.get()),
            ))
            .child(error_line(self.form.error))
            .child(submit_button(
                t(keys::AUTH_CONTINUE),
                self.form.pending,
                on_submit,
            ))
            .child(quiet_action(t(keys::AUTH_USE_DIFFERENT_ACCOUNT), on_cancel))
            .into_element()
    }
}

/// `BannedNotice.svelte`.
fn banned_notice(notice: BanNotice, on_dismiss: impl Fn() + 'static) -> AnyElement {
    let appeal = notice.appeal_path.as_deref().and_then(appeal_route);
    view()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(tokens::danger)
        .child(
            text(t(keys::AUTH_BANNED_HEADING))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(tokens::danger),
        )
        .child(
            text(t(keys::AUTH_BANNED_BODY))
                .text_sm()
                .text_color(tokens::danger),
        )
        .children(notice.reason_code.map(|code| {
            text(t_args(keys::AUTH_BANNED_REASON_CODE, move || {
                vec![("code", code.clone().into())]
            }))
            .text_xs()
            .text_color(tokens::danger)
        }))
        .children(appeal.map(|route| {
            quiet_action(t(keys::AUTH_BANNED_APPEAL_LINK), move || {
                nav::push(route.clone());
            })
        }))
        .child(quiet_action(
            t(keys::AUTH_USE_DIFFERENT_ACCOUNT),
            on_dismiss,
        ))
        .into_element()
}

/// The in-app route of a ban's `appealPath` (`/appeal` or `/appeal/<token>`).
fn appeal_route(path: &str) -> Option<Route> {
    if path == "/appeal" {
        return Some(Route::Appeal(None));
    }
    let token = path.strip_prefix("/appeal/")?;
    let token = token.split(['/', '?', '#']).next().unwrap_or_default();
    Some(Route::Appeal(Some(token.to_owned())))
}

/// `SignInForm.svelte` (page variant). `at_totp`: the pending-proof page,
/// which opens on the code step.
fn sign_in_form(at_totp: bool) -> AnyElement {
    let email = signal(String::new());
    let password = signal(String::new());
    let creds = FormState::new();
    let totp = TotpStep::new();
    let banned = signal(None::<BanNotice>);
    let step = signal(if at_totp {
        SignInView::Totp
    } else {
        SignInView::Credentials
    });

    let submit_credentials = move || {
        if creds.pending.get_untracked() {
            return;
        }
        let valid = creds.validate(&[
            email_rule(&email.get_untracked()),
            password_rule(&password.get_untracked()),
        ]);
        if !valid {
            return;
        }
        creds.begin();
        banned.set(None);
        creds.pending.set(true);
        let address = email.get_untracked().trim().to_owned();
        let secret = Secret::new(password.get_untracked());
        spawn(async move {
            let result = auth_store().sign_in(address, secret).await;
            creds.pending.set(false);
            match result {
                Ok(SignInStep::Done(user)) => after_sign_in(user.is_verified),
                Ok(SignInStep::TotpRequired) => {
                    totp.enter();
                    step.set(SignInView::Totp);
                }
                Err(err) => match failure(&err, LOGIN_FIELDS) {
                    form::Failure::Banned(notice) => banned.set(Some(notice)),
                    other => creds.fail(other),
                },
            }
        });
    };
    let cancel_totp = move || {
        auth_store().cancel_totp();
        totp.form.reset();
        totp.code.set(String::new());
        creds.error.set(None);
        step.set(SignInView::Credentials);
    };
    let submit_totp = move || {
        totp.submit(banned, after_sign_in, move || {
            cancel_totp();
            creds.error.set(Some(Msg::Lit(copy::SIGN_IN_EXPIRED)));
        });
    };

    dynamic(move || {
        if let Some(notice) = banned.get() {
            return banned_notice(notice, move || banned.set(None));
        }
        match step.get() {
            SignInView::Totp => totp.view(submit_totp, cancel_totp),
            SignInView::Credentials => credentials_view(creds, email, password, submit_credentials),
        }
    })
}

fn credentials_view(
    creds: FormState,
    email: Signal<String>,
    password: Signal<String>,
    on_submit: impl Fn() + Copy + 'static,
) -> AnyElement {
    view()
        .flex_col()
        .gap_4()
        .child(heading(
            t(keys::AUTH_SIGN_IN),
            t(keys::AUTH_SIGN_IN_SUBHEADING),
        ))
        .child(auth_field(
            creds,
            "email",
            t(keys::AUTH_FIELD_EMAIL),
            None,
            email,
            email_input(email),
            move || email_rule(&email.get()),
        ))
        .child(auth_field(
            creds,
            "password",
            t(keys::AUTH_FIELD_PASSWORD),
            None,
            password,
            password_field(password, tr(keys::AUTH_PASSWORD_PLACEHOLDER))
                .on_submit(move |_: &str| on_submit()),
            move || password_rule(&password.get()),
        ))
        .child(error_line(creds.error))
        .child(submit_button(
            t(keys::AUTH_SIGN_IN),
            creds.pending,
            on_submit,
        ))
        .child(footer_line(
            t(keys::AUTH_FORGOT_PASSWORD_PROMPT),
            quiet_action(t(keys::AUTH_RESET_PASSWORD_LINK), || {
                nav::push(Route::ForgotPassword);
            }),
        ))
        .child(footer_line(
            t(keys::AUTH_NO_ACCOUNT_PROMPT),
            quiet_action(t(keys::AUTH_CREATE_ONE_LINK), || {
                replace_top(Route::Register)
            }),
        ))
        .into_element()
}

/// `OAuthButtons.svelte`: the providers this build has client ids for
/// (`oauth::providers`) and the passkey button where passkeys work.
fn social_section() -> AnyElement {
    let providers = oauth::providers();
    let passkeys = signal(false);
    spawn(async move {
        let _ = passkeys.try_set(crate::services::passkeys::available().await);
    });
    if providers.is_empty() {
        return when(
            move || passkeys.get(),
            move || social_buttons(Vec::new(), passkeys),
        )
        .into_element();
    }
    social_buttons(providers, passkeys)
}

fn social_buttons(providers: Vec<OAuthProvider>, passkeys: Signal<bool>) -> AnyElement {
    // One attempt at a time, whichever button it came from.
    let pending = signal(false);
    let passkey_pending = signal(false);
    let error = signal(None::<Msg>);
    let begin = move |provider: OAuthProvider| {
        if pending.get_untracked() {
            return;
        }
        pending.set(true);
        error.set(None);
        spawn(async move {
            let outcome = oauth::sign_in(provider).await;
            pending.set(false);
            match outcome {
                Ok(SignInStep::Done(user)) => after_sign_in(user.is_verified),
                Ok(SignInStep::TotpRequired) => replace_top(Route::Totp),
                Err(()) => error.set(Some(Msg::Key(keys::AUTH_OAUTH_ERROR))),
            }
        });
    };
    let passkey_sign_in = move || {
        if pending.get_untracked() {
            return;
        }
        pending.set(true);
        passkey_pending.set(true);
        error.set(None);
        spawn(async move {
            let outcome = passkey::sign_in().await;
            let _ = pending.try_set(false);
            let _ = passkey_pending.try_set(false);
            match outcome {
                Ok(SignInStep::Done(user)) => after_sign_in(user.is_verified),
                Ok(SignInStep::TotpRequired) => replace_top(Route::Totp),
                Err(failure) => {
                    let _ = error.try_set(failure.message());
                }
            }
        });
    };
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .child(view().flex_1().child(divider()))
                .child(eyebrow(t(keys::AUTH_OR_CONTINUE_WITH)))
                .child(view().flex_1().child(divider())),
        )
        .child(error_line(error))
        .children(providers.into_iter().map(move |provider| {
            button_with(
                t_args(keys::AUTH_CONTINUE_WITH_PROVIDER, move || {
                    vec![("provider", oauth::label(provider).into())]
                }),
                ButtonVariant::Outline,
                ButtonSize::Large,
                move || pending.get(),
                None,
                move || begin(provider),
            )
        }))
        .child(when(
            move || passkeys.get(),
            move || {
                let label = t(keys::AUTH_CONTINUE_WITH_PASSKEY);
                let please_wait = t(keys::AUTH_PLEASE_WAIT);
                button_with(
                    move || {
                        if passkey_pending.get() {
                            return please_wait();
                        }
                        label()
                    },
                    ButtonVariant::Outline,
                    ButtonSize::Large,
                    move || pending.get(),
                    None,
                    passkey_sign_in,
                )
            },
        ))
        .into_element()
}

pub fn login_page() -> impl IntoElement {
    auth_screen(
        view()
            .flex_col()
            .gap_6()
            .child(sign_in_form(false))
            .child(social_section()),
    )
}

// --- register --------------------------------------------------------------------

/// Where a finished registration goes (`register/+page.svelte`).
fn after_register(verified: bool) {
    if !verified {
        replace_top(Route::VerifyEmail);
        return;
    }
    nav::open(Route::SurahIndex);
}

pub fn register_page() -> impl IntoElement {
    let name = signal(String::new());
    let email = signal(String::new());
    let password = signal(String::new());
    let confirm = signal(String::new());
    let dob = signal(String::new());
    let form = FormState::new();
    let totp = TotpStep::new();
    // RegisterForm never shows the banned screen; the TOTP step can still
    // report one, which is dropped as on web.
    let banned = signal(None::<BanNotice>);
    let step = signal(SignInView::Credentials);

    let submit = move || {
        if form.pending.get_untracked() {
            return;
        }
        let (n, e, p, c, d) = (
            name.get_untracked(),
            email.get_untracked(),
            password.get_untracked(),
            confirm.get_untracked(),
            dob.get_untracked(),
        );
        let valid = form.validate(&[
            name_rule(&n),
            email_rule(&e),
            password_rule(&p),
            confirm_rule(&p, &c),
            date_of_birth_rule(&d),
        ]);
        if !valid {
            return;
        }
        form.begin();
        form.pending.set(true);
        let email_value = e.trim().to_owned();
        let request = RegisterRequest {
            name: n.trim().to_owned(),
            email: email_value.clone(),
            password: Secret::new(p.clone()),
            date_of_birth: d,
            // Turnstile is a web widget: native has no token to send.
            turnstile_token: None,
        };
        spawn(async move {
            let store = auth_store();
            let Some(client) = store.client() else {
                form.pending.set(false);
                form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                return;
            };
            let registered = client.register(&request).await;
            let result = match registered {
                Ok(()) => store.sign_in(email_value, Secret::new(p)).await,
                Err(err)
                    if err
                        .http()
                        .is_some_and(eq_core::api::HttpError::is_already_authenticated) =>
                {
                    store
                        .complete_sign_in(Err(err), TransitionKind::Login)
                        .await
                }
                Err(err) => Err(err),
            };
            form.pending.set(false);
            match result {
                Ok(SignInStep::Done(user)) => after_register(user.is_verified),
                Ok(SignInStep::TotpRequired) => {
                    totp.enter();
                    step.set(SignInView::Totp);
                }
                Err(err) => form.fail(failure(&err, REGISTER_FIELDS)),
            }
        });
    };
    let cancel_totp = move || {
        auth_store().cancel_totp();
        totp.form.reset();
        totp.code.set(String::new());
        step.set(SignInView::Credentials);
    };
    let submit_totp = move || {
        totp.submit(banned, after_register, move || {
            cancel_totp();
            form.error.set(Some(Msg::Lit(copy::SIGN_IN_EXPIRED)));
        });
    };

    let content = dynamic(move || match step.get() {
        SignInView::Totp => totp.view(submit_totp, cancel_totp),
        SignInView::Credentials => view()
            .flex_col()
            .gap_4()
            .child(heading(
                t(keys::AUTH_REGISTER_HEADING),
                t(keys::AUTH_REGISTER_SUBHEADING),
            ))
            .child(auth_field(
                form,
                "name",
                t(keys::AUTH_FIELD_DISPLAY_NAME),
                None,
                name,
                text_field(name, tr(keys::AUTH_DISPLAY_NAME_PLACEHOLDER))
                    .auto_capitalize(AutoCapitalize::Words),
                move || name_rule(&name.get()),
            ))
            .child(auth_field(
                form,
                "email",
                t(keys::AUTH_FIELD_EMAIL),
                None,
                email,
                email_input(email),
                move || email_rule(&email.get()),
            ))
            .child(auth_field(
                form,
                "password",
                t(keys::AUTH_FIELD_PASSWORD),
                None,
                password,
                password_field(password, tr(keys::AUTH_PASSWORD_MIN_PLACEHOLDER)),
                move || password_rule(&password.get()),
            ))
            .child(auth_field(
                form,
                "confirm_password",
                t(keys::AUTH_FIELD_CONFIRM_PASSWORD),
                None,
                confirm,
                password_field(confirm, tr(keys::AUTH_CONFIRM_PASSWORD_PLACEHOLDER)),
                move || confirm_rule(&password.get(), &confirm.get()),
            ))
            .child(auth_field(
                form,
                "date_of_birth",
                t(keys::AUTH_FIELD_DATE_OF_BIRTH),
                Some(keys::AUTH_FIELD_DATE_OF_BIRTH_HINT),
                dob,
                // The web's `type="date"`: typed as YYYY-MM-DD.
                text_field(dob, "YYYY-MM-DD")
                    .keyboard_type(KeyboardType::NumbersAndPunctuation)
                    .max_length(10_u32)
                    .on_submit(move |_: &str| submit()),
                move || date_of_birth_rule(&dob.get()),
            ))
            .child(error_line(form.error))
            .child(submit_button(
                t(keys::AUTH_CREATE_ACCOUNT),
                form.pending,
                submit,
            ))
            .child(footer_line(
                t(keys::AUTH_HAVE_ACCOUNT_PROMPT),
                quiet_action(t(keys::AUTH_SIGN_IN), || replace_top(Route::Login)),
            ))
            .into_element(),
    });
    auth_screen(
        view()
            .flex_col()
            .gap_6()
            .child(content)
            .child(social_section()),
    )
}

// --- verify email ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum VerifyView {
    AlreadyVerified,
    Verified,
    Form,
}

/// The verify-email reply mapping (`VerifyEmailFlow.verify` / `resend`).
fn verify_failure(err: &ApiError, fields: &[&str], form: FormState, already: Signal<bool>) {
    if matches!(err, ApiError::Net(_)) {
        form.error.set(Some(Msg::Lit(copy::NETWORK_ERROR)));
        return;
    }
    if err.status() == 401 {
        form.server.set(Default::default());
        form.error.set(Some(Msg::Lit(copy::SESSION_EXPIRED)));
        return;
    }
    let verified_only = err
        .http()
        .is_some_and(eq_core::api::HttpError::is_verification_required);
    if err.status() == 403 || verified_only {
        already.set(true);
        form.error.set(None);
        return;
    }
    form.fail(failure(err, fields));
}

/// A finished state: heading, summary and the one way on.
fn done_view(title: &'static str, summary: &'static str, action: &'static str) -> AnyElement {
    view()
        .flex_col()
        .gap_4()
        .child(heading(static_text(title), static_text(summary)))
        .child(submit_button(static_text(action), signal(false), || {
            nav::open(Route::SurahIndex);
        }))
        .into_element()
}

pub fn verify_email_page() -> impl IntoElement {
    let code = signal(String::new());
    let form = FormState::new();
    let resend_pending = signal(false);
    let already = signal(false);
    let verified = signal(false);
    let leaving = signal(false);
    let mode = computed(move || {
        let account_verified = auth_store().user().is_some_and(|u| u.is_verified);
        if verified.get() {
            return VerifyView::Verified;
        }
        if account_verified || already.get() {
            return VerifyView::AlreadyVerified;
        }
        VerifyView::Form
    });

    let submit = move || {
        if form.pending.get_untracked() {
            return;
        }
        if !form.validate(&[email_code_rule(&code.get_untracked())]) {
            return;
        }
        form.begin();
        form.pending.set(true);
        let secret = Secret::new(code.get_untracked().trim().to_owned());
        spawn(async move {
            let Some(client) = auth_store().client() else {
                form.pending.set(false);
                form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                return;
            };
            let result = client.verify_email(&secret).await;
            form.pending.set(false);
            match result {
                Ok(()) => {
                    verified.set(true);
                    auth_store().refresh_user().await;
                }
                Err(err) => verify_failure(&err, &["code"], form, already),
            }
        });
    };
    let resend = move || {
        if resend_pending.get_untracked() {
            return;
        }
        resend_pending.set(true);
        form.error.set(None);
        form.notice.set(None);
        spawn(async move {
            let Some(client) = auth_store().client() else {
                resend_pending.set(false);
                form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                return;
            };
            let result = client.resend_verification().await;
            resend_pending.set(false);
            match result {
                Ok(()) => form.notice.set(Some(Msg::Lit(copy::ACCOUNT_EXISTS_RESEND))),
                Err(err) => verify_failure(&err, &[], form, already),
            }
        });
    };
    let switch_account = move || {
        if leaving.get_untracked() {
            return;
        }
        leaving.set(true);
        spawn(async move {
            let result = auth_store().sign_out(None).await;
            leaving.set(false);
            match result {
                Ok(()) => replace_top(Route::Login),
                Err(err) => form.fail(failure(&err, &[])),
            }
        });
    };

    // i18n: none (web literals in routes/(auth)/verify-email/+page.svelte)
    let content = dynamic(move || match mode.get() {
        VerifyView::AlreadyVerified => done_view(
            "You're verified",
            "Your email is already verified. There's nothing more to do here.",
            "Go to the reader",
        ),
        VerifyView::Verified => done_view(
            "Email verified",
            "Thanks for confirming your email. Your account is ready.",
            "Continue to the reader",
        ),
        VerifyView::Form => view()
            .flex_col()
            .gap_4()
            .child(heading(
                static_text("Verify your email"),
                static_text("Enter the code we sent to your inbox to confirm your account."),
            ))
            .child(auth_field(
                form,
                "code",
                static_text("Verification code"),
                None,
                code,
                text_field(code, "")
                    .max_length(8_u32)
                    .auto_capitalize(AutoCapitalize::None)
                    .auto_correct(false)
                    .on_submit(move |_: &str| submit()),
                move || email_code_rule(&code.get()),
            ))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(button_with(
                        move || {
                            if resend_pending.get() {
                                return SharedString::from("Sending…");
                            }
                            SharedString::from("Resend code")
                        },
                        ButtonVariant::Ghost,
                        ButtonSize::Small,
                        move || resend_pending.get(),
                        None,
                        resend,
                    ))
                    .child(muted(static_text(copy::VERIFY_EMAIL_NEXT))),
            )
            .child(error_line(form.error))
            .child(notice_line(form.notice))
            .child(submit_button(
                static_text("Confirm email"),
                form.pending,
                submit,
            ))
            .child(footer_line(
                static_text("Need to switch accounts?"),
                button_with(
                    static_text("Sign in again"),
                    ButtonVariant::Ghost,
                    ButtonSize::Small,
                    move || leaving.get(),
                    None,
                    switch_account,
                ),
            ))
            .into_element(),
    });
    auth_screen(content)
}

// --- forgot password -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum ForgotView {
    Request,
    Verify,
    Reset,
}

pub fn forgot_password_page() -> impl IntoElement {
    let email = signal(String::new());
    let code = signal(String::new());
    let password = signal(String::new());
    let confirm = signal(String::new());
    let request_form = FormState::new();
    let verify_form = FormState::new();
    let reset_form = FormState::new();
    let step = signal(ForgotView::Request);
    let reset_token = signal(None::<Secret>);

    let request = move || {
        let form = request_form;
        if form.pending.get_untracked() {
            return;
        }
        if !form.validate(&[email_rule(&email.get_untracked())]) {
            return;
        }
        form.begin();
        form.pending.set(true);
        let address = email.get_untracked().trim().to_owned();
        email.set(address.clone());
        spawn(async move {
            let Some(client) = auth_store().client() else {
                form.pending.set(false);
                form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                return;
            };
            let result = client.forgot_password_request(&address).await;
            form.pending.set(false);
            match result {
                Ok(()) => {
                    form.notice.set(Some(Msg::Lit(copy::RESET_CODE_SENT)));
                    step.set(ForgotView::Verify);
                }
                Err(err)
                    if err
                        .http()
                        .is_some_and(eq_core::api::HttpError::is_already_authenticated) =>
                {
                    form.error
                        .set(Some(Msg::Lit(copy::ALREADY_SIGNED_IN_RESET)));
                }
                Err(err) => form.fail(failure(&err, &["email"])),
            }
        });
    };
    let verify = move || {
        let form = verify_form;
        if form.pending.get_untracked() {
            return;
        }
        if !form.validate(&[six_digit_rule(&code.get_untracked())]) {
            return;
        }
        form.begin();
        form.pending.set(true);
        let address = email.get_untracked();
        let secret = Secret::new(code.get_untracked().trim().to_owned());
        spawn(async move {
            let Some(client) = auth_store().client() else {
                form.pending.set(false);
                form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                return;
            };
            let result = client.forgot_password_verify(&address, &secret).await;
            form.pending.set(false);
            match result {
                Ok(token) if !token.is_empty() => {
                    reset_token.set(Some(token));
                    step.set(ForgotView::Reset);
                }
                Ok(_) | Err(ApiError::Decode(_)) => {
                    form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                }
                Err(err) => form.fail(failure(&err, &["email", "code"])),
            }
        });
    };
    let reset = move || {
        let form = reset_form;
        if form.pending.get_untracked() {
            return;
        }
        let (p, c) = (password.get_untracked(), confirm.get_untracked());
        if !form.validate(&[password_rule(&p), confirm_rule(&p, &c)]) {
            return;
        }
        let Some(token) = reset_token.get_untracked() else {
            request_form.reset();
            request_form.error.set(Some(Msg::Lit(copy::RESET_EXPIRED)));
            step.set(ForgotView::Request);
            return;
        };
        form.begin();
        form.pending.set(true);
        spawn(async move {
            let Some(client) = auth_store().client() else {
                form.pending.set(false);
                form.error.set(Some(Msg::Lit(copy::GENERIC_TRY_AGAIN)));
                return;
            };
            let result = client
                .forgot_password_reset(&token, &Secret::new(p), &Secret::new(c))
                .await;
            form.pending.set(false);
            match result {
                Ok(()) => {
                    reset_token.set(None);
                    replace_top(Route::Login);
                }
                Err(err) => form.fail(failure(&err, &["password", "confirm_password"])),
            }
        });
    };

    // i18n: none (web literals in routes/(auth)/forgot-password/+page.svelte)
    let content = dynamic(move || match step.get() {
        ForgotView::Request => view()
            .flex_col()
            .gap_4()
            .child(heading(
                static_text("Reset your password"),
                static_text("Enter your account email and we'll send a reset code to it."),
            ))
            .child(auth_field(
                request_form,
                "email",
                t(keys::AUTH_FIELD_EMAIL),
                None,
                email,
                email_input(email).on_submit(move |_: &str| request()),
                move || email_rule(&email.get()),
            ))
            .child(error_line(request_form.error))
            .child(notice_line(request_form.notice))
            .child(submit_button(
                static_text("Send reset code"),
                request_form.pending,
                request,
            ))
            .child(footer_line(
                static_text("Remembered it?"),
                quiet_action(static_text("Back to sign in"), || {
                    replace_top(Route::Login);
                }),
            ))
            .into_element(),
        ForgotView::Verify => view()
            .flex_col()
            .gap_4()
            .child(heading(
                static_text("Enter your reset code"),
                static_text("We sent a 6-digit code to your email."),
            ))
            .child(auth_field(
                verify_form,
                "code",
                static_text("Reset code"),
                None,
                code,
                code_input(code, 6).on_submit(move |_: &str| verify()),
                move || six_digit_rule(&code.get()),
            ))
            .child(error_line(verify_form.error))
            .child(submit_button(
                static_text("Verify code"),
                verify_form.pending,
                verify,
            ))
            .into_element(),
        ForgotView::Reset => view()
            .flex_col()
            .gap_4()
            .child(heading(
                static_text("Set a new password"),
                static_text("Choose a strong password of at least 12 characters."),
            ))
            .child(auth_field(
                reset_form,
                "password",
                static_text("New password"),
                None,
                password,
                password_field(password, ""),
                move || password_rule(&password.get()),
            ))
            .child(auth_field(
                reset_form,
                "confirm_password",
                static_text("Confirm new password"),
                None,
                confirm,
                password_field(confirm, "").on_submit(move |_: &str| reset()),
                move || confirm_rule(&password.get(), &confirm.get()),
            ))
            .child(error_line(reset_form.error))
            .child(submit_button(
                static_text("Reset password"),
                reset_form.pending,
                reset,
            ))
            .into_element(),
    });
    auth_screen(content)
}

// --- TOTP (pending proof) --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProofView {
    Loading,
    Failed,
    Ready,
}

/// Reads the pending sign-in (`/auth/v1/login/totp/pending`) and opens the
/// code step on it; no pending sign-in goes back to the login page.
fn load_pending_proof(state: Signal<ProofView>) {
    state.set(ProofView::Loading);
    let store = auth_store();
    if untracked(|| matches!(store.phase(), AuthPhase::PendingTotp)) {
        state.set(ProofView::Ready);
        return;
    }
    spawn(async move {
        let Some(client) = store.client() else {
            state.set(ProofView::Failed);
            return;
        };
        match client.login_totp_pending().await {
            Ok(Some(token)) => {
                let adopted = store
                    .complete_sign_in(
                        Ok(SignInOutcome::TotpRequired(token)),
                        TransitionKind::Login,
                    )
                    .await;
                let next = match adopted {
                    Ok(_) => ProofView::Ready,
                    Err(_) => ProofView::Failed,
                };
                state.set(next);
            }
            Ok(None) => replace_top(Route::Login),
            Err(_) => state.set(ProofView::Failed),
        }
    });
}

pub fn totp_page() -> impl IntoElement {
    let state = signal(ProofView::Loading);
    let started = signal(false);
    effect(move || {
        if !auth_store().ready() || started.get_untracked() {
            return;
        }
        started.set(true);
        load_pending_proof(state);
    });
    // i18n: none (web literals in routes/auth/[provider]/totp/+page.svelte)
    auth_screen(dynamic(move || match state.get() {
        ProofView::Ready => sign_in_form(true),
        ProofView::Failed => view()
            .flex_col()
            .gap_3()
            .items_center()
            .child(text(static_text("Sign-in couldn't continue. Try again.")).text_sm())
            .child(quiet_action(static_text("Try again"), move || {
                load_pending_proof(state);
            }))
            .into_element(),
        ProofView::Loading => muted(static_text("Finishing your sign-in…")).into_element(),
    }))
}
