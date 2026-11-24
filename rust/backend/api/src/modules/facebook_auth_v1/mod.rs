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
        .route("/token/link", post(controller::facebook_token_link))
        .route_layer(middleware::from_fn(auth_guard::authenticated));
    Router::new()
        .route("/login", get(controller::facebook_login))
        .route("/callback", get(controller::facebook_callback))
        .route("/exchange", post(controller::facebook_exchange))
        .route("/token", post(controller::facebook_token))
        .merge(linked)
        .route(
            "/native/relay",
            get(crate::services::oauth::native_relay::facebook_relay),
        )
        .route("/user", get(controller::facebook_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
