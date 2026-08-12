use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use chrono::Utc;
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{error, warn};

use crate::error::{ErrorCode, ErrorResponse};

use super::validator::AppleIdTokenClaims;

const APPLE_AUTH_URL: &str = "https://appleid.apple.com/auth/authorize";
const APPLE_TOKEN_URL: &str = "https://appleid.apple.com/auth/token";
const APPLE_JWKS_URL: &str = "https://appleid.apple.com/auth/keys";
const APPLE_ISSUER: &str = "https://appleid.apple.com";
const APPLE_JWKS_MAX_BYTES: usize = 64 * 1024;
const APPLE_JWKS_TTL: Duration = Duration::from_secs(3600);

struct CachedAppleJwks {
    fetched_at: SystemTime,
    keys: Vec<AppleJwkKey>,
}

static APPLE_JWKS_CACHE: LazyLock<RwLock<Option<CachedAppleJwks>>> =
    LazyLock::new(|| RwLock::new(None));

pub struct AppleConfig {
    pub client_id: String,
    pub team_id: String,
    pub key_id: String,
    pub redirect_uri: String,
    pub private_key_pem: String,
}

pub fn apple_token_audiences(primary: &str, extras: Option<&str>) -> Vec<String> {
    let mut audiences = Vec::new();
    for audience in
        std::iter::once(primary).chain(extras.into_iter().flat_map(|value| value.split(',')))
    {
        let audience = audience.trim();
        if !audience.is_empty() && !audiences.iter().any(|existing| existing == audience) {
            audiences.push(audience.to_string());
        }
    }
    audiences
}

#[allow(clippy::result_large_err)]
pub fn load_apple_token_audiences() -> Result<Vec<String>, ErrorResponse> {
    let primary = std::env::var("APPLE_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("APPLE_CLIENT_ID not configured")
    })?;
    let extras = std::env::var("APPLE_MOBILE_CLIENT_IDS").ok();
    let audiences = apple_token_audiences(&primary, extras.as_deref());
    if audiences.is_empty() {
        return Err(ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("APPLE_CLIENT_ID not configured"));
    }
    Ok(audiences)
}

#[allow(clippy::result_large_err)]
pub fn load_apple_config() -> Result<AppleConfig, ErrorResponse> {
    let client_id = std::env::var("APPLE_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("APPLE_CLIENT_ID not configured")
    })?;
    let team_id = std::env::var("APPLE_TEAM_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("APPLE_TEAM_ID not configured")
    })?;
    let key_id = std::env::var("APPLE_KEY_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("APPLE_KEY_ID not configured")
    })?;
    let redirect_uri = std::env::var("APPLE_REDIRECT_URI").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("APPLE_REDIRECT_URI not configured")
    })?;

    let private_key_pem = match std::env::var("APPLE_PRIVATE_KEY") {
        Ok(raw) if !raw.trim().is_empty() => unescape_pem_newlines(&raw),
        _ => {
            let path = std::env::var("APPLE_PRIVATE_KEY_PATH").map_err(|_| {
                ErrorResponse::new(ErrorCode::InternalServerError)
                    .with_message("APPLE_PRIVATE_KEY or APPLE_PRIVATE_KEY_PATH not configured")
            })?;
            std::fs::read_to_string(&path).map_err(|e| {
                ErrorResponse::new(ErrorCode::InternalServerError)
                    .with_message("Failed to read Apple private key file")
                    .with_details(e.to_string())
            })?
        }
    };

    Ok(AppleConfig {
        client_id,
        team_id,
        key_id,
        redirect_uri,
        private_key_pem,
    })
}

fn unescape_pem_newlines(raw: &str) -> String {
    if raw.contains("\\n") {
        raw.replace("\\n", "\n")
    } else {
        raw.to_string()
    }
}

#[allow(clippy::result_large_err)]
pub fn mint_apple_client_secret(cfg: &AppleConfig) -> Result<String, ErrorResponse> {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(cfg.key_id.clone());

    let now = Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": cfg.team_id,
        "iat": now,
        "exp": now + 157_77000,
        "aud": APPLE_ISSUER,
        "sub": cfg.client_id,
    });

    let encoding_key = EncodingKey::from_ec_pem(cfg.private_key_pem.as_bytes()).map_err(|e| {
        error!(error = ?e, "Failed to parse Apple EC private key");
        ErrorResponse::new(ErrorCode::ConfigurationError)
            .with_message("Invalid Apple private key (expected P-256 PEM)")
            .with_details(e.to_string())
    })?;

    encode(&header, &claims, &encoding_key).map_err(|e| {
        error!(error = ?e, "Failed to sign Apple client_secret JWT");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to mint Apple client_secret")
            .with_details(e.to_string())
    })
}

