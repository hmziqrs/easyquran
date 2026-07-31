pub mod controller;
pub mod validator;

use axum::{middleware, routing::post, Router};

use crate::{middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let mut public = Router::<AppState>::new();

    {
        public = public
            .route("/log_in", post(controller::log_in))
            .route("/login/totp", post(controller::login_totp));
    }

    {
        public = public.route("/register", post(controller::register));
    }

    let public = public.route_layer(middleware::from_fn(auth_guard::unauthenticated));

    let mut authenticated = Router::<AppState>::new().route("/log_out", post(controller::log_out));

    {
        authenticated = authenticated
            .route("/2fa/setup", post(controller::twofa_setup))
            .route("/2fa/verify", post(controller::twofa_verify))
            .route("/2fa/disable", post(controller::twofa_disable));
    }

    let authenticated = authenticated
        .route("/sessions/list", post(controller::sessions_list))
        .route(
            "/sessions/terminate/{id}",
            post(controller::sessions_terminate),
        )
        .route_layer(middleware::from_fn(auth_guard::authenticated));

    public.merge(authenticated)
}
