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
        .route("/login", get(controller::facebook_login))
        .route("/callback", get(controller::facebook_callback))
        .route("/exchange", post(controller::facebook_exchange))
        .route("/token", post(controller::facebook_token))
        .route("/user", get(controller::facebook_user_info))
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
