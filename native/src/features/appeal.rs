//! Ban appeal flow (PARITY I10). Contract:
//! `pub fn page(token: Option<String>) -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/appeal/+page.svelte` (`None`: paste the link or
//! code from the notice email) and `appeal/[token]/+page.svelte` (the
//! statement form), with `lib/appeal/route.ts`. The token goes in the JSON
//! body only, never in a URL or a log.

use eq_core::{api::Secret, i18n::keys};
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    app::auth_store,
    features::auth::form::auth_screen,
    shell::{nav, routes::Route},
    stores::locale::t,
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        field::{field, field_message, invalid_when, multiline_field, text_field},
        text::{body, muted, page_heading},
        tokens,
    },
};

/// The API cap (`appeal_v1` `MAX_STATEMENT_CHARS`), in code points.
const MAX_STATEMENT_CHARS: usize = 2_000;

/// `APPEAL_TOKEN_PATTERN`: 32 random bytes in lowercase hex.
fn is_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `appealTokenFromInput`: the bare token, or the link from the email.
fn token_from_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let candidate = match trimmed.rfind("/appeal/") {
        Some(at) => &trimmed[at + "/appeal/".len()..],
        None => trimmed,
    };
    let token = candidate.split(['/', '?', '#']).next().unwrap_or_default();
    is_token(token).then(|| token.to_owned())
}

fn alert(key: eq_core::i18n::Key) -> impl IntoElement {
    text(t(key)).text_sm().text_color(tokens::danger)
}

pub fn page(token: Option<String>) -> impl IntoElement {
    let content = match token {
        Some(token) => statement_form(token),
        None => entry_form(),
    };
    auth_screen(
        view()
            .flex_col()
            .gap_6()
            .child(page_heading(t(keys::APPEAL_PAGE_TITLE)))
            .child(content),
    )
}

fn entry_form() -> AnyElement {
    let entry = signal(String::new());
    let invalid = signal(false);
    let open = move || match token_from_input(&entry.get_untracked()) {
        Some(token) => {
            invalid.set(false);
            nav::pop();
            nav::push(Route::Appeal(Some(token)));
        }
        None => invalid.set(true),
    };
    view()
        .flex_col()
        .gap_3()
        .child(muted(t(keys::APPEAL_INTRO)))
        .child(field(
            t(keys::APPEAL_ENTRY_LABEL),
            invalid_when(
                text_field(entry, "")
                    .auto_capitalize(AutoCapitalize::None)
                    .auto_correct(false)
                    .on_submit(move |_: &str| open()),
                move || invalid.get(),
            ),
        ))
        .child(field_message(t(keys::APPEAL_ENTRY_HELP), || false))
        .child(when(
            move || invalid.get(),
            || alert(keys::APPEAL_INVALID_LINK),
        ))
        .child(view().flex_row().child(button_with(
            t(keys::APPEAL_ENTRY_SUBMIT),
            ButtonVariant::Primary,
            ButtonSize::Medium,
            false,
            None,
            open,
        )))
        .into_element()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Sending,
    Received,
    Error,
}

fn statement_form(token: String) -> AnyElement {
    if !is_token(&token) {
        return alert(keys::APPEAL_INVALID_LINK).into_element();
    }
    let token = Secret::new(token);
    let statement = signal(String::new());
    let invalid = signal(false);
    let phase = signal(Phase::Idle);
    let send = move || {
        if phase.get_untracked() == Phase::Sending {
            return;
        }
        let text = statement.get_untracked().trim().to_owned();
        let length = text.chars().count();
        if length == 0 || length > MAX_STATEMENT_CHARS {
            invalid.set(true);
            return;
        }
        invalid.set(false);
        phase.set(Phase::Sending);
        let token = token.clone();
        spawn(async move {
            let Some(client) = auth_store().client() else {
                phase.set(Phase::Error);
                return;
            };
            match client.appeal_submit(&token, &text).await {
                Ok(_) => {
                    statement.set(String::new());
                    phase.set(Phase::Received);
                }
                Err(err) if err.status() == 400 => {
                    invalid.set(true);
                    phase.set(Phase::Idle);
                }
                Err(_) => phase.set(Phase::Error),
            }
        });
    };
    let send = std::rc::Rc::new(send);
    let form = {
        let send = send.clone();
        move || {
            let send = send.clone();
            view()
                .flex_col()
                .gap_3()
                .child(field(
                    t(keys::APPEAL_STATEMENT_LABEL),
                    invalid_when(multiline_field(statement, ""), move || invalid.get()),
                ))
                .child(field_message(t(keys::APPEAL_STATEMENT_HELP), || false))
                .child(when(
                    move || invalid.get(),
                    || alert(keys::APPEAL_STATEMENT_INVALID),
                ))
                .child(when(
                    move || phase.get() == Phase::Error,
                    || alert(keys::APPEAL_ERROR),
                ))
                .child(view().flex_row().child(button_with(
                    move || {
                        if phase.get() == Phase::Sending {
                            return t(keys::APPEAL_SENDING)();
                        }
                        t(keys::APPEAL_SUBMIT)()
                    },
                    ButtonVariant::Primary,
                    ButtonSize::Medium,
                    move || phase.get() == Phase::Sending,
                    None,
                    move || send(),
                )))
        }
    };
    when(
        move || phase.get() == Phase::Received,
        || body(t(keys::APPEAL_RECEIVED)),
    )
    .otherwise(form)
    .into_element()
}
