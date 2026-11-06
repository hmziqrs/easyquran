pub mod controller;
pub mod validator;

use axum::{extract::DefaultBodyLimit, middleware, routing::post, Router};

use crate::{config, middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let media_limited = Router::<AppState>::new()
        .route("/create", post(controller::create))
        .layer(DefaultBodyLimit::max(config::body_limits::MEDIA));

    let author = Router::<AppState>::new()
        .route("/view/{media_id}", post(controller::view))
        .route("/list/query", post(controller::find_with_query))
        .route("/usage/details", post(controller::list_usage_details))
        .merge(media_limited)
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_AUTHOR }>,
        ));

    // Delete is Admin-only (A28): a Moderator keeps read, not removal. Removing a
    // media row also removes its R2 objects and variants, so it is not an Author action.
    let admin = Router::<AppState>::new()
        .route("/delete/{media_id}", post(controller::delete))
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    author.merge(admin)
}
