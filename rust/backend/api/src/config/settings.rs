use axum_client_ip::ClientIpSource;
use ipnet::IpNet;

use crate::config::env::{env_bool, env_u64, env_u8, env_with_fallback};

// ONE production gate. Precedence: RUST_ENV -> NODE_ENV -> APP_ENV (first set
// wins). `production` is production; development|dev|test|testing|ci|local are
// non-production; unset/unknown is a configuration error outside tests (cfg!test
// reads unset/unknown as non-production so unit tests need not seed the env).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvClass {
    Production,
    NonProduction,
    Unset,
    Unknown,
}

pub fn env_class() -> EnvClass {
    let raw = std::env::var("RUST_ENV")
        .or_else(|_| std::env::var("NODE_ENV"))
        .or_else(|_| std::env::var("APP_ENV"))
        .ok();
    match raw.as_deref().map(str::trim) {
        None => EnvClass::Unset,
        Some("production") => EnvClass::Production,
        Some("development" | "dev" | "test" | "testing" | "ci" | "local") => {
            EnvClass::NonProduction
        }
        Some(_) => EnvClass::Unknown,
    }
}

pub fn is_production() -> Result<bool, String> {
    match env_class() {
        EnvClass::Production => Ok(true),
        EnvClass::NonProduction => Ok(false),
        EnvClass::Unset if cfg!(test) => Ok(false),
        EnvClass::Unknown if cfg!(test) => Ok(false),
        EnvClass::Unset => Err(
            "RUST_ENV/NODE_ENV/APP_ENV is unset. Set RUST_ENV=production (or one of \
             development|dev|test|testing|ci|local)."
                .to_string(),
        ),
        EnvClass::Unknown => Err(
            "RUST_ENV/NODE_ENV/APP_ENV has an unknown value. Use production or one of \
             development|dev|test|testing|ci|local."
                .to_string(),
        ),
    }
}

#[cfg(test)]
pub static TEST_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Manual Debug redacts access_key/secret_key — replacing with derive(Debug) leaks them.
#[derive(Clone)]
pub struct ObjectStorageConfig {
    pub region: String,
    pub account_id: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub public_url: String,
    pub endpoint: String,
}

