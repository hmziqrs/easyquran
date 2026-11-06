pub mod controller;
pub mod dto;

use axum::{routing::post, Router};

use crate::AppState;

/// Appeal intake (A35). Public and CSRF-protected like every unsafe route. Mounted
/// by router.rs only when the ban write flag is on, so it ships dark with the bans.
pub fn routes() -> Router<AppState> {
    Router::<AppState>::new().route("/submit", post(controller::submit))
}
