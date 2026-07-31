pub mod controller;
pub mod validator;

use axum::{middleware, routing::post, Router};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let verified = Router::<AppState>::new()
        .route("/register/begin", post(controller::register_begin))
        .route("/register/finish", post(controller::register_finish))
        .route("/list", post(controller::list))
        .route("/remove", post(controller::remove))
        .route_layer(middleware::from_fn(auth_guard::verified));

    let unauthenticated = Router::<AppState>::new()
        .route("/login/begin", post(controller::login_begin))
        .route("/login/finish", post(controller::login_finish))
        .route_layer(middleware::from_fn(auth_guard::unauthenticated));

    verified.merge(unauthenticated)
}
