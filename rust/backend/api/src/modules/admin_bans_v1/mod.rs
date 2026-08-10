pub mod controller;
pub mod dto;

use axum::{middleware, routing::get, Router};

use crate::{middlewares::auth_guard, AppState};

// Routes use absolute paths (merged, not nested) so the spec's exact operator
// URLs are honored verbatim: axum 0.8 `nest("/admin/bans", inner)` with an inner
// `/` route would realize the collection at `/admin/bans/` (trailing slash),
// which a machine/proxy client or the spec's URL surface would not match.
//
// Two distinct auth layers:
//   - list + delete: session admin ACL (ROLE_ADMIN). DELETE is therefore also
//     CSRF-gated; the export token is NEVER accepted here.
//   - export: session admin ACL OR read-only BAN_EXPORT_TOKEN (constant-time).
pub fn routes() -> Router<AppState> {
    let session_admin = Router::<AppState>::new()
        .route(
            "/admin/bans",
            get(controller::list_bans).delete(controller::delete_ban),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    let token_or_admin = Router::<AppState>::new()
        .route("/admin/bans/export", get(controller::export_bans))
        .route_layer(middleware::from_fn(controller::admin_or_export_token));

    Router::<AppState>::new()
        .merge(session_admin)
        .merge(token_or_admin)
}