pub fn build_apple_authorize_url(
    cfg: &AppleConfig,
    state_secret: &str,
    pkce_challenge: &str,
    nonce: &str,
) -> Result<String, ErrorResponse> {
    let mut url = reqwest::Url::parse(APPLE_AUTH_URL).map_err(|e| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Invalid Apple auth URL")
            .with_details(e.to_string())
    })?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("response_mode", "query");
        q.append_pair("client_id", &cfg.client_id);
        q.append_pair("redirect_uri", &cfg.redirect_uri);
        q.append_pair("state", state_secret);
        q.append_pair("scope", "name email");
        q.append_pair("code_challenge", pkce_challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("nonce", nonce);
    }
    Ok(url.to_string())
}

#[derive(Debug, Deserialize)]
pub struct AppleTokenResponse {
    #[allow(dead_code)]
    pub access_token: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
    pub id_token: String,
}

pub async fn exchange_apple_code(
    http_client: &reqwest::Client,
    cfg: &AppleConfig,
    code: &str,
    client_secret_jwt: &str,
    code_verifier: Option<&str>,
) -> Result<AppleTokenResponse, ErrorResponse> {
    let mut form_params: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", cfg.redirect_uri.as_str()),
        ("client_id", cfg.client_id.as_str()),
        ("client_secret", client_secret_jwt),
    ];
    if let Some(verifier) = code_verifier {
        form_params.push(("code_verifier", verifier));
    }

    let resp = http_client
        .post(APPLE_TOKEN_URL)
        .form(&form_params)
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to POST Apple token endpoint");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange Apple authorization code")
        })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        error!(status = %status, body = %body, "Apple token endpoint returned non-2xx");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Apple rejected the authorization code")
            .with_details(body));
    }

    resp.json::<AppleTokenResponse>().await.map_err(|e| {
        error!(error = ?e, "Failed to parse Apple token response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse Apple token response")
    })
}

#[derive(Clone, Debug, Deserialize)]
struct AppleJwkKey {
    kid: Option<String>,
    n: String,
    e: String,
}

#[derive(Debug, Deserialize)]
struct AppleJwkSet {
    keys: Vec<AppleJwkKey>,
}

#[allow(clippy::result_large_err)]
pub async fn verify_apple_id_token(
    id_token: &str,
    expected_aud: &[&str],
    expected_nonce: Option<&str>,
) -> Result<AppleIdTokenClaims, ErrorResponse> {
    let header = decode_header(id_token).map_err(|e| {
        warn!(error = ?e, "Apple id_token header decode failed");
        ErrorResponse::new(ErrorCode::InvalidToken).with_message("Malformed Apple id_token")
    })?;
    if header.alg != Algorithm::RS256 {
        warn!(alg = ?header.alg, "Rejecting Apple id_token with non-RS256 alg");
        return Err(ErrorResponse::new(ErrorCode::InvalidToken)
            .with_message("Unsupported Apple id_token algorithm"));
    }

    let keys = fetch_apple_jwks().await?;
    let signing_key = match header
        .kid
        .as_deref()
        .and_then(|kid| keys.iter().find(|k| k.kid.as_deref() == Some(kid)))
        .cloned()
    {
        Some(key) => key,
        None => {
            warn!("Apple id_token kid not in cached JWKS; forcing one refresh");
            let refreshed = fetch_apple_jwks_bypass_cache().await?;
            header
                .kid
                .as_deref()
                .and_then(|kid| refreshed.iter().find(|key| key.kid.as_deref() == Some(kid)))
                .cloned()
                .ok_or_else(|| {
                    warn!("No matching Apple signing key for id_token kid");
                    ErrorResponse::new(ErrorCode::InvalidToken)
                        .with_message("Untrusted Apple id_token signer")
                })?
        }
    };

    let decoding_key =
        DecodingKey::from_rsa_components(&signing_key.n, &signing_key.e).map_err(|e| {
            error!(error = ?e, "Failed to build RSA decoding key from Apple JWKS");
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid Apple signing key")
        })?;

    let validation = apple_id_token_validation(expected_aud);

    let token_data =
        decode::<AppleIdTokenClaims>(id_token, &decoding_key, &validation).map_err(|e| {
            warn!(error = ?e, "Apple id_token verification failed");
            ErrorResponse::new(ErrorCode::InvalidToken).with_message("Invalid Apple id_token")
        })?;

    if let Some(expected) = expected_nonce {
        match &token_data.claims.nonce {
            Some(actual) if actual == expected => { /* bound */ }
            other => {
                warn!(
                    ?other,
                    "Apple id_token nonce missing or mismatched — rejecting login"
                );
                return Err(ErrorResponse::new(ErrorCode::InvalidToken)
                    .with_message("Invalid Apple id_token"));
            }
        }
    }

    Ok(token_data.claims)
}

