pub mod controller;
pub mod validator;

use axum::{middleware, routing::post, Router};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let base = Router::<AppState>::new()
        .route("/verify", post(controller::verify))
        .route("/resend", post(controller::resend))
        .route_layer(middleware::from_fn(auth_guard::unverified));

    let admin = Router::<AppState>::new()
        .route("/admin/list", post(controller::admin_list))
        .route("/admin/delete", post(controller::admin_delete))
        .route("/admin/issue_code", post(controller::admin_issue_code))
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    base.merge(admin)
}