impl std::fmt::Debug for ObjectStorageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectStorageConfig")
            .field("region", &self.region)
            .field("account_id", &self.account_id)
            .field("bucket", &self.bucket)
            .field("access_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .field("public_url", &self.public_url)
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

impl ObjectStorageConfig {
    pub fn from_env() -> Self {
        let bucket = env_with_fallback(&["S3_BUCKET", "AWS_S3_BUCKET"], None)
            .expect("S3_BUCKET or AWS_S3_BUCKET must be set");
        let access_key = env_with_fallback(&["S3_ACCESS_KEY", "AWS_ACCESS_KEY_ID"], None)
            .expect("S3_ACCESS_KEY or AWS_ACCESS_KEY_ID must be set");
        let secret_key = env_with_fallback(&["S3_SECRET_KEY", "AWS_SECRET_ACCESS_KEY"], None)
            .expect("S3_SECRET_KEY or AWS_SECRET_ACCESS_KEY must be set");
        let endpoint =
            env_with_fallback(&["S3_ENDPOINT", "AWS_ENDPOINT", "GARAGE_S3_ENDPOINT"], None)
                .expect("S3_ENDPOINT, AWS_ENDPOINT, or GARAGE_S3_ENDPOINT must be set");
        let public_url = env_with_fallback(&["S3_PUBLIC_URL", "AWS_S3_PUBLIC_URL"], None)
            .unwrap_or_else(|| endpoint.clone());
        let region = env_with_fallback(
            &[
                "S3_REGION",
                "GARAGE_S3_REGION",
                "AWS_S3_REGION",
                "AWS_REGION",
            ],
            Some("auto"),
        )
        .unwrap();
        let account_id = std::env::var("S3_ACCOUNT_ID").unwrap_or_else(|_| "local".to_string());

        Self {
            region,
            account_id,
            bucket,
            access_key,
            secret_key,
            public_url,
            endpoint,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OptimizerConfig {
    pub enabled: bool,
    pub max_pixels: u64,
    pub keep_original: bool,
    pub default_webp_quality: u8,
}

impl OptimizerConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_bool("OPTIMIZE_ON_UPLOAD", true),
            max_pixels: env_u64("OPTIMIZER_MAX_PIXELS", 12_000_000),
            keep_original: env_bool("OPTIMIZER_KEEP_ORIGINAL", true),
            default_webp_quality: env_u8("OPTIMIZER_WEBP_QUALITY_DEFAULT", 80),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HttpSettings {
    pub host: String,
    pub port: String,
    pub ip_source: ClientIpSource,
    pub cookie_secure: bool,
}

impl HttpSettings {
    pub fn from_env() -> Result<Self, String> {
        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("PORT").unwrap_or_else(|_| "8888".to_string());
        let is_prod = is_production()?;
        let ip_source = std::env::var("IP_SOURCE")
            .unwrap_or_else(|_| "ConnectInfo".to_string())
            .parse::<ClientIpSource>()
            .map_err(|e| {
                format!(
                    "Invalid IP_SOURCE value: {e}. Valid: ConnectInfo (default, TCP peer) | \
                     CfConnectingIp (Cloudflare CF-Connecting-IP — use behind Cloudflare→Traefik) \
                     | XRealIp | TrueClientIp | FlyClientIp | RightmostXForwardedFor | \
                     RightmostForwarded | CloudFrontViewerAddress."
                )
            })?;
        // Production sits behind Cloudflare→Traefik; ConnectInfo would rate-limit on the
        // proxy's IP and collapse every caller into one bucket. Require a verified header.
        if is_prod && matches!(ip_source, ClientIpSource::ConnectInfo) {
            return Err(
                "IP_SOURCE=ConnectInfo is invalid in production: behind Cloudflare→Traefik every \
                 request shares the proxy IP. Set IP_SOURCE=CfConnectingIp and restrict origin \
                 ingress to Cloudflare ranges (see deploy/README.md)."
                    .to_string(),
            );
        }
        let cookie_secure = env_bool("COOKIE_SECURE", true);
        Ok(Self {
            host,
            port,
            ip_source,
            cookie_secure,
        })
    }
}

pub struct SiteSettings {
    pub url: String,
    pub name: String,
    pub consumer_site_url: String,
}

impl SiteSettings {
    pub fn from_env() -> Self {
        Self {
            url: std::env::var("SITE_URL").unwrap_or_else(|_| "http://localhost:8888".to_string()),
            name: std::env::var("SITE_NAME").unwrap_or_else(|_| "Ruxlog".to_string()),
            consumer_site_url: std::env::var("CONSUMER_SITE_URL")
                .unwrap_or_else(|_| "https://ruxlog.com".to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuranSettings {
    pub uthmani_path: String,
    pub simple_clean_path: String,
    pub metadata_xml_path: String,
    pub translations_dir: String,
    pub max_resident_translations: u64,
    pub max_resident_bytes: u64,
    pub translation_idle_ttl_secs: u64,
}

const QURAN_DB_BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran/tanzil");

impl QuranSettings {
    pub fn from_env() -> Self {
        Self {
            uthmani_path: std::env::var("QURAN_UTHMANI_PATH")
                .unwrap_or_else(|_| format!("{QURAN_DB_BASE}/arabic/quran-uthmani.sqlite")),
            simple_clean_path: std::env::var("QURAN_SIMPLE_CLEAN_PATH")
                .unwrap_or_else(|_| format!("{QURAN_DB_BASE}/arabic/quran-simple-clean.sqlite")),
            metadata_xml_path: std::env::var("QURAN_METADATA_XML_PATH")
                .unwrap_or_else(|_| format!("{QURAN_DB_BASE}/quran-data.xml")),
            translations_dir: std::env::var("QURAN_TRANSLATIONS_DIR")
                .unwrap_or_else(|_| format!("{QURAN_DB_BASE}/translations")),
            max_resident_translations: env_u64("QURAN_MAX_RESIDENT_TRANSLATIONS", 8),
            max_resident_bytes: env_u64("QURAN_MAX_RESIDENT_TRANSLATION_BYTES", 48 * 1024 * 1024),
            translation_idle_ttl_secs: env_u64("QURAN_TRANSLATION_IDLE_TTL_SECS", 1800),
        }
    }
}

// W3a escalation config. Default-OFF: QURAN_BAN_ESCALATION_ENABLED=false, so the
// whole state machine is a no-op until an operator explicitly enables it AND the
// hard-gate checklist passes (W2 ingress + W3b + W3c + valid CIDR allowlist).
// Parsed once at boot; an enabled flag with a missing/invalid allowlist fails the
// boot. Allowlist CIDRs are matched against the RAW client IpAddr before BanUnit
// normalization; an allowlisted unit never escalates and never appears in export.
#[derive(Clone, Debug)]
pub struct EscalationConfig {
    pub enabled: bool,
    pub key_prefix: &'static str,
    pub temp_after: u32,
    pub temp_window_secs: u64,
    pub temp_duration_secs: u64,
    pub long_after: u32,
    pub long_window_secs: u64,
    pub long_duration_secs: u64,
    pub suspicious_4xx_per_window: u32,
    pub max_tracked_identities: usize,
    pub max_active_bans: usize,
    pub allowlist: Vec<IpNet>,
}

impl EscalationConfig {
    // Disabled default for tests / non-production builds that never touch the env.
    #[allow(dead_code)]
    pub fn disabled(max_active_bans: usize) -> Self {
        Self {
            enabled: false,
            key_prefix: "quran-ban",
            temp_after: 5,
            temp_window_secs: 3600,
            temp_duration_secs: 3600,
            long_after: 20,
            long_window_secs: 86400,
            long_duration_secs: 604800,
            suspicious_4xx_per_window: 20,
            max_tracked_identities: 10_000,
            max_active_bans,
            allowlist: Vec::new(),
        }
    }

    pub fn from_env(max_active_bans: usize) -> Result<Self, String> {
        let enabled = env_bool("QURAN_BAN_ESCALATION_ENABLED", false);
        let allowlist_raw = std::env::var("QURAN_BAN_ALLOWLIST");
        let allowlist = match allowlist_raw {
            Ok(raw) => parse_allowlist(&raw)?,
            Err(_) => {
                if enabled {
                    return Err(
                        "QURAN_BAN_ESCALATION_ENABLED=true requires QURAN_BAN_ALLOWLIST to be set \
                         (comma-separated CIDRs; set to empty for no exceptions)."
                            .to_string(),
                    );
                }
                Vec::new()
            }
        };
        Ok(Self {
            enabled,
            key_prefix: "quran-ban",
            temp_after: 5,
            temp_window_secs: 3600,
            temp_duration_secs: 3600,
            long_after: 20,
            long_window_secs: 86400,
            long_duration_secs: 604800,
            suspicious_4xx_per_window: 20,
            max_tracked_identities: env_u64("QURAN_ESCALATION_MAX_IDENTITIES", 10_000) as usize,
            max_active_bans,
            allowlist,
        })
    }
}

// Parse comma-separated CIDRs. IPv6 prefixes narrower than /64 are rejected:
// proxy export blocks /64 units and cannot preserve a narrower exception, so an
// allowlist entry below /64 would silently fail to exempt its addresses.
fn parse_allowlist(raw: &str) -> Result<Vec<IpNet>, String> {
    let mut nets = Vec::new();
    for part in raw.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let net: IpNet = p
            .parse()
            .map_err(|e| format!("invalid CIDR '{p}' in QURAN_BAN_ALLOWLIST: {e}"))?;
        if let IpNet::V6(v6) = &net {
            if v6.prefix_len() > 64 {
                return Err(format!(
                    "QURAN_BAN_ALLOWLIST IPv6 prefix '{net}' is narrower than /64; proxy export \
                     blocks /64 units and cannot preserve a narrower exception"
                ));
            }
        }
        nets.push(net);
    }
    Ok(nets)
}

// Manual Debug redacts ban_export_token + internal_token — deriving Debug would
// leak bearer secrets into logs, matching the ObjectStorageConfig / cookie_key discipline.
#[derive(Clone)]
pub struct RateLimitSettings {
    pub active_ban_max: usize,
    // Read once at boot into AppState (never per-request). Empty/unset disables
    // machine access to GET /admin/bans/export; human access still works via the
    // admin session ACL. NEVER accepted by mutation routes (list/delete).
    pub ban_export_token: String,
    // Server-only shared secret for trusted Docker-internal SSR (Bun). Compared
    // constant-time; never logged, never exposed in a public env var or response.
    pub internal_token: String,
    // Separate non-escalating buckets: internal SSR + public readiness. They never
    // share an external IP bucket and never enter W3a escalation state.
    pub internal_requests_per_minute: u64,
    pub health_requests_per_minute: u64,
}

impl std::fmt::Debug for RateLimitSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let export_state = if self.ban_export_token.is_empty() {
            "unset"
        } else {
            "set"
        };
        let internal_state = if self.internal_token.is_empty() {
            "unset"
        } else {
            "set"
        };
        f.debug_struct("RateLimitSettings")
            .field("active_ban_max", &self.active_ban_max)
            .field("ban_export_token", &export_state)
            .field("internal_token", &internal_state)
            .field(
                "internal_requests_per_minute",
                &self.internal_requests_per_minute,
            )
            .field(
                "health_requests_per_minute",
                &self.health_requests_per_minute,
            )
            .finish()
    }
}

impl RateLimitSettings {
    pub fn from_env() -> Result<Self, String> {
        let is_prod = is_production()?;
        let internal_token = std::env::var("INTERNAL_QURAN_API_TOKEN").unwrap_or_default();
        let internal_requests_per_minute = env_u64("QURAN_INTERNAL_REQUESTS_PER_MINUTE", 600);
        let health_requests_per_minute = env_u64("QURAN_HEALTH_REQUESTS_PER_MINUTE", 120);
        if is_prod {
            if internal_token.trim().is_empty() {
                return Err("INTERNAL_QURAN_API_TOKEN is unset/empty in production. \
                     Generate with: openssl rand -hex 32."
                    .to_string());
            }
            if internal_requests_per_minute == 0 {
                return Err(
                    "QURAN_INTERNAL_REQUESTS_PER_MINUTE must be > 0 in production.".to_string(),
                );
            }
            if health_requests_per_minute == 0 {
                return Err(
                    "QURAN_HEALTH_REQUESTS_PER_MINUTE must be > 0 in production.".to_string(),
                );
            }
        }
        Ok(Self {
            active_ban_max: env_u64("QURAN_ACTIVE_BAN_MAX", 2_000) as usize,
            ban_export_token: std::env::var("BAN_EXPORT_TOKEN").unwrap_or_default(),
            internal_token,
            internal_requests_per_minute,
            health_requests_per_minute,
        })
    }
}

/// Readiness-only per-provider state. `ready` reflects whether the provider's
/// credentials are present at boot — never the credential values themselves.
/// Surfaced on the public readiness endpoint so an operator can see WHICH
/// provider is misconfigured without exposing secrets (W8f).
#[derive(Clone, Debug, Default)]
pub struct ProviderStatus {
    pub name: String,
    pub ready: bool,
}

// Web auth gate (W8f). Default OFF: when WEB_AUTH_ENABLED is false the auth
// router is not mounted and production does not require auth-side credentials.
// Production with WEB_AUTH_ENABLED=true fails boot unless every listed provider
// has its credentials + HTTPS redirect (origin-matched), WebAuthn is a real RP,
// and MAIL_PROVIDER is a real transport (not none).
#[derive(Clone, Debug, Default)]
pub struct WebAuthSettings {
    pub enabled: bool,
    pub oauth_providers: Vec<String>,
    pub providers_status: Vec<ProviderStatus>,
}

impl WebAuthSettings {
    pub fn from_env() -> Result<Self, String> {
        let enabled = env_bool("WEB_AUTH_ENABLED", false);
        let oauth_providers: Vec<String> = std::env::var("WEB_OAUTH_PROVIDERS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        // providers_status is computed always (readiness DTO reports it). `ready`
        // means "credentials present" — no values stored.
        let providers_status = oauth_providers
            .iter()
            .map(|p| ProviderStatus {
                name: p.clone(),
                ready: provider_creds_present(p),
            })
            .collect();

        let is_prod = is_production()?;
        if is_prod && enabled {
            // Auth needs a real mail transport for verification + recovery.
            let mail = std::env::var("MAIL_PROVIDER").unwrap_or_default();
            if mail.trim() == "none" {
                return Err(
                    "Production with WEB_AUTH_ENABLED=true rejects MAIL_PROVIDER=none; set \
                     MAIL_PROVIDER=smtp|cloudflare and configure credentials."
                        .to_string(),
                );
            }
            // Verification + recovery mail must come from a real identity. Require
            // non-empty from-address + from-name; both are read by build_mail_router.
            if std::env::var("MAIL_FROM_ADDRESS")
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(
                    "Production with WEB_AUTH_ENABLED=true requires a non-empty MAIL_FROM_ADDRESS \
                     (the From: identity used for verification + recovery mail)."
                        .to_string(),
                );
            }
            if std::env::var("MAIL_FROM_NAME")
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(
                    "Production with WEB_AUTH_ENABLED=true requires a non-empty MAIL_FROM_NAME \
                     (display name paired with MAIL_FROM_ADDRESS)."
                        .to_string(),
                );
            }
            // The selected provider must have its credentials present. Values are
            // never logged — presence is checked, mirroring provider_creds_present.
            for k in mail_provider_required_keys(&mail) {
                if std::env::var(k).unwrap_or_default().trim().is_empty() {
                    return Err(format!(
                        "Production with WEB_AUTH_ENABLED=true: MAIL_PROVIDER='{mail}' is missing \
                         {k}"
                    ));
                }
            }
            for p in &oauth_providers {
                if provider_required_keys(p).is_empty() {
                    return Err(format!(
                        "Unknown provider '{p}' in WEB_OAUTH_PROVIDERS; valid: google, apple, facebook, github"
                    ));
                }
                for k in provider_required_keys(p) {
                    if std::env::var(k).unwrap_or_default().trim().is_empty() {
                        return Err(format!(
                            "Production with WEB_AUTH_ENABLED=true: provider '{p}' is missing {k}"
                        ));
                    }
                }
                if p == "apple" {
                    let pk = std::env::var("APPLE_PRIVATE_KEY").unwrap_or_default();
                    let pkp = std::env::var("APPLE_PRIVATE_KEY_PATH").unwrap_or_default();
                    if pk.trim().is_empty() && pkp.trim().is_empty() {
                        return Err("Production with WEB_AUTH_ENABLED=true: apple requires \
                             APPLE_PRIVATE_KEY or APPLE_PRIVATE_KEY_PATH"
                            .to_string());
                    }
                }
            }
            // Redirect-origin match: each provider callback origin must be in the
            // allowed list or equal FRONTEND_URL.
            let allowed: Vec<String> = std::env::var("OAUTH_ALLOWED_REDIRECT_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let frontend_origin = origin_of(&std::env::var("FRONTEND_URL").unwrap_or_default());
            // W8F-3: OAuth redirects must never cross to an http:// origin — the
            // authorization code + session cookie would traverse cleartext. Reject
            // any non-HTTPS entry in the allowlist or FRONTEND_URL before the
            // provider-loop membership test runs.
            for o in &allowed {
                if o.starts_with("http://") {
                    return Err(format!(
                        "Production with WEB_AUTH_ENABLED=true: OAUTH_ALLOWED_REDIRECT_ORIGINS \
                         entry '{o}' is not HTTPS (got http://); OAuth callbacks require https:// \
                         origins"
                    ));
                }
            }
            if frontend_origin.starts_with("http://") {
                return Err(format!(
                    "Production with WEB_AUTH_ENABLED=true: FRONTEND_URL origin \
                     '{frontend_origin}' is not HTTPS (got http://); OAuth callbacks require \
                     https:// origins"
                ));
            }
            // Presence gate: the HTTPS-scheme check above is a no-op when FRONTEND_URL
            // is unset/empty (empty origin parses as neither http:// nor https://). An
            // absent FRONTEND_URL boots successfully then breaks every OAuth
            // success/failure redirect at runtime, so reject it explicitly here.
            if std::env::var("FRONTEND_URL")
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(
                    "Production with WEB_AUTH_ENABLED=true requires FRONTEND_URL to be set \
                     (the OAuth success/failure redirect target; without it every post-login \
                     redirect breaks at runtime)."
                        .to_string(),
                );
            }
            for p in &oauth_providers {
                let rk = provider_redirect_key(p);
                let redirect = std::env::var(rk).unwrap_or_default();
                let origin = origin_of(&redirect);
                if !origin.is_empty() && !allowed.contains(&origin) && origin != frontend_origin {
                    return Err(format!(
                        "Production with WEB_AUTH_ENABLED=true: {rk} origin '{origin}' is not in \
                         OAUTH_ALLOWED_REDIRECT_ORIGINS and does not match FRONTEND_URL"
                    ));
                }
            }
        }
        Ok(Self {
            enabled,
            oauth_providers,
            providers_status,
        })
    }
}

fn provider_required_keys(p: &str) -> &'static [&'static str] {
    match p {
        "google" => &[
            "GOOGLE_CLIENT_ID",
            "GOOGLE_CLIENT_SECRET",
            "GOOGLE_REDIRECT_URI",
        ],
        "apple" => &[
            "APPLE_CLIENT_ID",
            "APPLE_TEAM_ID",
            "APPLE_KEY_ID",
            "APPLE_REDIRECT_URI",
        ],
        "facebook" => &[
            "FACEBOOK_CLIENT_ID",
            "FACEBOOK_CLIENT_SECRET",
            "FACEBOOK_REDIRECT_URI",
        ],
        "github" => &[
            "GITHUB_CLIENT_ID",
            "GITHUB_CLIENT_SECRET",
            "GITHUB_REDIRECT_URI",
        ],
        _ => &[],
    }
}

/// Required env vars for the selected mail transport (mirrors the keys
/// build_mail_router / smtp::create_connection / CloudflareMailProvider read).
/// Used only for presence checks at boot — values never logged.
fn mail_provider_required_keys(provider: &str) -> &'static [&'static str] {
    match provider.trim() {
        "smtp" => &["SMTP_HOST", "SMTP_USERNAME", "SMTP_PASSWORD"],
        "cloudflare" => &["CLOUDFLARE_EMAIL_ACCOUNT_ID", "CLOUDFLARE_EMAIL_API_TOKEN"],
        _ => &[],
    }
}

fn provider_redirect_key(p: &str) -> &'static str {
    match p {
        "google" => "GOOGLE_REDIRECT_URI",
        "apple" => "APPLE_REDIRECT_URI",
        "facebook" => "FACEBOOK_REDIRECT_URI",
        "github" => "GITHUB_REDIRECT_URI",
        _ => "",
    }
}

fn provider_creds_present(p: &str) -> bool {
    if provider_required_keys(p).is_empty() {
        return false;
    }
    for k in provider_required_keys(p) {
        if std::env::var(k).unwrap_or_default().trim().is_empty() {
            return false;
        }
    }
    if p == "apple" {
        let pk = std::env::var("APPLE_PRIVATE_KEY").unwrap_or_default();
        let pkp = std::env::var("APPLE_PRIVATE_KEY_PATH").unwrap_or_default();
        if pk.trim().is_empty() && pkp.trim().is_empty() {
            return false;
        }
    }
    true
}

/// scheme://host[:port] of an absolute URL, or empty if it does not parse as
/// http(s). Used to compare a provider callback against the allowed origins.
fn origin_of(url: &str) -> String {
    let url = url.trim();
    let scheme = if url.starts_with("https://") {
        "https://"
    } else if url.starts_with("http://") {
        "http://"
    } else {
        return String::new();
    };
    let rest = &url[scheme.len()..];
    let host_port = rest.split('/').next().unwrap_or("");
    if host_port.is_empty() {
        String::new()
    } else {
        format!("{scheme}{host_port}")
    }
}

/// Do NOT add `derive(Debug)` — `cookie_key` is a raw secret and would leak.
pub struct Settings {
    pub cookie_key: String,
    pub http: HttpSettings,
    pub site: SiteSettings,
    pub object_storage: ObjectStorageConfig,
    pub optimizer: OptimizerConfig,
    pub quran: QuranSettings,
    pub rate_limit: RateLimitSettings,
}

// Web auth is exposed via a process-wide accessor (not a Settings field) so the
// auth router gate + readiness surface can read it without every test that
// constructs a literal `Settings { ... }` needing to populate it. Set once by
// Settings::from_env at boot; tests that bypass from_env read the disabled
// default. Validates production config as a side effect of from_env (fails boot
// on MAIL_PROVIDER=none / missing creds / redirect mismatch).
static WEB_AUTH: std::sync::OnceLock<WebAuthSettings> = std::sync::OnceLock::new();

/// Process-wide web-auth settings. Returns a disabled default before the first
/// `Settings::from_env()` (e.g. in tests that build a literal `Settings`).
pub fn web_auth() -> &'static WebAuthSettings {
    WEB_AUTH.get_or_init(WebAuthSettings::default)
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        let cookie_key =
            std::env::var("COOKIE_KEY").map_err(|_| "COOKIE_KEY must be set".to_string())?;
        crate::state::validate_cookie_key(&cookie_key)?;

        let object_storage = ObjectStorageConfig::from_env();

        let optimizer = OptimizerConfig::from_env();

        let http = HttpSettings::from_env()?;
        let site = SiteSettings::from_env();
        let quran = QuranSettings::from_env();
        let rate_limit = RateLimitSettings::from_env()?;
        // Validate web-auth config as a boot side effect (fails boot in production
        // on MAIL_PROVIDER=none / missing provider creds / redirect-origin
        // mismatch). Stored process-wide so the auth router gate and the readiness
        // surface read it without a Settings field (keeps literal Settings test
        // construction working).
        let web_auth = WebAuthSettings::from_env()?;
        let _ = WEB_AUTH.set(web_auth);

        Ok(Self {
            cookie_key,
            http,
            site,
            object_storage,
            optimizer,
            quran,
            rate_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_cookie_key(strong: &str) {
        std::env::set_var("COOKIE_KEY", strong);
    }

    #[test]
    fn object_storage_panics_when_bucket_missing() {
        let prev_bucket = std::env::var("S3_BUCKET").ok();
        let prev_aws = std::env::var("AWS_S3_BUCKET").ok();
        std::env::remove_var("S3_BUCKET");
        std::env::remove_var("AWS_S3_BUCKET");

        let result = std::panic::catch_unwind(ObjectStorageConfig::from_env);
        assert!(
            result.is_err(),
            "ObjectStorageConfig::from_env must panic when S3_BUCKET is missing"
        );

        match prev_bucket {
            Some(v) => std::env::set_var("S3_BUCKET", v),
            None => std::env::remove_var("S3_BUCKET"),
        }
        match prev_aws {
            Some(v) => std::env::set_var("AWS_S3_BUCKET", v),
            None => std::env::remove_var("AWS_S3_BUCKET"),
        }
    }

    #[test]
    fn settings_reject_known_cookie_key_placeholder() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let prev = std::env::var("COOKIE_KEY").ok();
        set_cookie_key("CHANGE_ME_rotate_me_generate_with_openssl_rand_hex_32___");

        let result = Settings::from_env();
        assert!(
            result.is_err(),
            "Settings::from_env must reject the known COOKIE_KEY placeholder"
        );

        match prev {
            Some(v) => std::env::set_var("COOKIE_KEY", v),
            None => std::env::remove_var("COOKIE_KEY"),
        }
    }

    #[test]
    fn site_settings_defaults() {
        let prev_url = std::env::var("SITE_URL").ok();
        let prev_name = std::env::var("SITE_NAME").ok();
        let prev_consumer = std::env::var("CONSUMER_SITE_URL").ok();
        std::env::remove_var("SITE_URL");
        std::env::remove_var("SITE_NAME");
        std::env::remove_var("CONSUMER_SITE_URL");

        let site = SiteSettings::from_env();
        assert_eq!(site.url, "http://localhost:8888");
        assert_eq!(site.name, "Ruxlog");
        assert_eq!(site.consumer_site_url, "https://ruxlog.com");

        match prev_url {
            Some(v) => std::env::set_var("SITE_URL", v),
            None => std::env::remove_var("SITE_URL"),
        }
        match prev_name {
            Some(v) => std::env::set_var("SITE_NAME", v),
            None => std::env::remove_var("SITE_NAME"),
        }
        match prev_consumer {
            Some(v) => std::env::set_var("CONSUMER_SITE_URL", v),
            None => std::env::remove_var("CONSUMER_SITE_URL"),
        }
    }

    // --- serialized env tests for is_production / env_class -------------------
    // All env-touching tests hold TEST_ENV_MUTEX so RUST_ENV/NODE_ENV/APP_ENV
    // never race (state.rs field_enc_key tests take the same lock).

    struct EnvSnapshot {
        rust_env: Option<String>,
        node_env: Option<String>,
        app_env: Option<String>,
        ip_source: Option<String>,
        internal_token: Option<String>,
        internal_rpm: Option<String>,
        health_rpm: Option<String>,
        allowed_origins: Option<String>,
    }

    fn snapshot_env() -> EnvSnapshot {
        EnvSnapshot {
            rust_env: std::env::var("RUST_ENV").ok(),
            node_env: std::env::var("NODE_ENV").ok(),
            app_env: std::env::var("APP_ENV").ok(),
            ip_source: std::env::var("IP_SOURCE").ok(),
            internal_token: std::env::var("INTERNAL_QURAN_API_TOKEN").ok(),
            internal_rpm: std::env::var("QURAN_INTERNAL_REQUESTS_PER_MINUTE").ok(),
            health_rpm: std::env::var("QURAN_HEALTH_REQUESTS_PER_MINUTE").ok(),
            allowed_origins: std::env::var("ALLOWED_ORIGINS").ok(),
        }
    }

    fn clear_env_vars() {
        for k in [
            "RUST_ENV",
            "NODE_ENV",
            "APP_ENV",
            "IP_SOURCE",
            "INTERNAL_QURAN_API_TOKEN",
            "QURAN_INTERNAL_REQUESTS_PER_MINUTE",
            "QURAN_HEALTH_REQUESTS_PER_MINUTE",
            "ALLOWED_ORIGINS",
        ] {
            std::env::remove_var(k);
        }
    }

    fn restore_env(snap: EnvSnapshot) {
        match snap.rust_env {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
        match snap.node_env {
            Some(v) => std::env::set_var("NODE_ENV", v),
            None => std::env::remove_var("NODE_ENV"),
        }
        match snap.app_env {
            Some(v) => std::env::set_var("APP_ENV", v),
            None => std::env::remove_var("APP_ENV"),
        }
        match snap.ip_source {
            Some(v) => std::env::set_var("IP_SOURCE", v),
            None => std::env::remove_var("IP_SOURCE"),
        }
        match snap.internal_token {
            Some(v) => std::env::set_var("INTERNAL_QURAN_API_TOKEN", v),
            None => std::env::remove_var("INTERNAL_QURAN_API_TOKEN"),
        }
        match snap.internal_rpm {
            Some(v) => std::env::set_var("QURAN_INTERNAL_REQUESTS_PER_MINUTE", v),
            None => std::env::remove_var("QURAN_INTERNAL_REQUESTS_PER_MINUTE"),
        }
        match snap.health_rpm {
            Some(v) => std::env::set_var("QURAN_HEALTH_REQUESTS_PER_MINUTE", v),
            None => std::env::remove_var("QURAN_HEALTH_REQUESTS_PER_MINUTE"),
        }
        match snap.allowed_origins {
            Some(v) => std::env::set_var("ALLOWED_ORIGINS", v),
            None => std::env::remove_var("ALLOWED_ORIGINS"),
        }
    }

    #[test]
    fn env_class_reads_rust_env_first() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("NODE_ENV", "development");
        std::env::set_var("APP_ENV", "development");
        assert_eq!(env_class(), EnvClass::Production);
        assert!(is_production().unwrap());
        restore_env(snap);
    }

    #[test]
    fn env_class_falls_through_to_app_env() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("APP_ENV", "development");
        assert_eq!(env_class(), EnvClass::NonProduction);
        assert!(!is_production().unwrap());
        restore_env(snap);
    }

    #[test]
    fn env_class_dev_aliases_are_non_production() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        for v in ["dev", "test", "testing", "ci", "local", "development"] {
            clear_env_vars();
            std::env::set_var("RUST_ENV", v);
            assert_eq!(env_class(), EnvClass::NonProduction, "value: {v}");
            assert!(!is_production().unwrap(), "value: {v}");
        }
        restore_env(snap);
    }

    #[test]
    fn env_class_unset_is_config_error_outside_test_only() {
        // cfg!(test) is true here, so unset reads as non-production. The Err
        // branch is exercised by is_production()'s cfg gate, not by this test.
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        assert_eq!(env_class(), EnvClass::Unset);
        assert!(!is_production().unwrap());
        restore_env(snap);
    }

    #[test]
    fn env_class_unknown_value_is_unknown() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "staging-xyz");
        assert_eq!(env_class(), EnvClass::Unknown);
        // cfg!(test) reads Unknown as non-production (never silently production).
        assert!(!is_production().unwrap());
        restore_env(snap);
    }

    #[test]
    fn http_settings_prod_connect_info_rejected() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("IP_SOURCE", "ConnectInfo");
        let err = HttpSettings::from_env().expect_err("prod + ConnectInfo must error");
        assert!(
            err.contains("ConnectInfo") && err.contains("production"),
            "missing context: {err}"
        );
        restore_env(snap);
    }

    #[test]
    fn http_settings_prod_cf_connecting_ip_ok() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("IP_SOURCE", "CfConnectingIp");
        let s = HttpSettings::from_env().expect("prod + CfConnectingIp must succeed");
        assert!(matches!(s.ip_source, ClientIpSource::CfConnectingIp));
        restore_env(snap);
    }

    #[test]
    fn http_settings_dev_connect_info_ok() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "development");
        std::env::set_var("IP_SOURCE", "ConnectInfo");
        let s = HttpSettings::from_env().expect("dev + ConnectInfo must succeed");
        assert!(matches!(s.ip_source, ClientIpSource::ConnectInfo));
        restore_env(snap);
    }

    #[test]
    fn http_settings_rejects_invalid_ip_source() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "development");
        std::env::set_var("IP_SOURCE", "cf-connecting-ip");
        let err = HttpSettings::from_env().expect_err("lowercase cf-connecting-ip must not parse");
        assert!(err.contains("Invalid IP_SOURCE"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn rate_limit_prod_requires_internal_token() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        let err = RateLimitSettings::from_env().expect_err("prod must require internal token");
        assert!(err.contains("INTERNAL_QURAN_API_TOKEN"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn rate_limit_prod_rejects_nonpositive_limits() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("INTERNAL_QURAN_API_TOKEN", "t");
        std::env::set_var("QURAN_INTERNAL_REQUESTS_PER_MINUTE", "0");
        let err = RateLimitSettings::from_env().expect_err("internal rpm=0 must error");
        assert!(err.contains("INTERNAL_REQUESTS_PER_MINUTE"), "got: {err}");

        std::env::set_var("QURAN_INTERNAL_REQUESTS_PER_MINUTE", "10");
        std::env::set_var("QURAN_HEALTH_REQUESTS_PER_MINUTE", "0");
        let err = RateLimitSettings::from_env().expect_err("health rpm=0 must error");
        assert!(err.contains("HEALTH_REQUESTS_PER_MINUTE"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn rate_limit_dev_accepts_missing_token() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "development");
        let s = RateLimitSettings::from_env().expect("dev must accept missing internal token");
        assert_eq!(s.internal_token, "");
        restore_env(snap);
    }

    #[test]
    fn rate_limit_debug_redacts_internal_token() {
        let cfg = RateLimitSettings {
            active_ban_max: 1,
            ban_export_token: "export-secret".to_string(),
            internal_token: "internal-secret".to_string(),
            internal_requests_per_minute: 600,
            health_requests_per_minute: 120,
        };
        let rendered = format!("{:?}", cfg);
        assert!(!rendered.contains("internal-secret"), "leaked: {rendered}");
        assert!(!rendered.contains("export-secret"), "leaked: {rendered}");
        assert!(
            rendered.contains("set"),
            "redaction marker missing: {rendered}"
        );
    }

    // --- W8a allowed origins (env-path boot matrix) ----------------------------
    // build_allowed_origins carries the full parse + production-policy matrix as a
    // pure function (see utils/cors.rs). These cover the thin env wrapper that the
    // boot sequence calls: it reads ALLOWED_ORIGINS + the env class and fails boot
    // on the same conditions.

    #[test]
    fn allowed_origins_production_reads_env() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var(
            "ALLOWED_ORIGINS",
            "https://easyquran.fyi,https://hmziq.rs,https://hzmiqrs.com,https://blog.hmziq.rs",
        );
        let v = crate::utils::cors::allowed_origins_from_env()
            .expect("production list with the EasyQuran origin must boot");
        assert!(v.origins().contains(&"https://easyquran.fyi".to_string()));
        // Production must not pull in localhost/LAN dev defaults.
        assert!(
            !v.origins()
                .iter()
                .any(|o| o.contains("localhost") || o.contains("192.168.")),
            "production list leaked a dev default: {:?}",
            v.origins()
        );
        restore_env(snap);
    }

    #[test]
    fn allowed_origins_production_rejects_placeholder_at_boot() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        std::env::set_var("RUST_ENV", "production");
        // Every byte is printable ASCII, so this parses cleanly — but the
        // ${...} placeholder must still fail boot before any login is rejected.
        std::env::set_var("ALLOWED_ORIGINS", "https://${DOMAIN}");
        let err = crate::utils::cors::allowed_origins_from_env()
            .expect_err("placeholder ALLOWED_ORIGINS must fail boot");
        assert!(err.contains("placeholder"), "got: {err}");
        restore_env(snap);
    }

    // --- W3a escalation config -------------------------------------------------

    fn clear_escalation_env() {
        for k in [
            "QURAN_BAN_ESCALATION_ENABLED",
            "QURAN_BAN_ALLOWLIST",
            "QURAN_ESCALATION_MAX_IDENTITIES",
            "QURAN_ACTIVE_BAN_MAX",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    fn escalation_defaults_off() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_escalation_env();
        std::env::set_var("RUST_ENV", "development");
        let cfg = EscalationConfig::from_env(2_000).unwrap();
        assert!(!cfg.enabled, "escalation must default to OFF");
        assert!(cfg.allowlist.is_empty());
        assert_eq!(cfg.key_prefix, "quran-ban");
        assert_eq!(cfg.temp_after, 5);
        assert_eq!(cfg.long_after, 20);
        assert_eq!(cfg.suspicious_4xx_per_window, 20);
        assert_eq!(cfg.max_tracked_identities, 10_000);
        assert_eq!(cfg.max_active_bans, 2_000);
        restore_env(snap);
    }

    #[test]
    fn escalation_enabled_requires_allowlist_var() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_escalation_env();
        std::env::set_var("RUST_ENV", "development");
        std::env::set_var("QURAN_BAN_ESCALATION_ENABLED", "true");
        let err =
            EscalationConfig::from_env(2_000).expect_err("enabled + unset allowlist must fail");
        assert!(err.contains("QURAN_BAN_ALLOWLIST"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn escalation_enabled_accepts_empty_allowlist_var() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_escalation_env();
        std::env::set_var("RUST_ENV", "development");
        std::env::set_var("QURAN_BAN_ESCALATION_ENABLED", "true");
        std::env::set_var("QURAN_BAN_ALLOWLIST", "");
        let cfg = EscalationConfig::from_env(2_000).unwrap();
        assert!(cfg.enabled);
        assert!(cfg.allowlist.is_empty());
        restore_env(snap);
    }

    #[test]
    fn escalation_rejects_invalid_cidr() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_escalation_env();
        std::env::set_var("RUST_ENV", "development");
        let err = parse_allowlist("not-a-cidr").expect_err("garbage must not parse");
        assert!(err.contains("invalid CIDR"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn escalation_rejects_ipv6_prefix_narrower_than_64() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_escalation_env();
        std::env::set_var("RUST_ENV", "development");
        let err = parse_allowlist("2001:db8::1/128").expect_err("/128 must be rejected");
        assert!(err.contains("narrower than /64"), "got: {err}");
        // /64 and broader (0..=64) are accepted.
        parse_allowlist("2001:db8::/64").unwrap();
        parse_allowlist("2001:db8::/48").unwrap();
        restore_env(snap);
    }

    #[test]
    fn escalation_parses_v4_and_v6_allowlist() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_escalation_env();
        std::env::set_var("RUST_ENV", "development");
        let nets = parse_allowlist("203.0.113.0/24, 2001:db8::/64,198.51.100.5/32").unwrap();
        assert_eq!(nets.len(), 3);
        restore_env(snap);
    }

    // --- W8f web-auth production gate ------------------------------------------

    fn clear_web_auth_env() {
        for k in [
            "WEB_AUTH_ENABLED",
            "WEB_OAUTH_PROVIDERS",
            "MAIL_PROVIDER",
            "MAIL_FROM_ADDRESS",
            "MAIL_FROM_NAME",
            "SMTP_HOST",
            "SMTP_USERNAME",
            "SMTP_PASSWORD",
            "CLOUDFLARE_EMAIL_ACCOUNT_ID",
            "CLOUDFLARE_EMAIL_API_TOKEN",
            "FRONTEND_URL",
            "OAUTH_ALLOWED_REDIRECT_ORIGINS",
            "GOOGLE_CLIENT_ID",
            "GOOGLE_CLIENT_SECRET",
            "GOOGLE_REDIRECT_URI",
            "FACEBOOK_CLIENT_ID",
            "FACEBOOK_CLIENT_SECRET",
            "FACEBOOK_REDIRECT_URI",
            "GITHUB_CLIENT_ID",
            "GITHUB_CLIENT_SECRET",
            "GITHUB_REDIRECT_URI",
            "APPLE_CLIENT_ID",
            "APPLE_TEAM_ID",
            "APPLE_KEY_ID",
            "APPLE_PRIVATE_KEY",
            "APPLE_PRIVATE_KEY_PATH",
            "APPLE_REDIRECT_URI",
        ] {
            std::env::remove_var(k);
        }
    }

    /// Set a complete, valid SMTP mail config so the W8f mail-transport gate
    /// passes and a test can exercise a later check (oauth creds / redirect
    /// origin / boot success). Complement to clear_web_auth_env.
    fn set_valid_smtp_mail() {
        std::env::set_var("MAIL_FROM_ADDRESS", "no-reply@example.com");
        std::env::set_var("MAIL_FROM_NAME", "EasyQuran");
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_USERNAME", "u");
        std::env::set_var("SMTP_PASSWORD", "p");
    }

    #[test]
    fn web_auth_defaults_off_when_unset() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "development");
        let cfg = WebAuthSettings::from_env().expect("defaults must succeed");
        assert!(!cfg.enabled, "WEB_AUTH_ENABLED must default to false");
        assert!(cfg.oauth_providers.is_empty());
        assert!(cfg.providers_status.is_empty());
        restore_env(snap);
    }

    #[test]
    fn web_auth_providers_status_reports_ready_when_creds_present() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "development");
        std::env::set_var("WEB_OAUTH_PROVIDERS", "google, github");
        std::env::set_var("GOOGLE_CLIENT_ID", "g");
        std::env::set_var("GOOGLE_CLIENT_SECRET", "s");
        std::env::set_var("GOOGLE_REDIRECT_URI", "https://easyquran.fyi/cb");
        let cfg = WebAuthSettings::from_env().unwrap();
        let google = cfg
            .providers_status
            .iter()
            .find(|p| p.name == "google")
            .unwrap();
        assert!(google.ready, "google has all creds → ready");
        let github = cfg
            .providers_status
            .iter()
            .find(|p| p.name == "github")
            .unwrap();
        assert!(!github.ready, "github has no creds → not ready");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_mail_none_when_auth_enabled() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "none");
        let err = WebAuthSettings::from_env().expect_err("prod+auth+MAIL=none must fail");
        assert!(err.contains("MAIL_PROVIDER=none"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_missing_provider_creds() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        set_valid_smtp_mail();
        std::env::set_var("WEB_OAUTH_PROVIDERS", "google");
        let err = WebAuthSettings::from_env().expect_err("missing google creds must fail");
        assert!(err.contains("google"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_redirect_origin_mismatch() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        set_valid_smtp_mail();
        std::env::set_var("WEB_OAUTH_PROVIDERS", "google");
        std::env::set_var("GOOGLE_CLIENT_ID", "g");
        std::env::set_var("GOOGLE_CLIENT_SECRET", "s");
        // evil.example is neither in the allowlist nor FRONTEND_URL.
        std::env::set_var("GOOGLE_REDIRECT_URI", "https://evil.example/cb");
        std::env::set_var("OAUTH_ALLOWED_REDIRECT_ORIGINS", "https://easyquran.fyi");
        std::env::set_var("FRONTEND_URL", "https://easyquran.fyi");
        let err = WebAuthSettings::from_env().expect_err("redirect-origin mismatch must fail");
        assert!(err.contains("GOOGLE_REDIRECT_URI"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_accepts_matched_redirect_origin() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        set_valid_smtp_mail();
        std::env::set_var("WEB_OAUTH_PROVIDERS", "google");
        std::env::set_var("GOOGLE_CLIENT_ID", "g");
        std::env::set_var("GOOGLE_CLIENT_SECRET", "s");
        std::env::set_var("GOOGLE_REDIRECT_URI", "https://easyquran.fyi/api/cb");
        std::env::set_var("OAUTH_ALLOWED_REDIRECT_ORIGINS", "https://easyquran.fyi");
        std::env::set_var("FRONTEND_URL", "https://easyquran.fyi");
        let cfg = WebAuthSettings::from_env().expect("matched origin must boot");
        assert!(cfg.enabled);
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_http_allowed_redirect_origin() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        set_valid_smtp_mail();
        // An http:// origin in the allowlist must fail boot even with no oauth
        // providers — the HTTPS scheme is enforced before membership is considered.
        std::env::set_var("OAUTH_ALLOWED_REDIRECT_ORIGINS", "http://easyquran.fyi");
        let err = WebAuthSettings::from_env().expect_err("http allowed origin must fail");
        assert!(err.contains("HTTPS"), "got: {err}");
        assert!(err.contains("OAUTH_ALLOWED_REDIRECT_ORIGINS"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_http_frontend_url() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        set_valid_smtp_mail();
        std::env::set_var("OAUTH_ALLOWED_REDIRECT_ORIGINS", "https://easyquran.fyi");
        std::env::set_var("FRONTEND_URL", "http://easyquran.fyi");
        let err = WebAuthSettings::from_env().expect_err("http FRONTEND_URL must fail");
        assert!(err.contains("HTTPS"), "got: {err}");
        assert!(err.contains("FRONTEND_URL"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_missing_frontend_url() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        set_valid_smtp_mail();
        std::env::set_var("OAUTH_ALLOWED_REDIRECT_ORIGINS", "https://easyquran.fyi");
        // FRONTEND_URL intentionally left unset: the HTTPS-scheme check is a no-op
        // on an empty origin, so a presence gate must catch it before boot succeeds.
        let err = WebAuthSettings::from_env().expect_err("missing FRONTEND_URL must fail");
        assert!(err.contains("FRONTEND_URL"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_empty_mail_from_address() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        // MAIL_FROM_ADDRESS intentionally left unset; from-name + smtp creds set
        // so the only failing check is the from-address requirement.
        std::env::set_var("MAIL_FROM_NAME", "EasyQuran");
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_USERNAME", "u");
        std::env::set_var("SMTP_PASSWORD", "p");
        let err = WebAuthSettings::from_env().expect_err("empty MAIL_FROM_ADDRESS must fail");
        assert!(err.contains("MAIL_FROM_ADDRESS"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_empty_mail_from_name() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        std::env::set_var("MAIL_FROM_ADDRESS", "no-reply@example.com");
        // MAIL_FROM_NAME intentionally left unset.
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_USERNAME", "u");
        std::env::set_var("SMTP_PASSWORD", "p");
        let err = WebAuthSettings::from_env().expect_err("empty MAIL_FROM_NAME must fail");
        assert!(err.contains("MAIL_FROM_NAME"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_smtp_missing_creds() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "smtp");
        std::env::set_var("MAIL_FROM_ADDRESS", "no-reply@example.com");
        std::env::set_var("MAIL_FROM_NAME", "EasyQuran");
        // SMTP creds intentionally unset.
        let err = WebAuthSettings::from_env().expect_err("missing SMTP creds must fail");
        assert!(err.contains("SMTP_HOST"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_rejects_cloudflare_missing_creds() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "cloudflare");
        std::env::set_var("MAIL_FROM_ADDRESS", "no-reply@example.com");
        std::env::set_var("MAIL_FROM_NAME", "EasyQuran");
        // CLOUDFLARE_EMAIL_* creds intentionally unset.
        let err = WebAuthSettings::from_env().expect_err("missing CF creds must fail");
        assert!(err.contains("CLOUDFLARE_EMAIL_ACCOUNT_ID"), "got: {err}");
        restore_env(snap);
    }

    #[test]
    fn prod_accepts_cloudflare_with_creds() {
        let _g = TEST_ENV_MUTEX.lock().unwrap();
        let snap = snapshot_env();
        clear_env_vars();
        clear_web_auth_env();
        std::env::set_var("RUST_ENV", "production");
        std::env::set_var("WEB_AUTH_ENABLED", "true");
        std::env::set_var("MAIL_PROVIDER", "cloudflare");
        std::env::set_var("MAIL_FROM_ADDRESS", "no-reply@example.com");
        std::env::set_var("MAIL_FROM_NAME", "EasyQuran");
        std::env::set_var("CLOUDFLARE_EMAIL_ACCOUNT_ID", "acc");
        std::env::set_var("CLOUDFLARE_EMAIL_API_TOKEN", "tok");
        std::env::set_var("FRONTEND_URL", "https://easyquran.fyi");
        // No oauth providers configured → oauth loop is a no-op → boot succeeds.
        let cfg = WebAuthSettings::from_env().expect("cloudflare with creds must boot");
        assert!(cfg.enabled);
        restore_env(snap);
    }

    #[test]
    fn origin_of_parses_scheme_host_port() {
        assert_eq!(
            origin_of("https://easyquran.fyi/cb"),
            "https://easyquran.fyi"
        );
        assert_eq!(
            origin_of("https://easyquran.fyi:8443/a/b"),
            "https://easyquran.fyi:8443"
        );
        assert_eq!(origin_of("http://localhost:8080"), "http://localhost:8080");
        assert_eq!(origin_of("not-a-url"), "");
        assert_eq!(origin_of("ftp://example.com"), "");
    }
}
