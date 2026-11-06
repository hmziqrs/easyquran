pub mod controller;
pub mod dto;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{middlewares::auth_guard, AppState};

/// Legal holds (A25) for operators: list, place, renew. Session admin ACL only. Merged with
/// absolute paths, like the other admin modules. Every write is audited by `services::legal_hold`.
pub fn routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route(
            "/admin/legal-holds/v1",
            get(controller::list_holds).post(controller::place_hold),
        )
        .route(
            "/admin/legal-holds/v1/{id}/renew",
            post(controller::renew_hold),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ))
}
