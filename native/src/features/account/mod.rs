//! Account page (PARITY I3 settings side, I4 unlink, I6, I7, I9, I13).
//! Contract: `pub fn page() -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/(account)/account/+page.svelte` and the
//! controls it mounts (`lib/auth/components/{AgeDeclaration,
//! TwoFactorControls,FreshAuthControls,ConnectedAccounts,
//! DeleteAccountControls}.svelte`): loading, signed-out and signed-in
//! views; profile; age declaration; devices and sessions; two-factor;
//! security confirmation; connected accounts; delete account.
//!
//! Passkeys (I5) work where the platform has them for the app
//! (`services::passkeys`); elsewhere the card says so, as on web.

mod connected;
mod delete;
mod devices;
mod passkeys;
mod profile;
mod security;

use eq_core::{
    api::{ApiError, AuthPhase},
    i18n::keys,
};
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    app::auth_store,
    features::auth::form::{Msg, replace_top},
    shell::{nav, routes::Route},
    stores::locale::t,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::{page_body, page_insets},
        text::{muted, title},
    },
};

/// Which of the page's three views shows (`resolveAccountView`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountView {
    Loading,
    Anonymous,
    Authenticated,
}

fn resolve_view(ready: bool, phase: &AuthPhase) -> AccountView {
    if phase.user().is_some() {
        return AccountView::Authenticated;
    }
    if !ready {
        return AccountView::Loading;
    }
    match phase {
        AuthPhase::SignedOut { .. } | AuthPhase::Banned(_) => AccountView::Anonymous,
        _ => AccountView::Loading,
    }
}

/// The reply means the session is gone (the web's `status === "anonymous"`).
pub(super) fn is_anonymous(err: &ApiError) -> bool {
    err.status() == 401 || err.is_session_lost()
}

/// The fresh-auth refusal (`isFreshAuthenticationRequired`).
pub(super) fn needs_fresh_auth(err: &ApiError) -> bool {
    err.http().is_some_and(|h| h.is_fresh_auth_required())
}

/// `goto(ANONYMOUS_CTA_HREF)`: the sign-in page in this one's place.
pub(super) fn go_sign_in() {
    replace_top(Route::Login);
}

/// `createLogoutFlow().run()` then `goto("/login")`; one flow per page.
pub(super) fn sign_out(pending: Signal<bool>) {
    if pending.get_untracked() {
        return;
    }
    pending.set(true);
    spawn(async move {
        let result = auth_store().sign_out(None).await;
        let _ = pending.try_set(false);
        if result.is_ok() {
            go_sign_in();
        }
    });
}

/// The generic failure line (`copy.genericError`).
pub(super) fn generic() -> Msg {
    Msg::Key(keys::ACCOUNT_GENERIC_ERROR)
}

/// A labeled form section heading (`h2 text-lg`).
pub(super) fn section_heading(label: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    crate::features::downloads::rows::heading(label)
}

/// A card's heading inside a section (`h3 text-base font-medium`).
pub(super) fn section_sub_heading(
    label: impl IntoReactive<SharedString>,
) -> nsh_core::element::Text {
    text(label).text_base().font_weight(FontWeight::MEDIUM)
}

/// A button that starts at the column's leading edge (`self-start`).
pub(super) fn start(button: impl IntoElement) -> impl IntoElement {
    view().flex_row().child(button)
}

/// A label that reads `busy` while `pending` holds.
pub(super) fn busy_label(
    pending: impl Fn() -> bool + 'static,
    busy: impl Fn() -> SharedString + 'static,
    idle: impl Fn() -> SharedString + 'static,
) -> impl Fn() -> SharedString + 'static {
    move || {
        if pending() {
            return busy();
        }
        idle()
    }
}

fn loading_view() -> AnyElement {
    view()
        .w_full()
        .flex_col()
        .items_center()
        .gap_4()
        .py_12()
        .child(
            view()
                .flex_col()
                .items_center()
                .gap_1()
                .child(title(t(keys::ACCOUNT_LOADING_HEADING)))
                .child(muted(t(keys::ACCOUNT_LOADING_BODY))),
        )
        .child(button_with(
            t(keys::ACCOUNT_RETRY),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            false,
            None,
            || {
                spawn(async {
                    let _ = auth_store().refresh_user().await;
                });
            },
        ))
        .into_element()
}

fn anonymous_view() -> AnyElement {
    view()
        .w_full()
        .max_w(px(420.0))
        .flex_col()
        .gap_6()
        .child(
            view()
                .flex_col()
                .gap_2()
                .child(title(t(keys::ACCOUNT_HEADING)))
                .child(muted(t(keys::ACCOUNT_ANONYMOUS_BODY))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(button_with(
                    t(keys::ACCOUNT_SIGN_IN),
                    ButtonVariant::Primary,
                    ButtonSize::Large,
                    false,
                    None,
                    || nav::push(Route::Login),
                ))
                .child(button_with(
                    t(keys::ACCOUNT_CREATE_ACCOUNT),
                    ButtonVariant::Secondary,
                    ButtonSize::Large,
                    false,
                    None,
                    || nav::push(Route::Register),
                )),
        )
        .into_element()
}

fn header(logout: Signal<bool>) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_1()
                .child(title(t(keys::ACCOUNT_HEADING)))
                .child(muted(t(keys::ACCOUNT_SIGNED_IN_BODY))),
        )
        .child(button_with(
            busy_label(
                move || logout.get(),
                t(keys::ACCOUNT_SIGNING_OUT),
                t(keys::ACCOUNT_SIGN_OUT),
            ),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            move || logout.get(),
            None,
            move || sign_out(logout),
        ))
}

fn signed_in_view() -> AnyElement {
    let logout = signal(false);
    let passkeys = passkeys::Passkeys::new();
    view()
        .w_full()
        .flex_col()
        .gap_10()
        .child(header(logout))
        .child(profile::profile_section())
        .child(profile::age_declaration())
        .child(devices::devices_section(logout))
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_4()
                .child(section_heading(t(keys::ACCOUNT_SECURITY_HEADING)))
                .child(security::two_factor())
                .child(passkeys::passkeys_card(passkeys)),
        )
        .child(security::fresh_auth())
        .child(connected::connected_accounts(move || passkeys.ids()))
        .child(delete::delete_account())
        .into_element()
}

pub fn page() -> impl IntoElement {
    let auth = auth_store();
    let current = computed(move || resolve_view(auth.ready(), &auth.phase()));
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        view()
            .w_full()
            .items_center()
            .child(
                page_body()
                    .max_w(px(640.0))
                    .child(dynamic(move || match current.get() {
                        AccountView::Loading => loading_view(),
                        AccountView::Anonymous => anonymous_view(),
                        AccountView::Authenticated => signed_in_view(),
                    })),
            ),
    )
}
