//! The sync line (web `SyncIndicator.svelte`) and the app-wide sync notices
//! (web `SyncNotices.svelte`, PARITY F4).

use std::time::Duration;

use eq_core::{api::AuthPhase, i18n::keys, sync::engine::Phase};
use nsh_base::theme;
use nsh_core::prelude::*;

use super::data::{Shelf, fmt};
use crate::{
    app::auth_store,
    shell::{nav, routes::Route},
    stores::locale::{t, tr},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        tokens,
    },
};

/// The status line's words, truth order (ACCT-03): offline, error,
/// pending, then the in-flight and never-synced windows; "Synced" only
/// once a round landed.
fn label(online: bool, phase: Phase, pending: usize, synced_once: bool) -> String {
    if !online {
        return tr(keys::READER_BOOKMARKS_OFFLINE).to_owned();
    }
    if phase == Phase::Error {
        return tr(keys::READER_BOOKMARKS_SYNC_ERROR).to_owned();
    }
    if pending == 1 {
        return tr(keys::READER_BOOKMARKS_PENDING_ONE).to_owned();
    }
    if pending > 1 {
        return fmt(keys::READER_BOOKMARKS_PENDING, &[("count", pending.into())]);
    }
    if phase == Phase::Syncing || !synced_once {
        return tr(keys::READER_BOOKMARKS_SYNCING).to_owned();
    }
    tr(keys::READER_BOOKMARKS_SYNCED).to_owned()
}

/// Error: danger; offline or anything not yet true: muted; synced: primary.
fn dot(online: bool, phase: Phase, pending: usize, synced_once: bool) -> AdaptiveColor {
    if phase == Phase::Error {
        return tokens::danger();
    }
    if !online || pending > 0 || phase == Phase::Syncing || !synced_once {
        return tokens::muted_text();
    }
    theme().primary
}

/// The bookmarks sync line with its save, refused and capped lines.
pub(crate) fn sync_indicator(shelf: Signal<Shelf>) -> impl IntoElement {
    let sync = crate::features::bookmarks::store();
    let parts = move || {
        let status = sync.status();
        let pending = shelf.with(|s| s.pending);
        (
            crate::stores::connectivity::online(),
            status.phase,
            pending,
            status.last_sync_at.is_some(),
        )
    };
    let rejected = move || shelf.with(|s| s.rejected);
    let capped = move || shelf.with(|s| s.capped);
    view()
        .flex_col()
        .items_end()
        .gap_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_1p5()
                .child(view().size(px(6.0)).rounded_full().bg(move || {
                    let (o, p, n, s) = parts();
                    dot(o, p, n, s)
                }))
                .child(
                    text(move || {
                        let (o, p, n, s) = parts();
                        SharedString::from(label(o, p, n, s))
                    })
                    .text_sm()
                    .text_color(tokens::muted_text),
                ),
        )
        .child(when(
            move || sync.status().degraded,
            || {
                text(t(keys::READER_BOOKMARKS_SAVE_ERROR))
                    .text_sm()
                    .text_color(tokens::danger)
            },
        ))
        .child(when(
            move || rejected() > 0,
            move || {
                text(move || {
                    SharedString::from(fmt(
                        keys::READER_BOOKMARKS_ROWS_REJECTED,
                        &[("count", rejected().into())],
                    ))
                })
                .text_sm()
                .text_color(tokens::danger)
            },
        ))
        .child(when(
            move || capped() > 0,
            move || {
                text(move || {
                    SharedString::from(fmt(
                        keys::READER_BOOKMARKS_ROWS_CAPPED,
                        &[("count", capped().into())],
                    ))
                })
                .text_sm()
                .text_color(tokens::muted_text)
            },
        ))
}

/// How long the sign-out note stays.
const SIGN_OUT_NOTE: Duration = Duration::from_secs(8);

fn notice_card() -> nsh_core::element::View {
    view()
        .w_full()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .p(px(14.0))
        .rounded(tokens::radius_xl)
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::surface)
}

/// The app-wide sync notices: "Session expired — sign in to resume sync"
/// (stays until the next sign-in; covers a session lost mid-use), and,
/// for 8 s after a sign-out, the note that unsynced changes stay on this
/// device. Mount once per surface, at the top of the content.
pub fn sync_notices() -> impl IntoElement {
    let auth = auth_store();
    let expired = move || {
        matches!(
            auth.phase(),
            AuthPhase::SignedOut {
                session_expired: true
            }
        )
    };
    let show_note = signal(false);
    let was_signed_in = stored(false);
    effect(move || {
        let phase = auth.phase();
        let signed_in = phase.user().is_some();
        let before = was_signed_in.replace(signed_in);
        let plain_sign_out = before
            && matches!(
                phase,
                AuthPhase::SignedOut {
                    session_expired: false
                }
            );
        if plain_sign_out {
            show_note.set(true);
            set_timeout(SIGN_OUT_NOTE, move || {
                let _ = show_note.try_set(false);
            });
        }
    });
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(when(expired, || {
            notice_card()
                .child(
                    text(t(keys::AUTH_SESSION_EXPIRED_BANNER))
                        .text_sm()
                        .flex_1()
                        .text_color(move || theme().foreground),
                )
                .child(button_with(
                    t(keys::AUTH_SIGN_IN),
                    ButtonVariant::Primary,
                    ButtonSize::Small,
                    false,
                    None,
                    || nav::push(Route::Login),
                ))
        }))
        .child(when(show_note, || {
            notice_card().child(
                text(t(keys::AUTH_SIGN_OUT_UNSYNCED_NOTE))
                    .text_sm()
                    .text_color(tokens::foreground_secondary),
            )
        }))
}
