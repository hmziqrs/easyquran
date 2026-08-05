use axum::extract::FromRef;
use sea_orm::DatabaseConnection;

use crate::config::{ObjectStorageConfig, Settings};
use crate::services::auth::AuthBackend;
use crate::services::session_store::SqliteSessionStore;

use crate::services::billing::BillingRouter;

use crate::services::image_moderation::ImageModerator;
use rux_fcm::FcmClient;

pub use crate::config::OptimizerConfig;

pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("building shared reqwest::Client with timeouts must not fail")
}

#[derive(Clone)]
pub struct StorageState {
    pub config: ObjectStorageConfig,
    pub client: aws_sdk_s3::Client,
    pub optimizer: OptimizerConfig,
    pub image_moderator: Option<std::sync::Arc<dyn ImageModerator + Send + Sync>>,
}

#[derive(Clone, Copy, Debug)]
pub struct QuranRuntimeMetrics {
    pub arabic_load_duration_ms: u64,
    pub translation_catalogue_load_duration_ms: u64,
    pub translation_catalogue_entries: u64,
}

#[derive(Clone)]
pub struct AppState {
    pub sea_db: DatabaseConnection,
    pub gate_store: std::sync::Arc<rux_request_gate::InMemoryStore>,
    pub session_store: std::sync::Arc<SqliteSessionStore>,
    pub revoked_sessions: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    pub mailer: std::sync::Arc<crate::services::mail::MailRouter>,
    pub settings: std::sync::Arc<Settings>,
    pub storage: StorageState,
    pub secret_key: Vec<u8>,
    pub http_client: reqwest::Client,
    pub billing_router: std::sync::Arc<BillingRouter>,
    pub fcm: Option<std::sync::Arc<FcmClient>>,
    pub webauthn: Option<std::sync::Arc<crate::services::webauthn::WebauthnService>>,
    pub quran: std::sync::Arc<crate::quran::QuranStore>,
    pub quran_runtime_metrics: QuranRuntimeMetrics,
    pub quran_scripts:
        std::sync::Arc<tokio::sync::Mutex<Option<Vec<crate::modules::quran_v1::dto::Artifact>>>>,
    pub translation_pool: std::sync::Arc<crate::quran::TranslationPool>,
    pub quran_sources:
        std::sync::Arc<tokio::sync::Mutex<Option<Vec<crate::modules::quran_v1::dto::SourceDto>>>>,
}

impl FromRef<AppState> for AuthBackend {
    fn from_ref(state: &AppState) -> Self {
        AuthBackend::new(
            &state.sea_db,
            state.session_store.clone(),
            state.revoked_sessions.clone(),
        )
    }
}

pub const KNOWN_COOKIE_KEY_PLACEHOLDERS: &[&str] =
    &["CHANGE_ME_rotate_me_generate_with_openssl_rand_hex_32___"];

pub fn validate_cookie_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err(
            "COOKIE_KEY is empty or whitespace. Generate with: openssl rand -hex 32".to_string(),
        );
    }

    if KNOWN_COOKIE_KEY_PLACEHOLDERS.contains(&key) {
        return Err(
            "COOKIE_KEY is the known placeholder value shipped in committed .env files \
             — production must NOT boot on a publicly-known key. \
             Generate a per-env key with: openssl rand -hex 32. \
             See CRYPTO_AUDIT.md V-CRIT-1."
                .to_string(),
        );
    }

    if key.len() < 32 {
        return Err(format!(
            "COOKIE_KEY must be >= 32 bytes of CSPRNG output (got {}). \
             Generate with: openssl rand -hex 32.",
            key.len()
        ));
    }

    Ok(())
}

pub const FIELD_ENC_KEY_DEV_DEFAULT: &[u8] = b"ruxlog_dev_field_enc_key_do_not_"; // exactly 32 bytes

pub fn derive_field_enc_key() -> [u8; 32] {
    let raw = std::env::var("FIELD_ENC_KEY").ok();
    let is_prod = !matches!(
        std::env::var("RUST_ENV")
            .or_else(|_| std::env::var("NODE_ENV"))
            .or_else(|_| std::env::var("APP_ENV"))
            .as_deref()
            .ok(),
        Some("development" | "dev" | "test" | "testing" | "ci" | "local")
    );

    let key_bytes: Vec<u8> = match raw {
        Some(s) if !s.trim().is_empty() => s.into_bytes(),
        _ => {
            if is_prod {
                panic!(
                    "FIELD_ENC_KEY is not set and this looks like production. \
                     Generate a 32-byte key with: openssl rand -base64 32 \
                     (then take 32 raw bytes) and export it as FIELD_ENC_KEY. \
                     See CRYPTO_AUDIT.md V-MED-11."
                );
            }
            tracing::warn!(
                "FIELD_ENC_KEY unset in dev/test — using the documented \
                 non-secret dev default. DO NOT use in production."
            );
            FIELD_ENC_KEY_DEV_DEFAULT.to_vec()
        }
    };

    if key_bytes.len() != 32 {
        panic!(
            "FIELD_ENC_KEY must be exactly 32 bytes for AES-256 (got {}). \
             Export 32 raw bytes, e.g. FIELD_ENC_KEY=\"$(openssl rand -base64 32 \
             | head -c 32)\". See CRYPTO_AUDIT.md V-MED-11.",
            key_bytes.len()
        );
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&key_bytes);
    arr
}

