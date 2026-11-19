//! Delete account (`DeleteAccountControls.svelte`, `account-purge.ts`,
//! PARITY I9). The server ends every session before it erases; on success
//! this install drops the account's records (`AuthStore::account_deleted`:
//! account-keyed values, device key, then the app root purges its sync
//! file). Device-only reading data stays.

use eq_core::i18n::keys;
use nsh_core::{element::AnyElement, prelude::*};

use super::{busy_label, generic, needs_fresh_auth, section_heading, start};
use crate::{
    app::auth_store,
    features::{
        auth::form::{Msg, error_line},
        settings::export_data,
    },
    shell::{nav, routes::Route},
    stores::locale::{t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{field, text_field},
        layout::card,
        text::muted,
        toast::notify,
    },
};

fn remove(typed: Signal<String>, pending: Signal<bool>, error: Signal<Option<Msg>>) {
    if pending.get_untracked() {
        return;
    }
    let auth = auth_store();
    let Some(user) = auth.user() else {
        return;
    };
    let confirm = typed.get_untracked().trim().to_owned();
    if confirm.to_lowercase() != user.email.trim().to_lowercase() {
        error.set(Some(Msg::Key(keys::ACCOUNT_DELETE_MISMATCH)));
        return;
    }
    let Some(client) = auth.client() else {
        error.set(Some(generic()));
        return;
    };
    pending.set(true);
    error.set(None);
    spawn(async move {
        let result = client.delete_account(&confirm).await;
        let _ = pending.try_set(false);
        match result {
            Ok(_) => {
                let _ = typed.try_set(String::new());
                notify(tr(keys::ACCOUNT_DELETE_DONE));
                auth_store().account_deleted(user.id);
                nav::open(Route::Home);
            }
            Err(err) if needs_fresh_auth(&err) => {
                let _ = error.try_set(Some(Msg::Key(keys::ACCOUNT_DELETE_FRESH_AUTH)));
            }
            Err(_) => {
                let _ = error.try_set(Some(generic()));
            }
        }
    });
}

pub(super) fn delete_account() -> AnyElement {
    let typed = signal(String::new());
    let pending = signal(false);
    let error = signal(None::<Msg>);
    let submit = move || remove(typed, pending, error);
    card()
        .p_5()
        .gap_4()
        .child(
            view()
                .flex_col()
                .gap_1()
                .child(section_heading(t(keys::ACCOUNT_DELETE_HEADING)))
                .child(muted(t(keys::ACCOUNT_DELETE_DESCRIPTION))),
        )
        .child(
            view()
                .flex_col()
                .gap_2()
                .child(muted(t(keys::ACCOUNT_DELETE_EXPORT_HINT)))
                .child(start(button_with(
                    t(keys::ACCOUNT_DELETE_EXPORT),
                    ButtonVariant::Secondary,
                    ButtonSize::Medium,
                    false,
                    None,
                    export_data,
                ))),
        )
        .child(field(
            t(keys::ACCOUNT_DELETE_FIELD_EMAIL),
            text_field(typed, "")
                .keyboard_type(KeyboardType::Email)
                .auto_capitalize(AutoCapitalize::None)
                .auto_correct(false)
                .max_length(320_u32)
                .disabled(move || pending.get())
                .on_submit(move |_: &str| submit()),
        ))
        .child(start(button_with(
            busy_label(
                move || pending.get(),
                t(keys::ACCOUNT_DELETE_DELETING),
                t(keys::ACCOUNT_DELETE_SUBMIT),
            ),
            ButtonVariant::Danger,
            ButtonSize::Medium,
            move || pending.get(),
            None,
            submit,
        )))
        .child(error_line(error))
        .into_element()
}
