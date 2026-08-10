use axum::extract::State;
use axum::{http::HeaderName, middleware};
use axum_extra::extract::cookie::SameSite;
use std::{env, net::SocketAddr, sync::Arc, time::Duration};
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
};
use tower_sessions::{cookie::Key, Expiry, SessionManagerLayer};

use ruxlog::config::env::{env_bool, env_u64, env_with_fallback, parse_env_u64};
use ruxlog::utils::cors::allowed_origins_from_env;
use ruxlog::{
    config::Settings,
    db, middlewares, router,
    services::session_store::SqliteSessionStore,
    state::{AppState, QuranRuntimeMetrics, StorageState},
    utils::telemetry,
};

use ruxlog::services::acl_service::AclService;

use ruxlog::services::{route_blocker_config, route_blocker_service::RouteBlockerService};

use ruxlog::services::billing::BillingProvider;

use ruxlog::services::billing::router::{BillingRouter, GeoRouter, GeoRulesConfig};

async fn build_mail_router(
    db: sea_orm::DatabaseConnection,
    gate_store: std::sync::Arc<rux_request_gate::InMemoryStore>,
    http_client: reqwest::Client,
) -> std::sync::Arc<ruxlog::services::mail::MailRouter> {
    use ruxlog::services::mail::{router::MailRouterLimits, smtp::SmtpMailProvider};
    use ruxlog::services::mail::{MailProvider, MailRouter};
    use std::collections::HashMap;

    let selected = env::var("MAIL_PROVIDER").unwrap_or_else(|_| "smtp".to_string());
    let rate_limit_enabled = env_bool("MAIL_RATE_LIMIT_ENABLED", true);
    let limits = MailRouterLimits {
        dedup_ttl_secs: parse_env_u64("MAIL_DEDUP_TTL_SECS", 300) as usize,
        soft_cooldown_secs: parse_env_u64("MAIL_SOFT_BOUNCE_COOLDOWN_SECS", 86_400) as i64,
        ..MailRouterLimits::default()
    };
    let from_address =
        env::var("MAIL_FROM_ADDRESS").unwrap_or_else(|_| "no-reply@domain.tld".to_string());
    let from_name = env::var("MAIL_FROM_NAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut providers: HashMap<String, std::sync::Arc<dyn MailProvider>> = HashMap::new();
    let default = match selected.as_str() {
        "none" => {
            providers.insert(
                "none".to_string(),
                std::sync::Arc::new(ruxlog::services::mail::none::NoOpMailProvider)
                    as std::sync::Arc<dyn MailProvider>,
            );
            "none"
        }
        "cloudflare" => {
            use ruxlog::services::mail::cloudflare::CloudflareMailProvider;
            use secrecy::SecretString;

            let account_id = env::var("CLOUDFLARE_EMAIL_ACCOUNT_ID")
                .expect("MAIL_PROVIDER=cloudflare requires CLOUDFLARE_EMAIL_ACCOUNT_ID");
            let api_token = SecretString::from(
                env::var("CLOUDFLARE_EMAIL_API_TOKEN")
                    .expect("MAIL_PROVIDER=cloudflare requires CLOUDFLARE_EMAIL_API_TOKEN"),
            );
            let webhook_secret =
                SecretString::from(env::var("CLOUDFLARE_EMAIL_WEBHOOK_SECRET").unwrap_or_default());
            let base_url = env::var("CLOUDFLARE_EMAIL_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.cloudflare.com/client/v4".to_string());
            let allowed = env::var("CLOUDFLARE_EMAIL_ALLOWED_ADDRESSES")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .collect::<Vec<_>>()
                });
            let cf = CloudflareMailProvider::new(
                account_id,
                api_token,
                webhook_secret,
                base_url,
                from_address,
                from_name,
                http_client,
                allowed,
            )
            .expect("failed to build Cloudflare mail provider");
            providers.insert("cloudflare".to_string(), std::sync::Arc::new(cf));
            "cloudflare"
        }
        _ => {
            let transport = ruxlog::services::mail::smtp::create_connection().await;
            let smtp = SmtpMailProvider::new(transport, from_address, from_name);
            providers.insert("smtp".to_string(), std::sync::Arc::new(smtp));
            "smtp"
        }
    };

    tracing::info!(provider = %default, rate_limit_enabled, "Mail router initialized");
    std::sync::Arc::new(MailRouter::new(
        providers,
        default.to_string(),
        gate_store,
        db,
        limits,
        rate_limit_enabled,
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let _telemetry_guard = telemetry::init();

    telemetry::init_pool_metrics();

    let settings = Arc::new(match Settings::from_env() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    });

    // Log the selected external identity source + both isolated policies. The
    // internal token is never logged — only whether it is set.
    tracing::info!(
        ip_source = ?settings.http.ip_source,
        internal_requests_per_minute = settings.rate_limit.internal_requests_per_minute,
        health_requests_per_minute = settings.rate_limit.health_requests_per_minute,
        internal_token_set = !settings.rate_limit.internal_token.is_empty(),
        "Ingress contract configured"
    );

    // Built once at boot from ALLOWED_ORIGINS (+ dev defaults in non-production);
    // shared immutable between CorsLayer and origin_guard. No env read happens per
    // request. Production fails closed without an HTTPS list containing the
    // EasyQuran origin; misconfiguration exits 1 like any other config error.
    let allowed_origins = match allowed_origins_from_env() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(origins = ?allowed_origins.origins(), "Allowed origins resolved");

    // Required boot side-effect: installs the field-encryption key into the process-wide OnceLock the SeaORM model layer reads — deleting it breaks field encryption.
    ruxlog::state::load_field_enc_key();

    let sea_db = db::sea_connect::get_sea_connection().await;

    let gate_store = std::sync::Arc::new(rux_request_gate::InMemoryStore::default());

    // rate_limit_state schema owned by migration m000002 (runs inside
    // get_sea_connection before this point); restore() reads what it created.
    gate_store.restore(ruxlog::services::rate_limit_store::load(&sea_db).await);

    let session_store = Arc::new(SqliteSessionStore::new(sea_db.clone()).await);

    let revoked_sessions: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));

    // W8e: reconcile the durable session-binding table (m000004) at boot. Revokes
    // live user_session audit rows that have no durable binding (pre-binding or
    // orphaned) so the process restart is one clean re-authentication boundary.
    // Gated on WEB_AUTH_ENABLED: until web auth is turned on, existing
    // mobile/backend sessions have no durable binding (the new record path is not
    // wired yet) and must NOT be mass-revoked. Runs only at the web-auth cutover.
    // Non-fatal: a failure logs loudly but does not block boot; the next boot
    // retries. Migrations have already run inside get_sea_connection().
    if ruxlog::config::settings::web_auth().enabled {
        let boot_backend = ruxlog::services::auth::AuthBackend::new(
            &sea_db,
            session_store.clone(),
            revoked_sessions.clone(),
        );
        match boot_backend
            .reconcile_unbound_sessions(&sea_db, session_store.clone())
            .await
        {
            Ok(n) if n > 0 => tracing::info!(
                revoked = n,
                "Startup session-binding reconciliation revoked unbound sessions"
            ),
            Ok(_) => tracing::debug!("Startup session-binding reconciliation: no unbound sessions"),
            Err(e) => tracing::error!(
                error = %e,
                "Session-binding reconciliation failed at boot (non-fatal)"
            ),
        }
    }

    let object_storage = settings.object_storage.clone();

    tracing::debug!(
        bucket = %object_storage.bucket,
        region = %object_storage.region,
        endpoint = %object_storage.endpoint,
        public_url = %object_storage.public_url,
        "Object Storage configured (access_key/secret_key redacted)"
    );
    let s3_config = aws_config::from_env()
        .endpoint_url(&object_storage.endpoint)
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            &object_storage.access_key,
            &object_storage.secret_key,
            None,
            None,
            "S3Compatible",
        ))
        .region(aws_sdk_s3::config::Region::new(
            object_storage.region.clone(),
        ))
        .load()
        .await;

    let s3_client = aws_sdk_s3::Client::new(&s3_config);

    let http_client = ruxlog::state::build_http_client();

    let mailer = build_mail_router(sea_db.clone(), gate_store.clone(), http_client.clone()).await;

    let billing_router: std::sync::Arc<BillingRouter> = {
        use ruxlog::services::billing::{
            airwallex::AirwallexProvider, crypto::CryptoProvider,
            lemon_squeezy::LemonSqueezyProvider, mercado_pago::MercadoPagoProvider,
            paddle::PaddleProvider, paypal::PayPalProvider, polar::PolarProvider,
            razorpay::RazorpayProvider, revolut::RevolutProvider, stripe::StripeProvider,
        };
        let http_client = http_client.clone();

        fn try_init<F>(name: &str, init: F) -> Option<(String, std::sync::Arc<dyn BillingProvider>)>
        where
            F: FnOnce() -> Option<std::sync::Arc<dyn BillingProvider>>,
        {
            match init() {
                Some(p) => {
                    tracing::info!(provider = name, "Billing provider initialized");
                    Some((name.to_string(), p))
                }
                None => {
                    tracing::info!(
                        provider = name,
                        "Billing provider skipped (missing env vars)"
                    );
                    None
                }
            }
        }

        let mut providers: std::collections::HashMap<String, std::sync::Arc<dyn BillingProvider>> =
            std::collections::HashMap::new();

        if let Some((k, v)) = try_init("stripe", || {
            let secret = env::var("STRIPE_SECRET_KEY").ok()?;
            let wh = env::var("STRIPE_WEBHOOK_SECRET").ok()?;
            Some(std::sync::Arc::new(
                StripeProvider::new(secret, wh).with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("polar", || {
            let token = env::var("POLAR_ACCESS_TOKEN").ok()?;
            let wh = env::var("POLAR_WEBHOOK_SECRET").ok()?;
            Some(std::sync::Arc::new(
                PolarProvider::new(token, wh).with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        // Key must stay "lemon_squeezy" — must match provider_name() and the /webhook/lemon_squeezy route; "lemonsqueezy" 404'd every webhook.
        if let Some((k, v)) = try_init("lemon_squeezy", || {
            let api_key = env::var("LEMONSQUEEZY_API_KEY").ok()?;
            let wh = env::var("LEMONSQUEEZY_WEBHOOK_SECRET").ok()?;
            let store_id = env::var("LEMONSQUEEZY_STORE_ID").ok()?;
            Some(std::sync::Arc::new(
                LemonSqueezyProvider::new(api_key, wh, store_id)
                    .with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("paddle", || {
            let client_token = env::var("PADDLE_CLIENT_TOKEN").ok()?;
            let wh = env::var("PADDLE_WEBHOOK_SECRET").ok()?;
            let mut provider =
                PaddleProvider::new(client_token, wh).with_http_client(http_client.clone());
            match env::var("PADDLE_PUBLIC_KEY") {
                Ok(hex_key) if !hex_key.trim().is_empty() => {
                    provider = provider.with_public_key(&hex_key).ok()?;
                }
                _ => tracing::warn!(
                    "PADDLE_PUBLIC_KEY not set; Paddle webhooks will fail verification until it is configured"
                ),
            }
            Some(std::sync::Arc::new(provider) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("crypto", || {
            let wallet = env::var("CRYPTO_WALLET_ADDRESS").ok()?;
            let api_url = env::var("CRYPTO_API_URL")
                .unwrap_or_else(|_| "https://api.blockcypher.com/v1".to_string());
            let api_key = env::var("CRYPTO_API_KEY").unwrap_or_else(|_| String::new());
            let currency = env::var("CRYPTO_CURRENCY").unwrap_or_else(|_| "BTC".to_string());
            Some(std::sync::Arc::new(
                CryptoProvider::new(wallet, api_url, api_key, currency)
                    .with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("crypto_multi", || {
            let provider = ruxlog::services::billing::crypto::MultiChainCryptoProvider::from_env()
                .ok()?
                .with_http_client(http_client.clone());
            Some(std::sync::Arc::new(provider) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("razorpay", || {
            let key_id = env::var("RAZORPAY_KEY_ID").ok()?;
            let key_secret = env::var("RAZORPAY_KEY_SECRET").ok()?;
            let wh = env::var("RAZORPAY_WEBHOOK_SECRET").ok()?;
            Some(std::sync::Arc::new(
                RazorpayProvider::new(key_id, key_secret, wh).with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("mercado_pago", || {
            let access_token = env::var("MERCADO_PAGO_ACCESS_TOKEN").ok()?;
            let wh = env::var("MERCADO_PAGO_WEBHOOK_SECRET").ok()?;
            Some(std::sync::Arc::new(
                MercadoPagoProvider::new(access_token, wh).with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("airwallex", || {
            let client_id = env::var("AIRWALLEX_CLIENT_ID").ok()?;
            let api_key = env::var("AIRWALLEX_API_KEY").ok()?;
            let wh = env::var("AIRWALLEX_WEBHOOK_SECRET").ok()?;
            Some(std::sync::Arc::new(
                AirwallexProvider::new(client_id, api_key, wh)
                    .with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("revolut", || {
            let api_key = env::var("REVOLUT_API_KEY").ok()?;
            let wh = env::var("REVOLUT_WEBHOOK_SECRET").ok()?;
            Some(std::sync::Arc::new(
                RevolutProvider::new(api_key, wh).with_http_client(http_client.clone()),
            ) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if let Some((k, v)) = try_init("paypal", || {
            let client_id = env::var("PAYPAL_CLIENT_ID").ok()?;
            let client_secret = env::var("PAYPAL_CLIENT_SECRET").ok()?;
            let wh = env::var("PAYPAL_WEBHOOK_SECRET").ok()?;
            let mut provider = PayPalProvider::new(client_id, client_secret, wh)
                .with_http_client(http_client.clone());
            if let Ok(id) = env::var("PAYPAL_WEBHOOK_ID") {
                if !id.is_empty() {
                    provider = provider.with_webhook_id(id);
                }
            }
            if provider.webhook_id.is_none() {
                tracing::warn!(
                    "PAYPAL_WEBHOOK_ID not set; PayPal webhooks will fail verification until it is configured"
                );
            }
            Some(std::sync::Arc::new(provider) as std::sync::Arc<dyn BillingProvider>)
        }) {
            providers.insert(k, v);
        }

        if providers.is_empty() {
            tracing::warn!("No billing providers initialized (missing env vars). Billing endpoints will return errors.");
        }

        let names: Vec<_> = providers.keys().collect();
        tracing::info!(providers = ?names, "Billing providers available");

        let geo_config = GeoRulesConfig::from_env();
        let geo_router = GeoRouter::new(geo_config);

        std::sync::Arc::new(BillingRouter::new(providers, geo_router))
    };

    let fcm: Option<std::sync::Arc<rux_fcm::FcmClient>> = {
        if env_bool("FCM_ENABLED", false) {
            let project_id = env::var("FCM_PROJECT_ID")
                .ok()
                .filter(|s| !s.trim().is_empty());
            let sa_path = env::var("FCM_SERVICE_ACCOUNT_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty());
            match (project_id, sa_path) {
                (Some(pid), Some(path)) => match rux_fcm::ServiceAccount::from_path(&path) {
                    Ok(sa) => Some(std::sync::Arc::new(rux_fcm::FcmClient::new(
                        sa,
                        pid,
                        http_client.clone(),
                    ))),
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            "FCM service-account load failed; push disabled \
                             (in-app notifications still work)"
                        );
                        None
                    }
                },
                _ => {
                    tracing::warn!(
                        "FCM_ENABLED=true but FCM_PROJECT_ID or FCM_SERVICE_ACCOUNT_PATH \
                         not set; push disabled"
                    );
                    None
                }
            }
        } else {
            None
        }
    };

    let webauthn_service: Option<std::sync::Arc<ruxlog::services::webauthn::WebauthnService>> =
        match ruxlog::services::webauthn::WebauthnService::from_env() {
            Ok(svc) => {
                tracing::info!("WebAuthn passkey service enabled");
                Some(std::sync::Arc::new(svc))
            }
            Err(e) => {
                // W8f: when web auth is enabled in production, a broken/localhost
                // WebAuthn config fails boot instead of silently disabling passkeys.
                if ruxlog::config::settings::web_auth().enabled
                    && matches!(
                        ruxlog::config::settings::env_class(),
                        ruxlog::config::settings::EnvClass::Production
                    )
                {
                    eprintln!(
                        "Configuration error: WEB_AUTH_ENABLED=true in production requires a valid \
                         WebAuthn RP — {e}"
                    );
                    std::process::exit(1);
                }
                tracing::warn!(
                    error = %e,
                    "WebAuthn passkey service disabled (invalid configuration; passkey endpoints will return 503)"
                );
                None
            }
        };

    let image_moderator: Option<
        std::sync::Arc<dyn ruxlog::services::image_moderation::ImageModerator + Send + Sync>,
    > = {
        use ruxlog::services::image_moderation::{HttpModerator, ImageModerator};
        let enabled = env_bool("IMAGE_MODERATION_ENABLED", false);
        let url = env_with_fallback(&["IMAGE_MODERATION_URL"], None);
        let api_key = env_with_fallback(&["IMAGE_MODERATION_API_KEY"], None).unwrap_or_default();
        if enabled {
            match url {
                Some(url) if !url.trim().is_empty() => {
                    tracing::info!(url = %url, "Image moderation enabled (HttpModerator)");
                    Some(
                        std::sync::Arc::new(HttpModerator::new(http_client.clone(), url, api_key))
                            as std::sync::Arc<dyn ImageModerator + Send + Sync>,
                    )
                }
                _ => {
                    tracing::warn!(
                        "IMAGE_MODERATION_ENABLED=true but IMAGE_MODERATION_URL is unset/empty; \
                         moderation disabled and uploads will be accepted unmoderated"
                    );
                    None
                }
            }
        } else {
            None
        }
    };

    let quran_load_started = std::time::Instant::now();
    let (quran_store, quran_load_duration_ms) =
        match ruxlog::quran::load_quran_store(&settings.quran).await {
            Ok(store) => {
                let resident_bytes = store.uthmani.bytes() + store.simple_clean.bytes();
                let load_duration_ms = quran_load_started.elapsed().as_millis() as u64;
                tracing::info!(
                    verse_count = ruxlog::quran::VERSE_COUNT,
                    resident_bytes,
                    load_duration_ms,
                    "Quran store loaded (uthmani + simple-clean); ready to serve Arabic reads"
                );
                (std::sync::Arc::new(store), load_duration_ms)
            }
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "Quran store failed to load — refusing to boot (§4.1 fail-fast)"
                );
                std::process::exit(1);
            }
        };

    let demand_collect = env_bool("QURAN_DEMAND_COLLECT", true);

    let (translation_pool, translation_catalogue_entries, translation_catalogue_load_duration_ms) = {
        let catalogue_load_started = std::time::Instant::now();
        let qset = &settings.quran;
        let catalogue_path = format!("{}/index.min.json", qset.translations_dir);
        match ruxlog::quran::load_catalogue(&catalogue_path).await {
            Ok(cat) => {
                let count = cat.len();
                let load_duration_ms = catalogue_load_started.elapsed().as_millis() as u64;
                let pool = ruxlog::quran::TranslationPool::new(
                    &cat,
                    std::path::PathBuf::from(&qset.translations_dir),
                    qset.max_resident_translations,
                    qset.max_resident_bytes,
                    std::time::Duration::from_secs(qset.translation_idle_ttl_secs),
                    demand_collect,
                    env_u64("QURAN_PREWARM_TRANSLATIONS", 2),
                );
                tracing::info!(
                    translations = count,
                    load_duration_ms,
                    max_resident_translations = qset.max_resident_translations,
                    max_resident_bytes = qset.max_resident_bytes,
                    idle_ttl_seconds = qset.translation_idle_ttl_secs,
                    "Translation catalogue loaded; pool ready for on-demand reads"
                );
                (std::sync::Arc::new(pool), count as u64, load_duration_ms)
            }
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "Translation catalogue failed to load — refusing to boot (§4 fail-fast)"
                );
                std::process::exit(1);
            }
        }
    };

    // Periodic durability runs as ONE spawned task on the shared SeaORM
    // connection: the rate-limit snapshot every 10s, plus — when demand
    // collection is on — the translation-popularity flush riding the same task
    // every 6th tick (~60s). Folding the popularity flush in here eliminates
    // the second spawned writer the W1 D1 draft left behind (one connection, no
    // extra write amplification). restore() stays above so rate-limit state is
    // live before any request; nothing between the original spawn site and here
    // serves requests, so the relocation is ordering-safe.
    let popularity_flush = if demand_collect {
        // Boot prewarm is backgrounded: translations are NOT fail-fast (Arabic
        // is). A bogus or absent candidate id warns and is skipped. load_ranked
        // decays in Rust; the pool filters by current catalogue membership
        // before truncating to N.
        let db_for_prewarm = sea_db.clone();
        let pool_for_prewarm = translation_pool.clone();
        tokio::spawn(async move {
            let ranked =
                ruxlog::services::translation_popularity_store::load_ranked(&db_for_prewarm).await;
            pool_for_prewarm.prewarm(ranked).await;
        });
        // Slow-companion callback for rate_limit_store::spawn_flush_task. Each
        // invocation clones the captured Arcs so the Fn (not FnOnce) can fire
        // every 6th tick for the life of the task. epoch_now is private to the
        // popularity module, so the same SystemTime->secs expression is inlined
        // here rather than widening that module's surface.
        let pop_db = sea_db.clone();
        let pop_pool = translation_pool.clone();
        Some(move || {
            let db = pop_db.clone();
            let pool = pop_pool.clone();
            async move {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let snapshot = pool.demand_snapshot();
                let catalogue_ids = pool.catalogue_ids().clone();
                match ruxlog::services::translation_popularity_store::flush(
                    &db,
                    &snapshot,
                    &catalogue_ids,
                    now,
                )
                .await
                {
                    Ok(ranked) => {
                        pool.set_top_demand(ranked);
                        // fetch_sub AFTER commit, exactly the snapshotted amount:
                        // counts accrued during the transaction stay in the
                        // atomic (snapshot is stale by that much).
                        pool.demand_subtract(&snapshot);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "translation popularity flush failed (non-fatal)")
                    }
                }
            }
        })
    } else {
        None
    };

    ruxlog::services::rate_limit_store::spawn_flush_task(
        sea_db.clone(),
        gate_store.clone(),
        popularity_flush,
    );

    let state = AppState {
        sea_db,
        gate_store,
        session_store: session_store.clone(),
        revoked_sessions: revoked_sessions.clone(),
        mailer,
        settings: settings.clone(),
        allowed_origins: allowed_origins.clone(),
        storage: StorageState {
            config: object_storage,
            client: s3_client,
            optimizer: settings.optimizer.clone(),
            image_moderator,
        },
        secret_key: settings.cookie_key.as_bytes().to_vec(),
        http_client,
        billing_router,
        fcm,
        webauthn: webauthn_service,
        quran: quran_store,
        quran_runtime_metrics: QuranRuntimeMetrics {
            arabic_load_duration_ms: quran_load_duration_ms,
            translation_catalogue_load_duration_ms,
            translation_catalogue_entries,
        },
        quran_scripts: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        translation_pool,
        quran_sources: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
    };

    {
        if let Err(err) = AclService::bootstrap_from_env(State(state.clone())).await {
            tracing::error!(error = %err, "Failed to bootstrap ACL constants from env");
        } else {
            tracing::info!("ACL constants bootstrapped from env");
        }
    }

    {
        let sync_interval_secs = env_u64("ROUTE_BLOCKER_SYNC_INTERVAL_SECS", 60 * 30);
        route_blocker_config::set_sync_interval_secs(sync_interval_secs);

        if let Err(err) = RouteBlockerService::initialize_cache(&state).await {
            tracing::error!(
                error = %err,
                "Initial route blocker cache sync failed; continuing without warm cache"
            );
        } else {
            tracing::info!("Initial route blocker cache sync completed successfully");
        }

        let state_for_blocker = state.clone();
        tokio::spawn(async move {
            let notify = route_blocker_config::notifier();

            route_blocker_config::set_next_sync_at(route_blocker_config::calculate_next_sync());

            loop {
                if route_blocker_config::is_paused() {
                    tokio::select! {
                        _ = notify.notified() => {},
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                    }
                    continue;
                }

                let force_sync = route_blocker_config::take_force_sync_flag();

                if !force_sync {
                    let interval_secs = route_blocker_config::get_sync_interval_secs();
                    let next_sync = route_blocker_config::calculate_next_sync();
                    route_blocker_config::set_next_sync_at(next_sync);

                    let sleep = tokio::time::sleep(Duration::from_secs(interval_secs));
                    tokio::pin!(sleep);

                    tokio::select! {
                        _ = &mut sleep => {},
                        _ = notify.notified() => {
                            continue;
                        }
                    }
                }

                if route_blocker_config::is_paused() {
                    continue;
                }

                route_blocker_config::set_sync_running(true);
                let sync_start = chrono::Utc::now();

                if let Err(err) = RouteBlockerService::initialize_cache(&state_for_blocker).await {
                    tracing::error!(
                        error = %err,
                        "Periodic route blocker cache sync failed"
                    );
                } else {
                    tracing::info!("Periodic route blocker cache sync completed successfully");
                }

                route_blocker_config::set_last_sync_at(sync_start);
                route_blocker_config::set_sync_running(false);
                route_blocker_config::set_next_sync_at(route_blocker_config::calculate_next_sync());
            }
        });
    }

    ruxlog::services::scheduler::start_scheduler(state.clone());

    tracing::info!("SQLite session store established.");
    {
        let sweep_store = session_store.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(3600));
            loop {
                ticker.tick().await;
                if let Err(e) = sweep_store.delete_expired().await {
                    tracing::warn!(error = %e, "session store expired-row sweep failed (non-fatal)");
                }
            }
        });
    }
    // HKDF-SHA256 is load-bearing — do NOT swap for a raw hash; a fast hash turns a weak COOKIE_KEY into a brute-forceable signing key.
    let cookie_key = Key::derive_from(settings.cookie_key.as_bytes());

    let cookie_secure = settings.http.cookie_secure;

    let session_layer = SessionManagerLayer::new((*session_store).clone())
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(24 * 14)))
        .with_same_site(SameSite::Lax)
        .with_secure(cookie_secure)
        .with_http_only(true)
        // Cookie name must stay hardcoded — the CSRF guard and frontend depend on it; a configurable name widens the cookie-fixing surface.
        .with_name("ruxlog.sid")
        .with_private(cookie_key);

    let compression = CompressionLayer::new();
    let cors = CorsLayer::new()
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(vec![
            HeaderName::from_static("csrf-token"),
            axum::http::header::ACCEPT,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT_ENCODING,
            axum::http::header::CONTENT_ENCODING,
        ])
        .expose_headers(vec![
            axum::http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            axum::http::header::SET_COOKIE,
            // W8b: client reads this to refresh its in-memory CSRF after a cycle_id().
            axum::http::HeaderName::from_static("x-eq-session-rotated"),
        ])
        .allow_origin(AllowOrigin::list(allowed_origins.header_values().to_vec()))
        .allow_credentials(true)
        .max_age(Duration::from_secs(360));

    let ip_source = settings.http.ip_source.clone();
    // Server-only token shared with trusted Docker-internal SSR. Layered as an
    // extension (never a public env var, response, or log) for the identity
    // middleware to compare constant-time.
    let internal_token: std::sync::Arc<str> =
        std::sync::Arc::<str>::from(settings.rate_limit.internal_token.as_str());

    let private = router::router(state.clone())
        .layer(middleware::from_fn(
            middlewares::client_ip::resolve_client_ip,
        ))
        .layer(ip_source.clone().into_extension())
        .layer(axum::Extension(middlewares::client_ip::InternalApiToken(
            internal_token.clone(),
        )))
        .layer(compression.clone())
        .layer(axum::Extension(state.clone()))
        .layer(middleware::from_fn(middlewares::cors::origin_guard))
        // session_layer must stay outer to csrf_guard — the Session must exist when csrf_guard recomputes the per-session HMAC.
        .layer(middleware::from_fn(middlewares::static_csrf::csrf_guard))
        .layer(session_layer)
        .layer(cors)
        .layer(middlewares::route_blocker::RouteBlockerLayer::new(
            state.clone(),
        ))
        // Outermost: every private API response (including origin/route-block
        // rejections) is uncacheable. Does not infer auth from `ruxlog.sid`, which
        // CSRF generation mints for anonymous sessions too. The public Quran router
        // never enters this middleware.
        .layer(middleware::from_fn(middlewares::cors::private_no_store));

    // W3a escalation: attached ONLY to the outer quran-v1 content limiter, and
    // only when QURAN_BAN_ESCALATION_ENABLED=true (default-off). Every other
    // limiter (internal SSR, health, search, auth, ...) holds None and keeps the
    // exact pre-W3a path. The engine talks to the in-memory gate store only.
    // Parsed independently here (not as a RateLimitSettings field) so existing
    // struct literals in other modules compile unchanged. enabled + a
    // missing/invalid allowlist is a boot error (fail closed).
    let escalation: Option<std::sync::Arc<dyn rux_request_gate::Escalation>> = {
        let cfg = match ruxlog::config::settings::EscalationConfig::from_env(
            settings.rate_limit.active_ban_max,
        ) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Configuration error: {e}");
                std::process::exit(1);
            }
        };
        if cfg.enabled {
            tracing::info!(
                allowlist_entries = cfg.allowlist.len(),
                temp_after = cfg.temp_after,
                long_after = cfg.long_after,
                "Quran ban escalation ENABLED (W3a)"
            );
            let engine: std::sync::Arc<dyn rux_request_gate::Escalation> = std::sync::Arc::new(
                ruxlog::services::rate_limit_store::escalation::EscalationEngine::new(
                    cfg,
                    state.gate_store.clone(),
                ),
            );
            Some(engine)
        } else {
            tracing::debug!("Quran ban escalation disabled (W3a default-off)");
            None
        }
    };

    let quran = ruxlog::modules::quran_v1::routes()
        .merge(
            ruxlog::modules::quran_v1::search_route()
                .layer(middlewares::rate_limit::rate_limit_layer(&state, 30, 60)),
        )
        .layer(middleware::from_fn(
            ruxlog::modules::quran_v1::error::shape_routing_errors,
        ));
    let public = router::with_observability(quran)
        .layer(compression)
        // Three isolated limiters: health (identity-independent) + internal SSR
        // (service label) + external content (verified IP). Each self-skips via
        // `applies`; only external IPs enter the content/escalation bucket.
        .layer(middlewares::rate_limit::quran_health_layer(&state))
        .layer(middlewares::rate_limit::quran_internal_layer(&state))
        .layer(middlewares::rate_limit::quran_content_layer(&state).with_escalation(escalation))
        .layer(middleware::from_fn(
            middlewares::client_ip::resolve_client_ip,
        ))
        .layer(ip_source.into_extension())
        .layer(axum::Extension(middlewares::client_ip::InternalApiToken(
            internal_token,
        )))
        .layer(axum::Extension(state.clone()))
        .layer(ruxlog::modules::quran_v1::cors::public_cors_layer());

    let app = axum::Router::new()
        .merge(private)
        .nest("/quran", public)
        .with_state(state);

    let host = settings.http.host.clone();
    let port = settings.http.port.clone();
    let address = format!("{}:{}", host, port);
    let address = address.parse::<std::net::SocketAddr>()?;
    tracing::info!("Listening on http://{}", address);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
