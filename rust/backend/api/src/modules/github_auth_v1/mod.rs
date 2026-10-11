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
    // /link is user-initiated account linking: gate it with the standard
    // authenticated guard so a missing session answers 401 AUTH_UNAUTHENTICATED
    // like every other authenticated route (the handler keeps its own
    // belt-and-braces user check).
    let link = Router::<AppState>::new()
        .route("/link", axum::routing::post(controller::github_link))
        .route_layer(middleware::from_fn(auth_guard::authenticated));

    Router::new()
        .route("/login", get(controller::github_login))
        .route("/callback", get(controller::github_callback))
        .route("/exchange", post(controller::github_exchange))
        .route("/token", post(controller::github_token))
        .route("/user", get(controller::github_user_info))
        .merge(link)
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
