pub mod controller;
pub mod service;
pub mod validator;

use axum::{extract::DefaultBodyLimit, routing::get, Router};

use crate::{config, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(controller::google_login))
        .route("/callback", get(controller::google_callback))
        .route(
            "/exchange",
            axum::routing::post(controller::google_exchange),
        )
        .route(
            "/token/nonce",
            axum::routing::post(controller::google_token_nonce),
        )
        .route("/token", axum::routing::post(controller::google_token))
        .route("/user", get(controller::google_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
