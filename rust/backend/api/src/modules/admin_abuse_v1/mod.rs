pub mod controller;

use axum::{middleware, routing::get, Router};

use crate::{middlewares::auth_guard, AppState};

// Session admin ACL only (ROLE_ADMIN), the same guard as /admin/bans. Merged
// with absolute paths so the operator URL is exactly /admin/abuse/v1/summary.
// The public URL is /api/admin/abuse/v1/summary: the edge strips /api.
pub fn routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/admin/abuse/v1/summary", get(controller::get_summary))
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ))
}
