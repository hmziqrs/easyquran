pub mod controller;
pub mod service;
pub mod validator;

use axum::{extract::DefaultBodyLimit, middleware, routing::get, Router};

use crate::{config, middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let link = Router::<AppState>::new()
        .route("/link", axum::routing::post(controller::google_link))
        .route_layer(middleware::from_fn(auth_guard::authenticated));

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
        .merge(link)
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
