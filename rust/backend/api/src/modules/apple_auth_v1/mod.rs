pub mod controller;
pub mod service;
pub mod validator;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};

use crate::{config, middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let linked = Router::new()
        .route("/token/link", post(controller::apple_token_link))
        .route_layer(middleware::from_fn(auth_guard::authenticated));
    Router::new()
        .route("/login", get(controller::apple_login))
        .route("/callback", get(controller::apple_callback))
        .route("/exchange", post(controller::apple_exchange))
        .route("/token/nonce", post(controller::apple_token_nonce))
        .route("/token", post(controller::apple_token))
        .merge(linked)
        .route(
            "/native/relay",
            post(crate::services::oauth::native_relay::apple_relay),
        )
        .route("/user", get(controller::apple_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
