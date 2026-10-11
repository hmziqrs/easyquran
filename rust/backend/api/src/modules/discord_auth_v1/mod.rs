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
    let link = Router::<AppState>::new()
        .route("/link", axum::routing::post(controller::discord_link))
        .route_layer(middleware::from_fn(auth_guard::authenticated));

    Router::new()
        .route("/login", get(controller::discord_login))
        .route("/callback", get(controller::discord_callback))
        .route("/exchange", post(controller::discord_exchange))
        .route("/token", post(controller::discord_token))
        .route("/user", get(controller::discord_user_info))
        .merge(link)
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
