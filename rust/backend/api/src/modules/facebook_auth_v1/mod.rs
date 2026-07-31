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
        .route("/login", get(controller::facebook_login))
        .route("/callback", get(controller::facebook_callback))
        .route("/exchange", post(controller::facebook_exchange))
        .route("/user", get(controller::facebook_user_info))
}
