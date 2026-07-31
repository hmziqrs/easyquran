pub mod controller;
pub mod service;
pub mod validator;

use axum::{
    routing::{get, post},
    Router,
};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(controller::apple_login))
        .route("/callback", get(controller::apple_callback))
        .route("/exchange", post(controller::apple_exchange))
        .route("/user", get(controller::apple_user_info))
}
