//! The Passkeys card of the account page (PARITY I5): list, add, remove.
//! The web has no rename. Adding and removing each spend one security
//! confirmation (`fresh_auth::consume`), so a refusal points at it.

use eq_core::{
    api::{ApiError, types::PasskeyView},
    i18n::keys,
};
use nsh_core::{element::AnyElement, prelude::*};

use super::{busy_label, needs_fresh_auth, section_sub_heading, security, start};
use crate::{
    app::auth_store,
    features::auth::{
        form::{Msg, error_line},
        passkey::{self, PasskeyFailure},
    },
    services::passkeys as platform,
    stores::locale::t,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::card,
        text::muted,
    },
};

#[derive(Clone, Copy)]
pub(super) struct Passkeys {
    items: Signal<Vec<PasskeyView>>,
    /// `None` until the platform answered.
    supported: Signal<Option<bool>>,
    adding: Signal<bool>,
    removing: Signal<bool>,
    error: Signal<Option<Msg>>,
}

impl Passkeys {
    /// Asks the platform and, where passkeys work, loads the list.
    pub(super) fn new() -> Self {
        let this = Self {
            items: signal(Vec::new()),
            supported: signal(None),
            adding: signal(false),
            removing: signal(false),
            error: signal(None),
        };
        spawn(async move {
            let supported = platform::available().await;
            let _ = this.supported.try_set(Some(supported));
            if supported {
                this.load().await;
            }
        });
        this
    }

    /// The passkeys on the page (tracked): the connected accounts reload
    /// with them, as their last-method rule counts passkeys.
    pub(super) fn ids(self) -> Vec<String> {
        self.items
            .with(|items| items.iter().map(|p| p.credential_id.clone()).collect())
    }

    fn busy(self) -> bool {
        self.adding.get() || self.removing.get()
    }

    fn begin(self) -> bool {
        if self.adding.get_untracked() || self.removing.get_untracked() {
            return false;
        }
        self.error.set(None);
        true
    }

    /// A list that cannot load shows as empty, as on web.
    async fn load(self) {
        let Some(client) = auth_store().client() else {
            return;
        };
        if let Ok(items) = client.passkeys().await {
            let _ = self.items.try_set(items);
        }
    }

    fn add(self) {
        if !self.begin() {
            return;
        }
        self.adding.set(true);
        spawn(async move {
            let result = passkey::register().await;
            let _ = self.adding.try_set(false);
            match result {
                Ok(made) => {
                    // Gone when the page closed during the sheet.
                    if self.items.try_get().is_some() {
                        self.items.update(|items| {
                            items.retain(|p| p.credential_id != made.credential_id);
                            items.push(made);
                        });
                    }
                    auth_store().refresh_user().await;
                }
                Err(failure) => {
                    let _ = self.error.try_set(add_failure(&failure));
                }
            }
        });
    }

    fn remove(self, id: String) {
        if !self.begin() {
            return;
        }
        let Some(client) = auth_store().client() else {
            return;
        };
        self.removing.set(true);
        spawn(async move {
            let result = client.passkey_remove(&id).await;
            let _ = self.removing.try_set(false);
            match result {
                Ok(()) => {
                    if self.items.try_get().is_some() {
                        self.items
                            .update(|items| items.retain(|p| p.credential_id != id));
                    }
                }
                Err(error) => {
                    let _ = self.error.try_set(remove_failure(error));
                }
            }
        });
    }
}

/// `register`'s refusals: fresh auth, then an unverified account, then the
/// shared rules.
fn add_failure(failure: &PasskeyFailure) -> Option<Msg> {
    if let PasskeyFailure::Api(error) = failure
        && let Some(message) = security::common_failure(error)
    {
        return Some(message);
    }
    failure.message()
}

/// `remove`'s refusals (`fail`).
fn remove_failure(error: ApiError) -> Option<Msg> {
    if needs_fresh_auth(&error) {
        return Some(security::fresh_auth_required());
    }
    PasskeyFailure::Api(error).message()
}

fn row(p: Passkeys, item: PasskeyView) -> AnyElement {
    let id = item.credential_id;
    let label = item.device_type.filter(|kind| !kind.is_empty());
    let name = move || match &label {
        Some(kind) => SharedString::from(kind.clone()),
        None => t(keys::ACCOUNT_PASSKEY_DEFAULT_LABEL)(),
    };
    view()
        .w_full()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(text(name).text_sm().flex_1().min_w_0())
        .child(button_with(
            t(keys::ACCOUNT_PASSKEY_REMOVE),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            move || p.busy(),
            None,
            move || p.remove(id.clone()),
        ))
        .into_element()
}

fn controls(p: Passkeys) -> AnyElement {
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(dynamic(move || {
            let items = p.items.get();
            if items.is_empty() {
                return view().into_element();
            }
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .children(items.into_iter().map(|item| row(p, item)))
                .into_element()
        }))
        .child(error_line(p.error))
        .child(start(button_with(
            busy_label(
                move || p.adding.get(),
                t(keys::ACCOUNT_PASSKEY_ADDING),
                t(keys::ACCOUNT_PASSKEY_ADD),
            ),
            ButtonVariant::Primary,
            ButtonSize::Medium,
            move || p.busy(),
            None,
            move || p.add(),
        )))
        .into_element()
}

pub(super) fn passkeys_card(p: Passkeys) -> AnyElement {
    card()
        .p_5()
        .gap_4()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(section_sub_heading(t(keys::ACCOUNT_PASSKEYS_HEADING)))
                .child(muted(t(keys::ACCOUNT_PASSKEYS_DESCRIPTION))),
        )
        .child(dynamic(move || match p.supported.get() {
            Some(true) => controls(p),
            Some(false) => muted(t(keys::ACCOUNT_PASSKEYS_UNSUPPORTED)).into_element(),
            None => view().into_element(),
        }))
        .into_element()
}
