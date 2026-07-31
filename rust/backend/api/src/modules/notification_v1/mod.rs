pub mod controller;
pub mod validator;

use axum::{middleware, routing::post, Router};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let admin = Router::<AppState>::new()
        .route("/create", post(controller::admin_create))
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    let authenticated = Router::<AppState>::new()
        .route("/list", post(controller::list))
        .route("/unread_count", post(controller::unread_count))
        .route("/mark_read", post(controller::mark_read))
        .route("/mark_all_read", post(controller::mark_all_read))
        .route_layer(middleware::from_fn(auth_guard::authenticated));

    admin.merge(authenticated)
}
