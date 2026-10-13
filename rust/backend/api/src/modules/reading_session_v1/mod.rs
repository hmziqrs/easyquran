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
        .route("/sync", post(controller::sync))
        .route("/archive", get(controller::archive))
        .route_layer(middleware::from_fn(auth_guard::verified))
}
