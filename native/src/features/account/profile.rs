//! Profile (name, email) and the one-time age declaration
//! (`AgeDeclaration.svelte`, PARITY I13).

use eq_core::{api::types::ProfileUpdateRequest, i18n::keys};
use nsh_core::{element::AnyElement, prelude::*};

use super::{busy_label, generic, go_sign_in, is_anonymous, section_heading, start};
use crate::{
    app::auth_store,
    features::auth::form::{Msg, error_line, notice_line},
    stores::locale::t,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{field, text_field},
        layout::card,
        text::muted,
    },
};

fn save_profile(
    name: Signal<String>,
    pending: Signal<bool>,
    error: Signal<Option<Msg>>,
    saved: Signal<Option<Msg>>,
) {
    if pending.get_untracked() {
        return;
    }
    let draft = name.get_untracked().trim().to_owned();
    if draft.is_empty() {
        error.set(Some(Msg::Key(keys::ACCOUNT_NAME_EMPTY)));
        return;
    }
    let Some(client) = auth_store().client() else {
        error.set(Some(generic()));
        return;
    };
    pending.set(true);
    error.set(None);
    saved.set(None);
    let request = ProfileUpdateRequest {
        name: Some(draft),
        ..ProfileUpdateRequest::default()
    };
    spawn(async move {
        let result = client.update_profile(&request).await;
        let _ = pending.try_set(false);
        match result {
            Ok(update) => {
                auth_store().set_user(update.user);
                let _ = saved.try_set(Some(Msg::Key(keys::ACCOUNT_PROFILE_SAVED)));
            }
            Err(err) if is_anonymous(&err) => go_sign_in(),
            Err(_) => {
                let _ = error.try_set(Some(generic()));
            }
        }
    });
}

pub(super) fn profile_section() -> AnyElement {
    let auth = auth_store();
    let user = auth.user();
    let name = signal(user.as_ref().map(|u| u.name.clone()).unwrap_or_default());
    let email = signal(user.map(|u| u.email).unwrap_or_default());
    let pending = signal(false);
    let error = signal(None::<Msg>);
    let saved = signal(None::<Msg>);
    let save = move || save_profile(name, pending, error, saved);
    view()
        .w_full()
        .flex_col()
        .gap_4()
        .child(section_heading(t(keys::ACCOUNT_PROFILE_HEADING)))
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_3()
                .child(field(
                    t(keys::ACCOUNT_FIELD_NAME),
                    text_field(name, "")
                        .auto_capitalize(AutoCapitalize::Words)
                        .disabled(move || pending.get())
                        .on_submit(move |_: &str| save()),
                ))
                .child(field(
                    t(keys::ACCOUNT_FIELD_EMAIL),
                    text_field(email, "").disabled(true),
                ))
                .child(error_line(error))
                .child(notice_line(saved))
                .child(start(button_with(
                    busy_label(
                        move || pending.get(),
                        t(keys::ACCOUNT_SAVING),
                        t(keys::ACCOUNT_SAVE_CHANGES),
                    ),
                    ButtonVariant::Primary,
                    ButtonSize::Medium,
                    move || pending.get(),
                    None,
                    save,
                ))),
        )
        .into_element()
}

/// `YYYY-MM-DD`, shape only (the server judges the date).
fn dob_shaped(value: &str) -> bool {
    value.len() == 10
        && value.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            _ => b.is_ascii_digit(),
        })
}

#[derive(Clone, Copy)]
struct AgeState {
    dob: Signal<String>,
    pending: Signal<bool>,
    error: Signal<Option<Msg>>,
    already: Signal<bool>,
    done: Signal<bool>,
}

fn declare(s: AgeState) {
    if s.pending.get_untracked() {
        return;
    }
    let dob = s.dob.get_untracked();
    if !dob_shaped(&dob) {
        s.error.set(Some(Msg::Key(keys::ACCOUNT_AGE_INVALID)));
        return;
    }
    let auth = auth_store();
    let Some(client) = auth.client() else {
        s.error.set(Some(generic()));
        return;
    };
    s.pending.set(true);
    s.error.set(None);
    spawn(async move {
        let result = client.declare_age(&dob).await;
        let _ = s.pending.try_set(false);
        // The date is sent once and not kept.
        let _ = s.dob.try_set(String::new());
        match result {
            Ok(profile) => {
                // A minor's device identity is memory-only from here on: the
                // stored id goes, so nothing persists under it.
                if profile.is_minor {
                    eq_core::device::identity::forget_device_id_for(auth.kv().as_ref(), profile.id);
                }
                auth.set_user(profile);
                let _ = s.done.try_set(true);
            }
            Err(err) if err.status() == 409 => {
                let _ = s.already.try_set(true);
            }
            Err(err) if err.status() == 400 => {
                let _ = s.error.try_set(Some(Msg::Key(keys::ACCOUNT_AGE_INVALID)));
            }
            Err(_) => {
                let _ = s.error.try_set(Some(generic()));
            }
        }
    });
}

fn age_form(s: AgeState) -> AnyElement {
    let submit = move || declare(s);
    card()
        .p_5()
        .child(muted(t(keys::ACCOUNT_AGE_DESCRIPTION)))
        .child(field(
            t(keys::ACCOUNT_AGE_FIELD),
            // The web's `type="date"`: typed as YYYY-MM-DD.
            text_field(s.dob, "YYYY-MM-DD")
                .keyboard_type(KeyboardType::NumbersAndPunctuation)
                .max_length(10_u32)
                .disabled(move || s.pending.get())
                .on_submit(move |_: &str| submit()),
        ))
        .child(error_line(s.error))
        .child(start(button_with(
            busy_label(
                move || s.pending.get(),
                t(keys::ACCOUNT_AGE_SAVING),
                t(keys::ACCOUNT_AGE_SUBMIT),
            ),
            ButtonVariant::Primary,
            ButtonSize::Medium,
            move || s.pending.get(),
            None,
            submit,
        )))
        .into_element()
}

/// Account page only, never a gate: reading and sync work the same whether
/// or not this is answered.
pub(super) fn age_declaration() -> AnyElement {
    let auth = auth_store();
    let s = AgeState {
        dob: signal(String::new()),
        pending: signal(false),
        error: signal(None),
        already: signal(false),
        done: signal(false),
    };
    let prompt =
        move || auth.user().is_some_and(|u| !u.age_declared) && !s.already.get() && !s.done.get();
    let shown = computed(move || prompt() || s.done.get());
    when(
        move || shown.get(),
        move || {
            view()
                .w_full()
                .flex_col()
                .gap_4()
                .child(section_heading(t(keys::ACCOUNT_AGE_HEADING)))
                .child(
                    when(
                        move || s.done.get(),
                        || {
                            notice_line(signal(Some(Msg::Key(keys::ACCOUNT_AGE_DONE))))
                                .into_element()
                        },
                    )
                    .otherwise(move || age_form(s)),
                )
                .into_element()
        },
    )
    .otherwise(move || {
        when(
            move || s.already.get(),
            || muted(t(keys::ACCOUNT_AGE_ALREADY)).into_element(),
        )
        .into_element()
    })
    .into_element()
}
