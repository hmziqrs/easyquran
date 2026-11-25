//! Connected accounts (`ConnectedAccounts.svelte`, PARITY I4): the linked
//! providers, linkable providers (via native OAuth link endpoints), unlink
//! behind an inline confirm, and the last-method rule.

use eq_core::{
    api::{
        account::{can_unlink_identity, identity_provider_label},
        types::{IdentitySummary, OAuthProvider},
    },
    i18n::keys,
    sync::clock::ms_from_iso,
};
use nsh_core::{element::AnyElement, prelude::*};

use super::{go_sign_in, is_anonymous, needs_fresh_auth, section_heading, security};
use crate::{
    app::auth_store,
    features::{
        auth::{
            form::{Msg, copy::GENERIC_TRY_AGAIN, error_line},
            oauth::{self, LinkError},
        },
        bookmarks::{fmt, medium_date},
    },
    stores::locale::t,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::card,
        text::muted,
        toast::notify,
        tokens,
    },
};

#[derive(Clone, Copy)]
struct Connected {
    summary: Signal<Option<IdentitySummary>>,
    loading: Signal<bool>,
    load_error: Signal<Option<Msg>>,
    pending: Signal<Option<String>>,
    confirming: Signal<Option<String>>,
    unlink_error: Signal<Option<Msg>>,
    sequence: Stored<u64>,
}

impl Connected {
    fn load(self) {
        let Some(client) = auth_store().client() else {
            return;
        };
        let current = self.sequence.get() + 1;
        self.sequence.with(|s| *s = current);
        self.loading.set(true);
        self.load_error.set(None);
        spawn(async move {
            let result = client.identities().await;
            // A newer load wins; a late reply never overwrites it.
            if self.sequence.get() != current {
                return;
            }
            let _ = self.loading.try_set(false);
            match result {
                Ok(summary) => {
                    let _ = self.summary.try_set(Some(summary));
                }
                Err(err) if is_anonymous(&err) => go_sign_in(),
                Err(_) => {
                    let _ = self.load_error.try_set(Some(Msg::Lit(GENERIC_TRY_AGAIN)));
                }
            }
        });
    }

    fn link(self, provider: OAuthProvider) {
        if self.pending.get_untracked().is_some() {
            return;
        }
        let slug = provider.as_str().to_owned();
        self.pending.set(Some(slug));
        self.unlink_error.set(None);
        spawn(async move {
            let result = oauth::link_account(provider).await;
            let _ = self.pending.try_set(None);
            match result {
                Ok(summary) => {
                    self.sequence.with(|s| *s += 1);
                    let _ = self.loading.try_set(false);
                    let _ = self.load_error.try_set(None);
                    let _ = self.summary.try_set(Some(summary));
                    notify(fmt(
                        keys::AUTH_CONNECTED_LINKED_BANNER,
                        &[("provider", oauth::label(provider).into())],
                    ));
                }
                Err(LinkError::Cancelled) => {}
                Err(LinkError::FreshAuthRequired) => {
                    let _ = self
                        .unlink_error
                        .try_set(Some(security::fresh_auth_required()));
                }
                Err(LinkError::Api(err)) if is_anonymous(&err) => go_sign_in(),
                Err(LinkError::Api(err)) if needs_fresh_auth(&err) => {
                    let _ = self
                        .unlink_error
                        .try_set(Some(security::fresh_auth_required()));
                }
                Err(LinkError::Api(err)) => {
                    let message = err.http().and_then(|h| h.message.clone());
                    let _ = self
                        .unlink_error
                        .try_set(Some(message.map_or(Msg::Lit(GENERIC_TRY_AGAIN), Msg::Text)));
                }
            }
        });
    }

    fn unlink(self, provider: String) {
        if self.pending.get_untracked().is_some() {
            return;
        }
        let Some(client) = auth_store().client() else {
            return;
        };
        self.pending.set(Some(provider.clone()));
        self.unlink_error.set(None);
        spawn(async move {
            let result = client.unlink_identity(&provider).await;
            let _ = self.pending.try_set(None);
            let _ = self.confirming.try_set(None);
            match result {
                Ok(summary) => {
                    // The reply is newer than any load in flight.
                    self.sequence.with(|s| *s += 1);
                    self.loading.set(false);
                    self.load_error.set(None);
                    self.summary.set(Some(summary));
                }
                Err(err) if is_anonymous(&err) => go_sign_in(),
                Err(err) if needs_fresh_auth(&err) => {
                    self.unlink_error.set(Some(security::fresh_auth_required()));
                }
                Err(err) => {
                    let message = err.http().and_then(|h| h.message.clone());
                    self.unlink_error.set(Some(
                        message.map_or(Msg::Key(keys::AUTH_CONNECTED_UNLINK_ERROR), Msg::Text),
                    ));
                }
            }
        });
    }
}

fn linked_on(created_at: Option<&str>) -> Option<String> {
    let created = created_at.filter(|c| !c.is_empty())?;
    let date = ms_from_iso(created).map_or_else(|| created.to_owned(), medium_date);
    Some(fmt(
        keys::AUTH_CONNECTED_LINKED_ON,
        &[("date", date.into())],
    ))
}

fn small_button(
    label: impl IntoReactive<SharedString>,
    disabled: impl IntoReactive<bool>,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button_with(
        label,
        ButtonVariant::Ghost,
        ButtonSize::Small,
        disabled,
        None,
        on_press,
    )
}

