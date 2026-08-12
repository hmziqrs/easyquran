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
        .route("/login", get(controller::apple_login))
        .route("/callback", get(controller::apple_callback))
        .route("/exchange", post(controller::apple_exchange))
        .route("/token/nonce", post(controller::apple_token_nonce))
        .route("/token", post(controller::apple_token))
        .route("/user", get(controller::apple_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
