pub mod controller;
pub mod service;
pub mod validator;

use axum::{
    routing::{get, post},
    Router,
};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(controller::github_login))
        .route("/callback", get(controller::github_callback))
        .route("/exchange", post(controller::github_exchange))
        .route("/user", get(controller::github_user_info))
}
