//! The app's central error surface: one error type and one view any page
//! can raise into. The root mounts [`error_surface`]; anything calls
//! [`report_error`].

use nsh_base::theme;
use nsh_core::element::{AccessibilityRole, SharedString, Signal};
use nsh_core::prelude::*;

use crate::state::event_bus;
use crate::ui::button::{ButtonSize, ButtonVariant, button_with};

/// The most queued errors; older ones drop as new ones arrive.
pub const ERROR_QUEUE_MAX: usize = 5;

/// The app's error: a stable `code` callers branch on, a `message` the
/// surface shows, and an optional `detail` with what only diagnostics
/// needs.
#[derive(Clone, Debug, PartialEq)]
pub struct AppError {
    /// Stable machine name: `"network"`, `"storage"`, `"invalid"`,
    /// `"unavailable"`, `"internal"`, or a feature's `"feed.offline"`.
    pub code: SharedString,
    /// One sentence on what happened and, when useful, how to recover.
    pub message: SharedString,
    /// The technical part, for logs and diagnostics.
    pub detail: Option<SharedString>,
    /// When it was raised (`foundation::time::now_ms`).
    pub at_ms: f64,
    /// Ids it within the queue ([`dismiss_error`]).
    pub id: u64,
}

impl AppError {
    /// An error of `code` and `message`, raised now.
    pub fn new(code: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
            at_ms: crate::foundation::time::now_ms(),
            id: crate::foundation::id::next_id(),
        }
    }

    /// Attaches the technical `detail`.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::new("internal", message)
    }
}

impl From<nsh_core::storage::StorageFull> for AppError {
    fn from(full: nsh_core::storage::StorageFull) -> Self {
        Self::new("storage", "The app's storage is full").detail(full.to_string())
    }
}

/// The queued errors, oldest first. One per surface: `global` keeps one
/// value per type.
fn queue() -> Signal<Vec<AppError>> {
    global::<Vec<AppError>>()
}

/// Raises `error` into the central surface (the root's [`error_surface`]
/// shows it), into the monitoring sink, and onto the bus (`app.error`).
pub fn report_error(error: impl Into<AppError>) {
    let error = error.into();
    crate::foundation::logging::log_error(&format!("{error}"));
    crate::services::monitoring::sink().record_error(&error);
    event_bus::publish(
        event_bus::TOPIC_APP_ERROR,
        format!("{} ({})", error.message, error.code),
    );
    queue().update(|queue| {
        queue.push(error);
        if queue.len() > ERROR_QUEUE_MAX {
            queue.remove(0);
        }
    });
}

/// Removes the queued error with this id (the surface's Dismiss).
pub fn dismiss_error(id: u64) {
    queue().update(|queue| queue.retain(|error| error.id != id));
}

/// Removes every queued error.
pub fn clear_errors() {
    queue().update(|queue| queue.clear());
}

/// The queued errors, oldest first (tests and diagnostics read this).
pub fn errors() -> Signal<Vec<AppError>> {
    queue()
}

/// The central error surface: a bottom overlay showing the newest queued
/// error, with any older ones counted behind it. The root mounts it once,
/// above the rest of the app; a page never draws its own error banner.
/// Every part of the banner reads the queue, so a newer error replaces it
/// without the region being rebuilt.
pub fn error_surface() -> impl IntoElement {
    let errors = queue();
    view().absolute().bottom_0().left_0().right_0().child(when(
        move || errors.with(|queue| !queue.is_empty()),
        move || {
            view().px_4().pb_4().child(
                view()
                    .role(AccessibilityRole::Alert)
                    .w_full()
                    .flex_row()
                    .items_start()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(move || theme().destructive)
                    .bg(move || theme().card)
                    .shadow_sm()
                    .child(
                        view()
                            .flex_1()
                            .gap_1()
                            .child(
                                text(move || {
                                    errors.with(|queue| {
                                        queue.last().map(|e| e.message.clone()).unwrap_or_default()
                                    })
                                })
                                .text_sm()
                                .text_color(move || theme().foreground),
                            )
                            .child(
                                text(move || {
                                    errors.with(|queue| match queue.last() {
                                        None => String::new(),
                                        Some(error) => match (&error.detail, queue.len() - 1) {
                                            (Some(detail), _) => {
                                                format!("{} · {}", error.code, detail)
                                            }
                                            (None, 0) => error.code.to_string(),
                                            (None, n) => {
                                                format!("{} · {} earlier", error.code, n)
                                            }
                                        },
                                    })
                                })
                                .text_xs()
                                .text_color(move || theme().muted_foreground),
                            ),
                    )
                    .child(
                        // Top-aligned in the row: the banner's text can wrap
                        // under it.
                        button_with(
                            "Dismiss",
                            ButtonVariant::Secondary,
                            ButtonSize::Small,
                            false,
                            Some("Dismiss"),
                            move || {
                                let id = errors
                                    .with_untracked(|queue| queue.last().map(|error| error.id));
                                if let Some(id) = id {
                                    dismiss_error(id);
                                }
                            },
                        )
                        .self_end(),
                    ),
            )
        },
    ))
}
