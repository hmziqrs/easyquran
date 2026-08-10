pub mod controller;
pub mod validator;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};

use crate::{config, middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let admin = Router::<AppState>::new()
        .route(
            "/suppression",
            get(controller::list_suppressions)
                .post(controller::create_suppression)
                .delete(controller::delete_suppression),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    let public = Router::<AppState>::new().route(
        "/webhook/{provider}",
        post(controller::mail_webhook_receiver),
    );

    admin
        .merge(public)
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
