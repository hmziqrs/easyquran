pub mod controller;
pub mod dto;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{middlewares::auth_guard, AppState};

/// Account ban contract (A19). Mounted by router.rs only when
/// `controller::writes_enabled()` is true; default off.
pub fn routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/", get(controller::list_bans).post(controller::create_ban))
        .route("/{id}/revoke", post(controller::revoke_ban))
        .route("/{id}/renew", post(controller::renew_ban))
        .route("/{id}/notice/resend", post(controller::resend_notice))
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ))
}
