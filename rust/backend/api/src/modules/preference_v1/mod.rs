pub mod controller;
pub mod validator;

use axum::{middleware, routing::post, Router};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/sync", post(controller::sync))
        .route_layer(middleware::from_fn(auth_guard::verified))
}
