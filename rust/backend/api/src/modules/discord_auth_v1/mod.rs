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
        .route("/login", get(controller::discord_login))
        .route("/callback", get(controller::discord_callback))
        .route("/exchange", post(controller::discord_exchange))
        .route("/token", post(controller::discord_token))
        .route("/user", get(controller::discord_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
