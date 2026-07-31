use axum_client_ip::ClientIpSource;

use crate::config::env::{env_bool, env_u64, env_u8, env_with_fallback};

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
        let public_url =
            env_with_fallback(&["S3_PUBLIC_URL", "AWS_S3_PUBLIC_URL"], None)
                .unwrap_or_else(|| {
                    endpoint.clone()
                });
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

pub struct HttpSettings {
    pub host: String,
    pub port: String,
    pub ip_source: ClientIpSource,
    pub cookie_secure: bool,
}

impl HttpSettings {
    pub fn from_env() -> Self {
        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("PORT").unwrap_or_else(|_| "8888".to_string());
        let ip_source = std::env::var("IP_SOURCE")
            .unwrap_or_else(|_| "ConnectInfo".to_string())
            .parse::<ClientIpSource>()
            .unwrap_or_else(|e| {
                panic!(
                    "Invalid IP_SOURCE value: {e}. Valid: ConnectInfo (default, TCP peer) | \
                     CfConnectingIp (Cloudflare CF-Connecting-IP — use behind Cloudflare→Traefik) \
                     | XRealIp | TrueClientIp | FlyClientIp | RightmostXForwardedFor | \
                     RightmostForwarded | CloudFrontViewerAddress."
                )
            });
        let cookie_secure = env_bool("COOKIE_SECURE", true);
        Self {
            host,
            port,
            ip_source,
            cookie_secure,
        }
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
            url: std::env::var("SITE_URL")
                .unwrap_or_else(|_| "http://localhost:8888".to_string()),
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
    /// Assertion only — never the source of the content version value.
    pub expected_content_version: Option<String>,
}

impl QuranSettings {
    pub fn from_env() -> Self {
        Self {
            uthmani_path: std::env::var("QURAN_UTHMANI_PATH").unwrap_or_else(|_| {
                "db/quran/tanzil/arabic/quran-uthmani.sqlite".to_string()
            }),
            simple_clean_path: std::env::var("QURAN_SIMPLE_CLEAN_PATH").unwrap_or_else(|_| {
                "db/quran/tanzil/arabic/quran-simple-clean.sqlite".to_string()
            }),
            metadata_xml_path: std::env::var("QURAN_METADATA_XML_PATH")
                .unwrap_or_else(|_| "db/quran/tanzil/quran-data.xml".to_string()),
            expected_content_version: std::env::var("QURAN_CONTENT_VERSION").ok(),
        }
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
}

impl Settings {
    pub fn from_env() -> Self {
        let cookie_key = std::env::var("COOKIE_KEY").expect("COOKIE_KEY must be set");
        if let Err(reason) = crate::state::validate_cookie_key(&cookie_key) {
            panic!("{}", reason);
        }

        let object_storage = ObjectStorageConfig::from_env();

        let optimizer = OptimizerConfig::from_env();

        let http = HttpSettings::from_env();
        let site = SiteSettings::from_env();
        let quran = QuranSettings::from_env();

        Self {
            cookie_key,
            http,
            site,
            object_storage,
            optimizer,
            quran,
        }
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
        let prev = std::env::var("COOKIE_KEY").ok();
        set_cookie_key("CHANGE_ME_rotate_me_generate_with_openssl_rand_hex_32___");

        let result = std::panic::catch_unwind(Settings::from_env);
        assert!(
            result.is_err(),
            "Settings::from_env must panic on the known COOKIE_KEY placeholder"
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
}
