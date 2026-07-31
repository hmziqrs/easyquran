pub mod controller;
pub mod validator;

use crate::middlewares::auth_guard;
use crate::AppState;
use axum::{middleware, routing::post, Router};

// Seed routes mutate core tables — must stay feature-gated (`seed-system`,
// excluded from `full`) AND behind ROLE_SUPER_ADMIN. Do not relax either gate.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/seed_tags", post(controller::seed_tags))
        .route("/seed_categories", post(controller::seed_categories))
        .route("/seed_posts", post(controller::seed_posts))
        .route("/seed_post_comments", post(controller::seed_post_comments))
        .route("/seed_user_sessions", post(controller::seed_user_sessions))
        .route(
            "/seed_email_verifications",
            post(controller::seed_email_verifications),
        )
        .route(
            "/seed_forgot_passwords",
            post(controller::seed_forgot_passwords),
        )
        .route(
            "/seed_post_revisions",
            post(controller::seed_post_revisions),
        )
        .route("/seed_post_series", post(controller::seed_post_series))
        .route("/seed_post_views", post(controller::seed_post_views))
        .route(
            "/seed_scheduled_posts",
            post(controller::seed_scheduled_posts),
        )
        .route("/seed_media", post(controller::seed_media))
        .route(
            "/seed_media_variants",
            post(controller::seed_media_variants),
        )
        .route("/seed_media_usage", post(controller::seed_media_usage))
        .route("/seed_comment_flags", post(controller::seed_comment_flags))
        .route(
            "/seed_newsletter_subscribers",
            post(controller::seed_newsletter_subscribers),
        )
        .route("/seed_route_status", post(controller::seed_route_status))
        .route("/seed", post(controller::seed))
        .route("/presets", post(controller::list_presets))
        .route_layer(middleware::from_fn(
            auth_guard::verified_with_role::<{ auth_guard::ROLE_SUPER_ADMIN }>,
        ))
}
