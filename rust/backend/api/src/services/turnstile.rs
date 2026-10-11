use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::error::{ErrorCode, ErrorResponse};

const DEFAULT_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct SiteverifyResponse {
    success: bool,
}

/// Production signup requires captcha; development enables it when configured.
pub fn turnstile_enabled() -> bool {
    secret().is_some() || crate::config::settings::is_production().unwrap_or(true)
}

fn secret() -> Option<SecretString> {
    std::env::var("TURNSTILE_SECRET_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(SecretString::from)
}

fn verify_url() -> String {
    std::env::var("TURNSTILE_VERIFY_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_VERIFY_URL.to_string())
}

/// Requires siteverify in production or when configured; configuration and transport failures deny signup.
pub async fn verify_registration(
    token: Option<&str>,
    remote_ip: Option<&str>,
) -> Result<(), ErrorResponse> {
    let Some(secret) = secret() else {
        if crate::config::settings::is_production().unwrap_or(true) {
            return Err(captcha_unavailable());
        }
        return Ok(());
    };
    let token = token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            ErrorResponse::new(ErrorCode::InvalidInput)
                .with_message("Captcha verification required")
        })?;
    verify_with(&secret, token, remote_ip).await
}

async fn verify_with(
    secret: &SecretString,
    token: &str,
    remote_ip: Option<&str>,
) -> Result<(), ErrorResponse> {
    let client = reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .build()
        .map_err(|_| captcha_unavailable())?;
    let mut form = vec![
        ("secret", secret.expose_secret().to_string()),
        ("response", token.to_string()),
    ];
    if let Some(ip) = remote_ip {
        form.push(("remoteip", ip.to_string()));
    }
    let response = client
        .post(verify_url())
        .form(&form)
        .send()
        .await
        .map_err(|_| captcha_unavailable())?;
    if !response.status().is_success() {
        return Err(captcha_unavailable());
    }
    let body: SiteverifyResponse = response.json().await.map_err(|_| captcha_unavailable())?;
    if body.success {
        Ok(())
    } else {
        Err(ErrorResponse::new(ErrorCode::InvalidInput).with_message("Captcha verification failed"))
    }
}

fn captcha_unavailable() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::ServiceUnavailable)
        .with_message("Captcha verification unavailable, try again")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::TEST_ENV_MUTEX;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        secret: Option<String>,
        app_env: Option<String>,
        verify_url: Option<String>,
    }

    impl EnvGuard {
        fn set(secret: Option<&str>, verify_url: Option<&str>) -> Self {
            let lock = TEST_ENV_MUTEX.lock().unwrap();
            let guard = Self {
                _lock: lock,
                secret: std::env::var("TURNSTILE_SECRET_KEY").ok(),
                app_env: std::env::var("APP_ENV").ok(),
                verify_url: std::env::var("TURNSTILE_VERIFY_URL").ok(),
            };
            std::env::set_var("APP_ENV", "development");
            match secret {
                Some(value) => std::env::set_var("TURNSTILE_SECRET_KEY", value),
                None => std::env::remove_var("TURNSTILE_SECRET_KEY"),
            }
            match verify_url {
                Some(value) => std::env::set_var("TURNSTILE_VERIFY_URL", value),
                None => std::env::remove_var("TURNSTILE_VERIFY_URL"),
            }
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.app_env {
                Some(value) => std::env::set_var("APP_ENV", value),
                None => std::env::remove_var("APP_ENV"),
            }
            match &self.secret {
                Some(value) => std::env::set_var("TURNSTILE_SECRET_KEY", value),
                None => std::env::remove_var("TURNSTILE_SECRET_KEY"),
            }
            match &self.verify_url {
                Some(value) => std::env::set_var("TURNSTILE_VERIFY_URL", value),
                None => std::env::remove_var("TURNSTILE_VERIFY_URL"),
            }
        }
    }

    #[tokio::test]
    async fn unconfigured_gate_allows_missing_token() {
        let _env = EnvGuard::set(None, None);
        assert!(!turnstile_enabled());
        assert!(verify_registration(None, None).await.is_ok());
    }

    #[tokio::test]
    async fn production_registration_rejects_unconfigured_captcha() {
        let _env = EnvGuard::set(None, None);
        std::env::set_var("APP_ENV", "production");
        assert!(turnstile_enabled());
        let error = verify_registration(None, None).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::ServiceUnavailable);
    }

    #[tokio::test]
    async fn configured_gate_requires_token() {
        let _env = EnvGuard::set(Some("test-secret"), None);
        assert!(turnstile_enabled());
        let err = verify_registration(None, None).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        let err = verify_registration(Some("   "), None).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn siteverify_success_and_rejection_are_mapped() {
        for (success, expected) in [(true, None), (false, Some(ErrorCode::InvalidInput))] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/siteverify"))
                .and(body_string_contains("secret=test-secret"))
                .and(body_string_contains("response=widget-token"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({ "success": success })),
                )
                .mount(&server)
                .await;
            let _env = EnvGuard::set(
                Some("test-secret"),
                Some(&format!("{}/siteverify", server.uri())),
            );

            let result = verify_registration(Some("widget-token"), Some("203.0.113.7")).await;
            match expected {
                None => assert!(result.is_ok(), "expected success"),
                Some(code) => assert_eq!(result.unwrap_err().code, code),
            }
        }
    }

    #[tokio::test]
    async fn transport_failure_fails_closed() {
        let _env = EnvGuard::set(Some("test-secret"), Some("http://127.0.0.1:1/siteverify"));
        let err = verify_registration(Some("widget-token"), None)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ServiceUnavailable);
    }
}
