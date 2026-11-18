//! Newsletter confirm (PARITY I12; the signup form sits in the footer).
//! Contract: `pub fn confirm_page(email: String, token: String) -> impl IntoElement`.
//!
//! Mirrors `web/src/routes/newsletter/confirm/+page.svelte`: the link's
//! email and token (its fragment) are posted once to
//! `/newsletter/v1/confirm`; the page says how it went.

use eq_core::{api::Secret, i18n::keys};
use nsh_core::prelude::*;

use crate::{
    app::auth_store,
    features::auth::form::auth_screen,
    stores::locale::t,
    ui::text::{body, page_heading},
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Pending,
    Sent,
    Missing,
    Error,
}

pub fn confirm_page(email: String, token: String) -> impl IntoElement {
    let email = email.trim().to_owned();
    let token = token.trim().to_owned();
    let status = signal(Status::Pending);
    let started = signal(false);
    if email.is_empty() || token.is_empty() {
        status.set(Status::Missing);
    }
    let token = Secret::new(token);
    // Once the API client is up (the auth store starts it), post once.
    effect(move || {
        if !auth_store().ready() || started.get_untracked() {
            return;
        }
        started.set(true);
        if status.get_untracked() == Status::Missing {
            return;
        }
        let (email, token) = (email.clone(), token.clone());
        spawn(async move {
            let Some(client) = auth_store().client() else {
                status.set(Status::Error);
                return;
            };
            let next = match client.newsletter_confirm(&email, &token).await {
                Ok(()) => Status::Sent,
                Err(_) => Status::Error,
            };
            status.set(next);
        });
    });
    let message = move || {
        let key = match status.get() {
            Status::Sent => keys::NEWSLETTER_CONFIRM_SUCCESS,
            Status::Missing => keys::NEWSLETTER_CONFIRM_MISSING,
            Status::Error => keys::NEWSLETTER_CONFIRM_ERROR,
            Status::Pending => keys::NEWSLETTER_CONFIRM_PENDING,
        };
        t(key)()
    };
    auth_screen(
        view()
            .flex_col()
            .gap_4()
            .child(page_heading(t(keys::NEWSLETTER_CONFIRM_HEADING)))
            .child(body(message)),
    )
}
