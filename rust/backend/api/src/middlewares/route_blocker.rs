use crate::error::RouteBlockerError;
use crate::services::route_blocker_service::RouteBlockerService;
use crate::state::AppState;
use axum::{
    extract::{MatchedPath, Request},
    response::{IntoResponse, Response},
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::{debug, error, warn};

#[derive(Clone)]
pub struct RouteBlockerLayer {
    state: AppState,
}

impl RouteBlockerLayer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

impl<S> Layer<S> for RouteBlockerLayer {
    type Service = RouteBlockerMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RouteBlockerMiddleware {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RouteBlockerMiddleware<S> {
    inner: S,
    state: AppState,
}

impl<S> Service<Request> for RouteBlockerMiddleware<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let state = self.state.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let path = req.uri().path().to_string();

            // /healthz is the compose healthcheck target: exempt so a DB hiccup
            // cannot trigger a restart loop. Health shares no route-blocker state.
            if path == "/healthz" {
                return inner.call(req).await;
            }

            let matched_pattern = req
                .extensions()
                .get::<MatchedPath>()
                .map(|matched| matched.as_str().to_string());
            let pattern = matched_pattern.clone().unwrap_or_else(|| path.clone());

            // ONE production gate (RUST_ENV -> NODE_ENV -> APP_ENV). The blocker
            // is enabled in production; fail-closed on an unset/unknown env.
            let is_prod = crate::config::settings::is_production().unwrap_or(true);
            if !is_prod {
                debug!(path, "Route blocker disabled in non-production mode");
                return inner.call(req).await;
            }

            debug!(path = %path, pattern = %pattern, "Route blocker evaluating request");

            // Route-pattern recording stays best-effort (snapshot-gated in the
            // service: one DB write per pattern, not per request).
            if matched_pattern.is_some() {
                if let Err(err) =
                    RouteBlockerService::record_route_pattern(&state.sea_db, &pattern).await
                {
                    error!(
                        pattern = %pattern,
                        error = %err,
                        "Failed to record route pattern in cache"
                    );
                }
            }

            // Fail-closed for matched private routes: only Ok(false) proceeds.
            // Answered from the in-memory snapshot (5s TTL refresh).
            match RouteBlockerService::is_route_blocked(&state.sea_db, &pattern).await {
                Ok(true) => {
                    warn!(path = %path, pattern = %pattern, "Route blocked by dynamic route_blocker middleware");
                    let error_response: Response =
                        RouteBlockerError::Blocked { path }.into_response();
                    return Ok(error_response);
                }
                Ok(false) => {
                    debug!(path = %path, pattern = %pattern, "Route allowed");
                }
                Err(e) => {
                    error!(error = %e, path = %path, pattern = %pattern, "Failed to check route status");
                    let error_response: Response =
                        RouteBlockerError::CheckFailed(e.to_string()).into_response();
                    return Ok(error_response);
                }
            }

            inner.call(req).await
        })
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::config::{
        HttpSettings, ObjectStorageConfig, OptimizerConfig, QuranSettings, RateLimitSettings,
        Settings, SiteSettings,
    };
    use crate::quran::load_quran_store;
    use crate::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};
    use crate::services::mail::{router::MailRouterLimits, MailRouter};
    use crate::services::session_store::SqliteSessionStore;
    use crate::state::{build_http_client, AppState, QuranRuntimeMetrics, StorageState};
    use axum::routing::get;
    use axum::Router;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::config::settings::TEST_ENV_MUTEX.lock().unwrap()
    }

    fn quran_settings() -> QuranSettings {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran");
        QuranSettings {
            uthmani_path: format!("{base}/arabic/quran-uthmani.sqlite"),
            simple_clean_path: format!("{base}/arabic/quran-simple-clean.sqlite"),
            metadata_xml_path: format!("{base}/quran-data.xml"),
            translations_dir: format!("{base}/translations"),
            max_resident_translations: 1,
            max_resident_bytes: 1024,
            translation_idle_ttl_secs: 1,
        }
    }

    async fn state_with_broken_db() -> AppState {
        let quran = Arc::new(
            load_quran_store(&quran_settings())
                .await
                .expect("quran store loads"),
        );
        // In-memory DB with NO route_status table → is_route_blocked errors →
        // fail-closed CheckFailed for every non-health private route.
        let sea_db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let gate_store = Arc::new(rux_request_gate::InMemoryStore::default());
        let session_store = Arc::new(SqliteSessionStore::new(sea_db.clone()).await);
        let revoked_sessions = Arc::new(Mutex::new(HashSet::new()));
        let mailer = Arc::new(MailRouter::new(
            HashMap::new(),
            String::new(),
            gate_store.clone(),
            sea_db.clone(),
            MailRouterLimits::default(),
            false,
        ));
        let settings = Arc::new(Settings {
            cookie_key: "test_cookie_key_padded_to_more_than_32_bytes_for_tests".into(),
            http: HttpSettings {
                host: "0.0.0.0".into(),
                port: "0".into(),
                ip_source: axum_client_ip::ClientIpSource::ConnectInfo,
                cookie_secure: false,
                trusted_proxy_cidrs: Vec::new(),
            },
            site: SiteSettings {
                url: "http://localhost".into(),
                name: "test".into(),
                consumer_site_url: "http://localhost".into(),
            },
            object_storage: ObjectStorageConfig {
                region: "auto".into(),
                account_id: "test".into(),
                bucket: "test".into(),
                access_key: "test".into(),
                secret_key: "test".into(),
                public_url: "http://localhost".into(),
                endpoint: "http://localhost.invalid".into(),
            },
            optimizer: OptimizerConfig {
                enabled: false,
                max_pixels: 1,
                keep_original: false,
                default_webp_quality: 80,
            },
            rate_limit: RateLimitSettings {
                active_ban_max: 2_000,
                ban_export_token: String::new(),
                internal_token: String::new(),
                internal_requests_per_minute: 600,
                health_requests_per_minute: 120,
            },
            quran: quran_settings(),
        });
        let billing_router = Arc::new(BillingRouter::new(
            HashMap::new(),
            GeoRouter::new(GeoRulesConfig {
                default_provider: String::new(),
                rules: vec![],
            }),
        ));
        AppState {
            sea_db,
            gate_store,
            session_store,
            revoked_sessions,
            mailer,
            settings,
            allowed_origins: crate::utils::cors::build_allowed_origins(false, None, None, None)
                .expect("dev default origins parse"),
            storage: StorageState {
                config: ObjectStorageConfig {
                    region: "auto".into(),
                    account_id: "test".into(),
                    bucket: "test".into(),
                    access_key: "test".into(),
                    secret_key: "test".into(),
                    public_url: "http://localhost".into(),
                    endpoint: "http://localhost.invalid".into(),
                },
                client: aws_sdk_s3::Client::new(&aws_config::SdkConfig::builder().build()),
                optimizer: OptimizerConfig {
                    enabled: false,
                    max_pixels: 1,
                    keep_original: false,
                    default_webp_quality: 80,
                },
                image_moderator: None,
            },
            secret_key: b"test_secret_key".to_vec(),
            http_client: build_http_client(),
            billing_router,
            fcm: None,
            webauthn: None,
            quran,
            quran_runtime_metrics: QuranRuntimeMetrics {
                arabic_load_duration_ms: 1,
                translation_catalogue_load_duration_ms: 1,
                translation_catalogue_entries: 0,
            },
            quran_scripts: Arc::new(tokio::sync::Mutex::new(None)),
            translation_pool: Arc::new(dummy_pool().await),
            quran_sources: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn dummy_pool() -> crate::quran::TranslationPool {
        let qs = quran_settings();
        let catalogue_path = format!("{}/index.min.json", qs.translations_dir);
        let cat = crate::quran::load_catalogue(&catalogue_path)
            .await
            .expect("translation catalogue loads");
        crate::quran::TranslationPool::new(
            &cat,
            std::path::PathBuf::from(&qs.translations_dir),
            1,
            1024,
            std::time::Duration::from_secs(1),
            false,
            0,
        )
    }

    fn app(state: AppState) -> Router {
        Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route("/admin/route/v1/x", get(|| async { "ok" }))
            .layer(RouteBlockerLayer::new(state))
    }

    #[tokio::test]
    async fn healthz_exempt_while_db_errors_fail_closed() {
        let _g = env_lock();
        // RUST_ENV=production so the blocker is ENABLED, then prove /healthz
        // still succeeds while a non-health private route fails closed (503).
        let prev = std::env::var("RUST_ENV").ok();
        std::env::set_var("RUST_ENV", "production");

        let state = state_with_broken_db().await;
        let app = app(state);

        // Non-health private route: no route_status table → CheckFailed → 503.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/route/v1/x")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "non-health private route must fail closed on DB error"
        );

        // /healthz on the SAME broken DB: exempt → handler reached → 200.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            axum::http::StatusCode::OK,
            "/healthz must stay healthy while route-blocker DB errors fail closed"
        );

        match prev {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
    }

    #[tokio::test]
    async fn blocker_disabled_in_non_production() {
        let _g = env_lock();
        let prev = std::env::var("RUST_ENV").ok();
        std::env::set_var("RUST_ENV", "development");

        let state = state_with_broken_db().await;
        let app = app(state);
        // Blocker disabled → non-health route reaches handler despite broken DB.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/admin/route/v1/x")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        match prev {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
    }
}
