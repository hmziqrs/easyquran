pub mod controller;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{middlewares::auth_guard, AppState};

/// Operator review of incident reports (A24) and enforcement appeals (A35). Session admin ACL
/// only. Merged with absolute paths, like the other admin modules. Status changes and appeal
/// decisions are audited (`report.status_set`, `appeal.decision`).
pub fn routes() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/admin/reports/v1", get(controller::list_reports))
        .route(
            "/admin/reports/v1/{id}/status",
            post(controller::set_report_status),
        )
        .route("/admin/appeals/v1", get(controller::list_appeals))
        .route(
            "/admin/appeals/v1/{id}/decision",
            post(controller::decide_appeal),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ))
}
