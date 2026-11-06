use axum::{routing::post, Router};

use crate::AppState;

pub mod controller;
pub mod validator;

/// Public incident intake (A24). CSRF applies from the global stack; no login needed.
pub fn routes() -> Router<AppState> {
    Router::<AppState>::new().route("/submit", post(controller::submit))
}
