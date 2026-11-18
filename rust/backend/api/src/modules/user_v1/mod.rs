pub mod controller;
pub mod validator;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let base = Router::<AppState>::new()
        .route("/update", post(controller::update_profile))
        .route("/email/confirm", post(controller::confirm_email_change))
        .route_layer(middleware::from_fn(auth_guard::verified))
        .merge(
            Router::<AppState>::new()
                .route("/get", get(controller::get_profile))
                .route("/identities", get(controller::list_identities))
                .route("/identities/unlink", post(controller::unlink_identity))
                .route("/age/declare", post(controller::declare_age))
                .route_layer(middleware::from_fn(auth_guard::authenticated)),
        );

    let admin = Router::<AppState>::new()
        .route("/list", post(controller::admin_list))
        .route("/view/{user_id}", post(controller::admin_view))
        .route("/create", post(controller::admin_create))
        .route("/update/{user_id}", post(controller::admin_update))
        .route("/delete/{user_id}", post(controller::admin_delete))
        .route(
            "/data_request/open",
            post(controller::admin_open_data_request),
        )
        .route(
            "/data_request/list",
            post(controller::admin_list_data_requests),
        )
        .route(
            "/change_password/{user_id}",
            post(controller::admin_change_password),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    base.nest("/admin", admin)
}

/// Erasure routes, nested at the same prefix as [`routes`] but kept outside the
/// router-wide request timeout: `data_request::erase` walks every media object
/// through storage delete + CDN purge, so its length scales with the subject.
pub fn erase_routes() -> Router<AppState> {
    let own = Router::<AppState>::new()
        .route("/delete", post(controller::self_delete_account))
        .route_layer(middleware::from_fn(auth_guard::authenticated));

    let admin = Router::<AppState>::new()
        .route(
            "/data_request/erase/{request_id}",
            post(controller::admin_erase_data_request),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    own.nest("/admin", admin)
}
