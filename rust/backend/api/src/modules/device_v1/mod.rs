pub mod challenge;
pub mod controller;
pub mod keys;
pub mod recognise;
pub mod validator;

use axum::{middleware, routing::post, Router};

use crate::{
    middlewares::{auth_guard, rate_limit},
    AppState,
};

/// Register is client-throttled to once per 24 h per device. These ceilings are
/// the server side: 10 per hour per signed-in account, and an IP ceiling ten times
/// wider so one carrier NAT cannot be the binding limit for one user.
pub const REGISTER_USER_MAX: u64 = 10;
pub const REGISTER_IP_MAX: u64 = 100;
pub const REGISTER_WINDOW_SECS: u64 = 3600;

/// A challenge is issued once per register attempt, so the ceiling sits well above
/// the register one. It bounds how fast a session can fill the challenge map.
pub const CHALLENGE_USER_MAX: u64 = 60;
pub const CHALLENGE_IP_MAX: u64 = 600;

pub fn routes(state: &AppState) -> Router<AppState> {
    let register = rate_limit::authenticated_write_limits(
        Router::<AppState>::new().route("/register", post(controller::register)),
        state,
        "device:register",
        REGISTER_USER_MAX,
        REGISTER_IP_MAX,
        REGISTER_WINDOW_SECS,
    );
    let challenge = rate_limit::authenticated_write_limits(
        Router::<AppState>::new().route("/challenge", post(controller::issue_challenge)),
        state,
        "device:challenge",
        CHALLENGE_USER_MAX,
        CHALLENGE_IP_MAX,
        REGISTER_WINDOW_SECS,
    );
    Router::<AppState>::new()
        .merge(register)
        .merge(challenge)
        .route("/list", post(controller::list))
        .route("/rename", post(controller::rename))
        .route("/delete", post(controller::delete))
        .route("/token/delete", post(controller::delete_token))
        .route_layer(middleware::from_fn(auth_guard::verified))
}
