//! `/quran/v1` — public Quran content API (§6).
//!
//! Read-only and unauthenticated: served from the immutable in-memory
//! `QuranStore`, no SQLite query per request (§4.1, §10). Wired as a sibling
//! public branch in `main` with wildcard CORS, client-IP-aware rate limiting,
//! and the shared observability stack — but without the private branch's
//! `origin_guard` / `csrf_guard` / `session_layer` / `RouteBlockerLayer`
//! (§8.2).

pub mod cache;
pub mod controller;
pub mod cors;
pub mod dto;
pub mod error;

use axum::{routing::get, Router};

use crate::AppState;

/// All `/quran/v1` routes (§6.1). State is injected when the branch is nested
/// and `with_state`-pinned in `main`. `GET` also covers `HEAD` (axum auto-handles
/// HEAD by running the GET handler and dropping the body — §10).
pub fn routes() -> Router<AppState> {
    let r = Router::<AppState>::new()
        // ── surah metadata ──
        .route("/surahs", get(controller::list_surahs))
        .route("/surahs/{surah}", get(controller::get_surah))
        .route("/surahs/{surah}/ayahs", get(controller::surah_ayahs))
        // ── ayahs ──
        .route("/ayahs", get(controller::ayahs_multi))
        // `/ayahs/2:255` → 308; `/ayahs/abc` → 400 (single-segment alias, §6.1).
        .route("/ayahs/{verseKey}", get(controller::ayah_key_redirect))
        .route("/ayahs/{surah}/{ayah}", get(controller::get_ayah))
        // ── range families (same three-route shape each) ──
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
        .route("/hizb-quarters/{quarter}", get(controller::get_hizb_quarter))
        .route("/hizb-quarters/{quarter}/ayahs", get(controller::hizb_quarter_ayahs))
        .route("/manzils", get(controller::list_manzils))
        .route("/manzils/{manzil}", get(controller::get_manzil))
        .route("/manzils/{manzil}/ayahs", get(controller::manzil_ayahs))
        // ── sajdas ──
        .route("/sajdas", get(controller::list_sajdas))
        .route("/sajdas/{sajda}", get(controller::get_sajda))
        // ── operational / artifact / ayah-of-the-day ──
        .route("/scripts", get(controller::scripts))
        .route("/random", get(controller::random))
        .route("/version", get(controller::version))
        .route("/health/ready", get(controller::health_ready));
    // `/search` is a separate router (see [`search_route`]) so a tighter
    // search-specific rate limit can be layered at the nest site (§8.2).
    // §6.1: serve the OpenAPI document on the PUBLIC branch (feature-gated; the
    // app-wide SwaggerUi at /api/docs is separate and on the private branch).
    #[cfg(feature = "openapi")]
    let r = r.route("/openapi.json", get(controller::openapi_json));
    r
}

/// Just `/search`, split out so the caller can apply the tighter search rate
/// limit (§8.2: "a coarse per-IP limit to all public routes and a tighter one
/// to search") — search is the most CPU-intensive public route (substring scan).
pub fn search_route() -> Router<AppState> {
    Router::<AppState>::new().route("/search", get(controller::search))
}