fn apple_id_token_validation(expected_aud: &[&str]) -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.set_audience(expected_aud);
    validation.set_issuer(&[APPLE_ISSUER]);
    validation
}

fn http_client_for_jwks() -> Result<reqwest::Client, ErrorResponse> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| {
            error!(error = ?e, "Failed to build Apple JWKS HTTP client");
            ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
        })
}

async fn fetch_apple_jwks() -> Result<Vec<AppleJwkKey>, ErrorResponse> {
    {
        let guard = APPLE_JWKS_CACHE.read().await;
        if let Some(cached) = guard.as_ref() {
            let fresh = SystemTime::now()
                .duration_since(cached.fetched_at)
                .map(|elapsed| elapsed < APPLE_JWKS_TTL)
                .unwrap_or(false);
            if fresh {
                return Ok(cached.keys.clone());
            }
        }
    }

    fetch_apple_jwks_bypass_cache().await
}

async fn fetch_apple_jwks_bypass_cache() -> Result<Vec<AppleJwkKey>, ErrorResponse> {
    let http_client = http_client_for_jwks()?;
    let resp = http_client.get(APPLE_JWKS_URL).send().await.map_err(|e| {
        error!(error = ?e, "Failed to fetch Apple JWKS");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
    })?;

    let status = resp.status();
    if !status.is_success() {
        error!(status = %status, "Apple JWKS endpoint returned non-2xx");
        return Err(
            ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
        );
    }

    if resp
        .content_length()
        .is_some_and(|length| length > APPLE_JWKS_MAX_BYTES as u64)
    {
        error!("Apple JWKS content length exceeded size limit");
        return Err(
            ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
        );
    }

    let mut resp = resp;
    let mut bytes = Vec::with_capacity(4096);
    while let Some(chunk) = resp.chunk().await.map_err(|e| {
        error!(error = ?e, "Failed to read Apple JWKS body");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
    })? {
        if bytes.len().saturating_add(chunk.len()) > APPLE_JWKS_MAX_BYTES {
            error!("Apple JWKS response exceeded size limit");
            return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("JWKS fetch failed"));
        }
        bytes.extend_from_slice(&chunk);
    }

    let parsed: AppleJwkSet = serde_json::from_slice(&bytes).map_err(|e| {
        error!(error = ?e, "Failed to parse Apple JWKS JSON");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("Malformed JWKS")
    })?;

    let keys = parsed.keys;
    let mut guard = APPLE_JWKS_CACHE.write().await;
    *guard = Some(CachedAppleJwks {
        fetched_at: SystemTime::now(),
        keys: keys.clone(),
    });

    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_audiences_include_primary_and_trimmed_unique_extras() {
        let audiences = apple_token_audiences(
            "web.service.id",
            Some(" com.easyquran.ios,com.easyquran.macos,com.easyquran.ios "),
        );

        assert_eq!(
            audiences,
            vec!["web.service.id", "com.easyquran.ios", "com.easyquran.macos"]
        );
    }

    #[test]
    fn id_token_validation_requires_identity_claims() {
        let validation = apple_id_token_validation(&["com.easyquran.ios"]);

        assert!(["exp", "iss", "aud", "sub"]
            .iter()
            .all(|claim| validation.required_spec_claims.contains(*claim)));
    }
}
