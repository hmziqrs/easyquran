pub mod cache;
pub mod controller;
pub mod cors;
pub mod dto;
pub mod error;

use axum::{routing::get, Router};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    let r = Router::<AppState>::new()
        .route("/surahs", get(controller::list_surahs))
        .route("/surahs/{surah}", get(controller::get_surah))
        .route("/surahs/{surah}/ayahs", get(controller::surah_ayahs))
        .route(
            "/sources/{sourceId}/surah/{surah}",
            get(controller::source_surah),
        )
        .route("/sources/{sourceId}/range", get(controller::source_range))
        .route("/sources", get(controller::sources))
        .route("/ayahs", get(controller::ayahs_multi))
        .route("/ayahs/{verseKey}", get(controller::ayah_key_redirect))
        .route("/ayahs/{surah}/{ayah}", get(controller::get_ayah))
        .route("/juzs", get(controller::list_juzs))
        .route("/juzs/{juz}", get(controller::get_juz))
        .route("/juzs/{juz}/ayahs", get(controller::juz_ayahs))
        .route("/pages", get(controller::list_pages))
        .route("/pages/{page}", get(controller::get_page))
        .route("/pages/{page}/ayahs", get(controller::page_ayahs))
        .route("/rukus", get(controller::list_rukus))
        .route("/rukus/{ruku}", get(controller::get_ruku))
        .route("/rukus/{ruku}/ayahs", get(controller::ruku_ayahs))
        .route("/hizb-quarters", get(controller::list_hizb_quarters))
        .route(
            "/hizb-quarters/{quarter}",
            get(controller::get_hizb_quarter),
        )
        .route(
            "/hizb-quarters/{quarter}/ayahs",
            get(controller::hizb_quarter_ayahs),
        )
        .route("/manzils", get(controller::list_manzils))
        .route("/manzils/{manzil}", get(controller::get_manzil))
        .route("/manzils/{manzil}/ayahs", get(controller::manzil_ayahs))
        .route("/sajdas", get(controller::list_sajdas))
        .route("/sajdas/{sajda}", get(controller::get_sajda))
        .route("/scripts", get(controller::scripts))
        .route("/random", get(controller::random))
        .route("/health/ready", get(controller::health_ready));
    #[cfg(feature = "openapi")]
    let r = r.route("/openapi.json", get(controller::openapi_json));
    r
}

/// Kept separate so search gets a tighter rate limit than `routes()`; inlining lets the CPU-heavy substring scan inherit the coarse limit.
pub fn search_route() -> Router<AppState> {
    Router::<AppState>::new().route("/search", get(controller::search))
}
