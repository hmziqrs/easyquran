pub mod controller;
pub mod service;
pub mod validator;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

use crate::{config, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(controller::github_login))
        .route("/callback", get(controller::github_callback))
        .route("/exchange", post(controller::github_exchange))
        .route("/token", post(controller::github_token))
        .route("/user", get(controller::github_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