fn identity_row(
    c: Connected,
    provider: String,
    created_at: Option<String>,
    can_unlink: bool,
) -> AnyElement {
    let name = identity_provider_label(&provider).unwrap_or_default();
    let p = provider.clone();
    let confirming = move || c.confirming.with(|x| x.as_deref() == Some(p.as_str()));
    let busy = move || c.pending.with(Option::is_some);
    let p_confirm = provider.clone();
    let p_label = provider.clone();
    let p_open = provider.clone();
    let actions = when(confirming, move || {
        let target = p_confirm.clone();
        let mine = p_label.clone();
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .child(small_button(
                move || {
                    if c.pending.with(|x| x.as_deref() == Some(mine.as_str())) {
                        return t(keys::AUTH_PLEASE_WAIT)();
                    }
                    t(keys::AUTH_CONNECTED_UNLINK_CONFIRM)()
                },
                busy,
                move || c.unlink(target.clone()),
            ))
            .child(small_button(
                t(keys::AUTH_CONNECTED_UNLINK_CANCEL),
                busy,
                move || c.confirming.set(None),
            ))
            .into_element()
    })
    .otherwise(move || {
        let target = p_open.clone();
        small_button(
            t(keys::AUTH_CONNECTED_UNLINK),
            move || !can_unlink || busy(),
            move || c.confirming.set(Some(target.clone())),
        )
        .into_element()
    });
    card()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .py_3()
        .child(
            view()
                .flex_col()
                .gap_0p5()
                .child(
                    text(SharedString::from(name))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(linked_on(created_at.as_deref()).map(|line| {
                    text(SharedString::from(line))
                        .text_xs()
                        .text_color(tokens::muted_text)
                })),
        )
        .child(actions)
        .into_element()
}

fn oauth_provider_from_str(name: &str) -> Option<OAuthProvider> {
    match name {
        "google" => Some(OAuthProvider::Google),
        "apple" => Some(OAuthProvider::Apple),
        "facebook" => Some(OAuthProvider::Facebook),
        "github" => Some(OAuthProvider::Github),
        "discord" => Some(OAuthProvider::Discord),
        _ => None,
    }
}

fn body(c: Connected) -> AnyElement {
    dynamic(move || {
        if c.load_error.with(Option::is_some) {
            return view()
                .flex_col()
                .gap_2()
                .child(error_line(c.load_error))
                .child(view().flex_row().child(small_button(
                    t(keys::AUTH_CONNECTED_RETRY),
                    false,
                    move || c.load(),
                )))
                .into_element();
        }
        let Some(summary) = c.summary.get() else {
            if c.loading.get() {
                return muted(t(keys::AUTH_CONNECTED_LOADING)).into_element();
            }
            return view().into_element();
        };
        let can_unlink = can_unlink_identity(&summary);
        let has_rows = !summary.identities.is_empty();
        let configured = oauth::providers();
        let linked_names: Vec<String> = summary
            .identities
            .iter()
            .map(|i| i.provider.clone())
            .collect();
        let linkable: Vec<OAuthProvider> = summary
            .linkable_providers
            .iter()
            .filter(|name| !linked_names.iter().any(|l| l == *name))
            .filter_map(|name| oauth_provider_from_str(name))
            .filter(|p| configured.contains(p))
            .collect();
        let busy = move || c.pending.with(Option::is_some);
        view()
            .w_full()
            .flex_col()
            .gap_3()
            .child(
                view()
                    .w_full()
                    .flex_col()
                    .gap_2()
                    .children(summary.identities.into_iter().filter_map(|i| {
                        identity_provider_label(&i.provider)?;
                        Some(identity_row(c, i.provider, i.created_at, can_unlink))
                    }))
                    .child((has_rows && !can_unlink).then(|| {
                        text(t(keys::AUTH_CONNECTED_LAST_METHOD_HINT))
                            .text_xs()
                            .text_color(tokens::muted_text)
                    })),
            )
            .child((!linkable.is_empty()).then(|| {
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .children(linkable.into_iter().map(|provider| {
                        let label = fmt(
                            keys::AUTH_CONNECTED_LINK_PROVIDER,
                            &[("provider", oauth::label(provider).into())],
                        );
                        button_with(
                            SharedString::from(label),
                            ButtonVariant::Secondary,
                            ButtonSize::Small,
                            busy,
                            None,
                            move || c.link(provider),
                        )
                    }))
            }))
            .into_element()
    })
    .into_element()
}

/// `refresh`: what else changes the sign-in methods (the passkeys, web
/// `refreshKey`); the last-method rule is read again when it changes.
pub(super) fn connected_accounts(refresh: impl Fn() -> Vec<String> + 'static) -> AnyElement {
    let c = Connected {
        summary: signal(None),
        loading: signal(true),
        load_error: signal(None),
        pending: signal(None),
        confirming: signal(None),
        unlink_error: signal(None),
        sequence: stored(0),
    };
    let key = computed(refresh);
    effect(move || {
        key.with(|_| ());
        untracked(|| c.load());
    });
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(section_heading(t(keys::AUTH_CONNECTED_ACCOUNTS_HEADING)))
                .child(muted(t(keys::AUTH_CONNECTED_ACCOUNTS_DESCRIPTION))),
        )
        .child(body(c))
        .child(error_line(c.unlink_error))
        .into_element()
}
