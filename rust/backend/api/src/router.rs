use axum::{
    extract::State,
    http::{header, StatusCode},
    middleware,
    routing::{get, post},
    Json, Router,
};
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

use crate::middlewares::{http_metrics, rate_limit, request_id_middleware, security_headers};
use crate::modules::{
    auth_v1, category_v1, csrf_v1, feed_v1, mail_v1, media_v1, post_v1, search_v1, tag_v1, user_v1,
};

use crate::modules::google_auth_v1;

use crate::modules::{email_verification_v1, forgot_password_v1};

use crate::modules::post_comment_v1;

use crate::modules::newsletter_v1;

use crate::modules::analytics_v1;

use crate::modules::admin_acl_v1;

use crate::modules::admin_route_v1;

use crate::modules::admin_bans_v1;

#[cfg(feature = "seed-system")]
use crate::modules::seed_v1;

use crate::modules::billing_v1;

use crate::modules::passkey_v1;
use crate::modules::{apple_auth_v1, facebook_auth_v1, github_auth_v1};
use crate::modules::{device_v1, notification_v1};

use crate::utils::sanitize::xml_escape;

use super::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    let mut router = Router::new()
        .route("/healthz", get(health_check))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/csrf/v1/generate", post(csrf_v1::controller::generate))
        .nest(
            "/auth/v1",
            auth_v1::routes().layer(rate_limit::rate_limit_layer(&state, 100, 60)),
        );

    router = router.nest("/auth/google/v1", google_auth_v1::routes());

    router = router.nest("/user/v1", user_v1::routes());

    router = router
        .nest("/email_verification/v1", email_verification_v1::routes())
        .nest("/forgot_password/v1", forgot_password_v1::routes());

    router = router.nest(
        "/post/v1",
        post_v1::routes().layer(rate_limit::rate_limit_layer(&state, 200, 60)),
    );

    router = router.nest(
        "/post/comment/v1",
        post_comment_v1::routes().layer(rate_limit::rate_limit_layer(&state, 100, 60)),
    );

    router = router
        .nest("/category/v1", category_v1::routes())
        .nest("/tag/v1", tag_v1::routes())
        .nest("/mail/v1", mail_v1::routes())
        .nest(
            "/media/v1",
            media_v1::routes().layer(rate_limit::rate_limit_layer(&state, 30, 60)),
        )
        .nest("/feed/v1", feed_v1::routes())
        .nest(
            "/search/v1",
            search_v1::routes().layer(rate_limit::rate_limit_layer(&state, 30, 60)),
        );

    router = router.nest(
        "/newsletter/v1",
        newsletter_v1::routes().layer(rate_limit::rate_limit_layer(&state, 100, 60)),
    );

    router = router.nest("/analytics/v1", analytics_v1::routes());

    router = router.nest("/admin/route/v1", admin_route_v1::routes());

    router = router.nest("/admin/acl/v1", admin_acl_v1::routes());

    // Merged (not nested) so the module's absolute, spec-exact operator URLs
    // (/admin/bans, /admin/bans/export) are served verbatim. Auth layers and the
    // origin/CSRF/session stack come from the enclosing private router.
    router = router.merge(admin_bans_v1::routes());

    #[cfg(feature = "seed-system")]
    {
        router = router.nest("/admin/seed/v1", seed_v1::routes());
    }

    router = router.nest("/billing/v1", billing_v1::routes());

    router = router
        .nest(
            "/device/v1",
            device_v1::routes().layer(rate_limit::rate_limit_layer(&state, 100, 60)),
        )
        .nest(
            "/notification/v1",
            notification_v1::routes().layer(rate_limit::rate_limit_layer(&state, 100, 60)),
        );

    router = router.nest("/passkey/v1", passkey_v1::routes());

    router = router
        .nest("/auth/facebook/v1", facebook_auth_v1::routes())
        .nest("/auth/github/v1", github_auth_v1::routes())
        .nest("/auth/apple/v1", apple_auth_v1::routes());

    #[cfg(feature = "openapi")]
    {
        use utoipa::OpenApi;
        use utoipa_swagger_ui::SwaggerUi;

        router = router.merge(
            SwaggerUi::new("/api/docs").url("/api/docs.json", crate::docs::ApiDoc::openapi()),
        );
    }

    with_observability(router)
}

pub fn with_observability(router: Router<AppState>) -> Router<AppState> {
    router
        .route_layer(middleware::from_fn(http_metrics::track_metrics))
        .layer(middleware::from_fn(security_headers::security_headers))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(false),
                )
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Millis),
                ),
        )
}

async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let db_status = match state.sea_db.ping().await {
        Ok(()) => "ok",
        Err(_) => "error",
    };

    let healthy = db_status == "ok";
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(serde_json::json!({
            "status": if healthy { "healthy" } else { "degraded" },
            "components": {
                "database": db_status,
            }
        })),
    )
}

async fn robots_txt() -> (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain")],
        "User-agent: *\nAllow: /\nDisallow: /admin/\nDisallow: /api/\nDisallow: /auth/\n\nSitemap: https://ruxlog.com/sitemap.xml\n",
    )
}

async fn sitemap_xml(
    State(state): State<AppState>,
) -> Result<
    (
        StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        String,
    ),
    StatusCode,
> {
    use crate::db::sea_models::post;

    let posts = post::Entity::sitemap(&state.sea_db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let raw_base_url = state.settings.site.consumer_site_url.clone();
    // Slugs are author-controlled and only length-validated: escape before XML interpolation, or it's a stored XML-injection vector.
    let base_url = xml_escape(&raw_base_url);

    let mut urls = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    urls.push_str(&format!(
        "  <url><loc>{}/</loc><changefreq>daily</changefreq><priority>1.0</priority></url>\n",
        base_url
    ));

    for p in &posts {
        let lastmod = p.updated_at.to_rfc3339();
        let slug = xml_escape(&p.slug);
        urls.push_str(&format!(
            "  <url><loc>{}/posts/{}</loc><lastmod>{}</lastmod><changefreq>weekly</changefreq><priority>0.8</priority></url>\n",
            base_url, slug, lastmod
        ));
    }

    urls.push_str("</urlset>");

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/xml")],
        urls,
    ))
}
