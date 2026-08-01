pub mod controller;
pub mod validator;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};

use crate::{config, middlewares::auth_guard, AppState};

pub fn routes() -> Router<AppState> {
    let admin = Router::<AppState>::new()
        .route("/plan/list", post(controller::admin_list_plans))
        .route("/plan/create", post(controller::admin_create_plan))
        .route(
            "/plan/update/{plan_id}",
            post(controller::admin_update_plan),
        )
        .route(
            "/plan/delete/{plan_id}",
            post(controller::admin_delete_plan),
        )
        .route(
            "/subscription/list",
            post(controller::admin_list_subscriptions),
        )
        .route(
            "/subscription/cancel/{subscription_id}",
            post(controller::admin_cancel_subscription),
        )
        .route("/payment/list", post(controller::admin_list_payments))
        .route("/invoice/list", post(controller::admin_list_invoices))
        .route(
            "/discount/list",
            post(controller::admin_list_discount_codes),
        )
        .route(
            "/discount/create",
            post(controller::admin_create_discount_code),
        )
        .route(
            "/discount/delete/{code_id}",
            post(controller::admin_delete_discount_code),
        )
        .route(
            "/post/access/{post_id}",
            post(controller::admin_set_post_access),
        )
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_ADMIN }>,
        ));

    let authenticated = Router::<AppState>::new()
        .route("/checkout", post(controller::create_checkout))
        .route("/checkout/post", post(controller::create_post_checkout))
        .route("/subscriptions", get(controller::my_subscriptions))
        .route("/payments", get(controller::my_payments))
        .route_layer(middleware::from_fn(auth_guard::authenticated));

    let public = Router::<AppState>::new()
        .route("/plans", get(controller::public_list_plans))
        .route("/access/{post_id}", get(controller::check_post_access))
        .route("/webhook/{provider}", post(controller::webhook_receiver));

    // axum 0.8 has no default body limit; the public webhook receiver needs this cap to avoid CWE-400 memory exhaustion.
    public
        .merge(authenticated)
        .merge(admin)
        .layer(DefaultBodyLimit::max(config::body_limits::DEFAULT))
}
