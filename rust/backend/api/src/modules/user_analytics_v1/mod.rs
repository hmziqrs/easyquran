pub mod controller;
pub mod validator;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/events", post(controller::events))
        .route("/summary", get(controller::summary))
        .route_layer(middleware::from_fn(auth_guard::verified))
}