pub fn load_field_enc_key() -> [u8; 32] {
    let arr = derive_field_enc_key();
    if let Err(reason) = crate::utils::field_crypto::set_key(&arr) {
        panic!("{}", reason);
    }

    // FIELD_ENC_KEY_PREV installs the decrypt-only previous key for rolling rotation; removing it orphans fields encrypted before the rotation.
    let prev_raw = std::env::var("FIELD_ENC_KEY_PREV")
        .ok()
        .filter(|s| !s.trim().is_empty());
    if let Err(reason) =
        crate::utils::field_crypto::set_previous_key(prev_raw.as_deref().map(|s| s.as_bytes()))
    {
        panic!("{}", reason);
    }

    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_object_storage() -> ObjectStorageConfig {
        ObjectStorageConfig {
            region: "auto".to_string(),
            account_id: "acct_123".to_string(),
            bucket: "my-bucket".to_string(),
            access_key: "AKIA_SECRETVASDF123456".to_string(),
            secret_key: "super_secret_do_not_leak_value_xyz".to_string(),
            public_url: "https://cdn.example.com".to_string(),
            endpoint: "https://s3.example.com".to_string(),
        }
    }

    #[test]
    fn object_storage_debug_redacts_secrets() {
        let cfg = sample_object_storage();
        let rendered = format!("{:?}", cfg);

        assert!(
            !rendered.contains("AKIA_SECRETVASDF123456"),
            "access_key leaked into Debug output: {}",
            rendered
        );
        assert!(
            !rendered.contains("super_secret_do_not_leak_value_xyz"),
            "secret_key leaked into Debug output: {}",
            rendered
        );
        assert!(
            rendered.contains("my-bucket"),
            "non-secret field bucket missing: {}",
            rendered
        );
        assert!(
            rendered.contains("<redacted>"),
            "redaction marker missing: {}",
            rendered
        );
    }

    #[test]
    fn cookie_key_rejects_known_placeholder() {
        let err = validate_cookie_key("CHANGE_ME_rotate_me_generate_with_openssl_rand_hex_32___")
            .expect_err("known placeholder must be rejected");
        assert!(
            err.contains("placeholder"),
            "error should explain it is a placeholder: {}",
            err
        );
    }

    #[test]
    fn cookie_key_rejects_short() {
        let err = validate_cookie_key("shortkey").expect_err("short key must be rejected");
        assert!(
            err.contains("32"),
            "error should mention the 32-byte minimum: {}",
            err
        );
    }

    #[test]
    fn cookie_key_rejects_empty_and_whitespace() {
        validate_cookie_key("").expect_err("empty key must be rejected");
        validate_cookie_key("    \t\n  ").expect_err("whitespace-only key must be rejected");
    }

    #[test]
    fn cookie_key_accepts_strong_hex() {
        let strong = "9f3a7c1e4b6d820f5a4c6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b6d8f";
        validate_cookie_key(strong).expect("a strong 64-hex key must be accepted");
    }

    #[test]
    fn cookie_key_accepts_exactly_32_bytes() {
        validate_cookie_key("0123456789abcdef0123456789abcdef")
            .expect("32-byte key must be accepted");
    }

    #[test]
    fn http_client_has_nonzero_timeouts() {
        let _client = build_http_client();

        let connect = std::time::Duration::from_secs(5);
        let request = std::time::Duration::from_secs(15);
        let pool_idle = std::time::Duration::from_secs(30);
        assert!(!connect.is_zero(), "connect_timeout must be non-zero");
        assert!(!request.is_zero(), "request timeout must be non-zero");
        assert!(!pool_idle.is_zero(), "pool_idle_timeout must be non-zero");

        let _ = reqwest::Client::builder()
            .connect_timeout(connect)
            .timeout(request)
            .pool_idle_timeout(pool_idle)
            .build()
            .expect("builder with timeouts must construct a client");
    }

    #[test]
    fn field_enc_key_dev_default_is_32_bytes() {
        assert_eq!(
            FIELD_ENC_KEY_DEV_DEFAULT.len(),
            32,
            "dev default must be exactly 32 bytes for AES-256"
        );
    }

    #[test]
    fn field_enc_key_accepts_32_byte_value() {
        let prev = std::env::var("FIELD_ENC_KEY").ok();
        let prev_env = std::env::var("RUST_ENV").ok();
        let key: Vec<u8> = (1..=32u8).collect();
        std::env::set_var("FIELD_ENC_KEY", String::from_utf8(key.clone()).unwrap());
        std::env::set_var("RUST_ENV", "test");

        let loaded = derive_field_enc_key();
        assert_eq!(loaded.as_slice(), key.as_slice());

        match prev {
            Some(v) => std::env::set_var("FIELD_ENC_KEY", v),
            None => std::env::remove_var("FIELD_ENC_KEY"),
        }
        match prev_env {
            Some(v) => std::env::set_var("RUST_ENV", v),
            None => std::env::remove_var("RUST_ENV"),
        }
    }

    #[test]
    fn field_enc_key_rejects_wrong_length() {
        let prev = std::env::var("FIELD_ENC_KEY").ok();
        std::env::set_var("FIELD_ENC_KEY", "too-short");

        let result = std::panic::catch_unwind(load_field_enc_key);
        assert!(
            result.is_err(),
            "load_field_enc_key must panic on a non-32-byte key"
        );

        match prev {
            Some(v) => std::env::set_var("FIELD_ENC_KEY", v),
            None => std::env::remove_var("FIELD_ENC_KEY"),
        }
    }
}
