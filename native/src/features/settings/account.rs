//! Account (PARITY J8; web `AccountSection.svelte`): signed in, who, and
//! "Open account"; signed out, the sync note and "Sign in".

use eq_core::i18n::keys;
use nsh_core::{element::AnyElement, prelude::*};

use super::section_frame;
use crate::{
    app::auth_store,
    features::downloads::rows::{caption, list_card, row_frame, row_label},
    shell::routes::SettingsSection,
    shell::{nav, routes::Route},
    stores::locale::{t, t_args},
    ui::button::{ButtonSize, ButtonVariant, button_with},
};

fn signed_in() -> AnyElement {
    let auth = auth_store();
    let name = move || auth.user().map(|u| u.name).filter(|n| !n.is_empty());
    let email = move || auth.user().map(|u| u.email).filter(|e| !e.is_empty());
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .flex_col()
                .gap_0p5()
                .child(row_label(t(keys::SETTINGS_ACCOUNT_SIGNED_IN_HEADING)))
                .child(when(
                    move || name().is_some(),
                    move || {
                        caption(t_args(keys::SETTINGS_ACCOUNT_SIGNED_IN_AS, move || {
                            vec![("name", name().unwrap_or_default().into())]
                        }))
                        .into_element()
                    },
                ))
                .child(when(
                    move || email().is_some(),
                    move || {
                        caption(move || SharedString::from(email().unwrap_or_default()))
                            .into_element()
                    },
                )),
        )
        .child(button_with(
            t(keys::SETTINGS_ACCOUNT_OPEN),
            ButtonVariant::Ghost,
            ButtonSize::Small,
            false,
            None,
            || nav::push(Route::Account),
        ))
        .into_element()
}

fn signed_out() -> AnyElement {
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            caption(t(keys::SETTINGS_ACCOUNT_SIGNED_OUT_NOTE))
                .flex_1()
                .min_w_0(),
        )
        .child(button_with(
            t(keys::SETTINGS_ACCOUNT_SIGN_IN),
            ButtonVariant::Primary,
            ButtonSize::Small,
            false,
            None,
            || nav::push(Route::Login),
        ))
        .into_element()
}

pub fn section() -> AnyElement {
    let auth = auth_store();
    let is_signed_in = computed(move || auth.phase().user().is_some());
    section_frame(
        SettingsSection::Account,
        keys::SETTINGS_ACCOUNT_INTRO,
        view()
            .w_full()
            .flex_col()
            .gap_4()
            .child(dynamic(move || {
                let row = if is_signed_in.get() {
                    signed_in()
                } else {
                    signed_out()
                };
                list_card(vec![row]).into_element()
            }))
            .child(caption(t(keys::SETTINGS_ACCOUNT_DEVICE_NOTE))),
    )
}
