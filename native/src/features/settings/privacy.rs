//! Privacy (PARITY J6, N1; web `PrivacySection.svelte`): the analytics
//! consent toggle (held off for a minor account), the notifications line,
//! goal reminders (a later slice: [`goal_reminders`]), sign out.

use eq_core::i18n::keys;
use nsh_core::{element::AnyElement, prelude::*};

use super::section_frame;
use crate::{
    app::auth_store,
    features::downloads::rows::{caption, list_card, row_frame, row_label},
    shell::routes::SettingsSection,
    stores::{
        consent::ConsentStore,
        locale::{t, tr},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        tokens,
    },
};

/// Goal reminders (web `GoalReminders.svelte`, between notifications and
/// sign out): a later slice builds it and adds it to [`section`]'s card.
pub fn goal_reminders() -> AnyElement {
    crate::features::reminders::goal_reminders()
}

fn analytics_row() -> AnyElement {
    let consent = ConsentStore::get();
    let label = move || {
        let key = if consent.analytics() {
            keys::READER_ON
        } else {
            keys::READER_OFF
        };
        SharedString::from(tr(key))
    };
    let toggle = crate::ui::button::choice_button(
        label,
        tr(keys::READER_ANALYTICS),
        move || consent.analytics(),
        move || consent.minor_gate(),
        move || consent.set_analytics(!consent.analytics()),
    );
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(row_label(t(keys::READER_ANALYTICS)))
        .child(toggle)
        .into_element()
}

/// Native push is not configured in this build: the web's "unavailable"
/// line.
fn notifications_row() -> AnyElement {
    row_frame()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_4()
        .child(row_label(t(keys::READER_NOTIFICATIONS)))
        .child(caption(t(keys::READER_NOTIFICATIONS_UNAVAILABLE)))
        .into_element()
}

fn sign_out_row() -> AnyElement {
    let auth = auth_store();
    let pending = signal(false);
    let failed = signal(false);
    let signed_in = move || auth.phase().user().is_some();
    when(signed_in, move || {
        row_frame()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(when(
                move || failed.get(),
                || {
                    caption(t(keys::SETTINGS_PRIVACY_SIGN_OUT_ERROR))
                        .text_color(tokens::danger)
                        .into_element()
                },
            ))
            .child(view().flex_1())
            .child(button_with(
                t(keys::SETTINGS_PRIVACY_SIGN_OUT),
                ButtonVariant::Ghost,
                ButtonSize::Small,
                move || pending.get(),
                None,
                move || {
                    if pending.get_untracked() {
                        return;
                    }
                    pending.set(true);
                    failed.set(false);
                    spawn(async move {
                        let result = auth.sign_out(None).await;
                        pending.set(false);
                        failed.set(result.is_err());
                    });
                },
            ))
            .into_element()
    })
    .into_element()
}

pub fn section() -> AnyElement {
    section_frame(
        SettingsSection::Privacy,
        keys::SETTINGS_PRIVACY_INTRO,
        view()
            .w_full()
            .flex_col()
            .gap_4()
            .child(list_card(vec![
                analytics_row(),
                notifications_row(),
                super::goal_reminders(),
                sign_out_row(),
            ]))
            .child(caption(t(keys::SETTINGS_PRIVACY_SYNC_NOTE))),
    )
}
