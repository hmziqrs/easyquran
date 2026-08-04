use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use oauth2::basic::{
    BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
    BasicTokenType,
};
use oauth2::revocation::StandardRevocableToken;
use oauth2::{
    AuthUrl, Client, ClientId, ClientSecret, RedirectUrl, StandardTokenResponse, TokenUrl,
};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{error, warn};

use crate::error::{ErrorCode, ErrorResponse};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct IdTokenFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}
impl oauth2::ExtraTokenFields for IdTokenFields {}

pub type GoogleClient = Client<
    BasicErrorResponse,
    StandardTokenResponse<IdTokenFields, BasicTokenType>,
    BasicTokenType,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
>;

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleIdTokenClaims {
    pub sub: String,
    pub email: String,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub nonce: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct GoogleJwkKey {
    kid: Option<String>,
    n: String,
    e: String,
}

#[derive(Debug, Deserialize)]
struct GoogleJwkSet {
    keys: Vec<GoogleJwkKey>,
}

struct CachedJwks {
    fetched_at: SystemTime,
    keys: Vec<GoogleJwkKey>,
}

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];
const JWKS_TTL: Duration = Duration::from_secs(3600);

static JWKS_CACHE: LazyLock<RwLock<Option<CachedJwks>>> = LazyLock::new(|| RwLock::new(None));

#[allow(clippy::result_large_err)]
pub fn get_google_oauth_client() -> Result<GoogleClient, ErrorResponse> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GOOGLE_CLIENT_ID not configured")
    })?;

    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GOOGLE_CLIENT_SECRET not configured")
    })?;

    let redirect_url = std::env::var("GOOGLE_REDIRECT_URI").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GOOGLE_REDIRECT_URI not configured")
    })?;

    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid auth URL")
                .with_details(e.to_string())
        })?;

    let token_url =
        TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid token URL")
                .with_details(e.to_string())
        })?;

    let client = GoogleClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Invalid redirect URI")
            .with_details(e.to_string())
    })?);

    Ok(client)
}

pub async fn verify_google_id_token(
    id_token: &str,
    expected_aud: &[&str],
    expected_nonce: Option<&str>,
) -> Result<GoogleIdTokenClaims, ErrorResponse> {
    let keys = fetch_google_jwks().await?;
    match verify_id_token_with_keys_core(
        id_token,
        &keys,
        expected_aud,
        &GOOGLE_ISSUERS,
        expected_nonce,
    ) {
        Ok(claims) => Ok(claims),
        Err(err) => {
            if err.unknown_signer {
                warn!("id_token kid not in cached JWKS; forcing one bypass-cache re-fetch (R-5)");
                let refreshed = fetch_google_jwks_bypass_cache().await?;
                verify_id_token_with_keys_core(
                    id_token,
                    &refreshed,
                    expected_aud,
                    &GOOGLE_ISSUERS,
                    expected_nonce,
                )
                .map_err(|e| e.response)
            } else {
                Err(err.response)
            }
        }
    }
}

#[allow(clippy::result_large_err)]
fn verify_id_token_with_keys_core(
    id_token: &str,
    keys: &[GoogleJwkKey],
    expected_aud: &[&str],
    expected_iss: &[&str],
    expected_nonce: Option<&str>,
) -> Result<GoogleIdTokenClaims, IdTokenError> {
    // Pin RS256: accepting any other alg enables an alg-confusion (signature forgery) attack.
    let header = decode_header(id_token).map_err(|e| {
        warn!(error = ?e, "Google id_token header decode failed");
        IdTokenError::malformed("Malformed id_token header")
    })?;
    if header.alg != Algorithm::RS256 {
        warn!(alg = ?header.alg, "Rejecting id_token with non-RS256 alg");
        return Err(IdTokenError::malformed("Unsupported id_token algorithm"));
    }

    let signing_key = match header
        .kid
        .as_deref()
        .and_then(|kid| keys.iter().find(|k| k.kid.as_deref() == Some(kid)))
    {
        Some(k) => k,
        None => {
            warn!("No matching Google signing key for id_token kid");
            return Err(IdTokenError::unknown_signer());
        }
    };

    let decoding_key =
        DecodingKey::from_rsa_components(&signing_key.n, &signing_key.e).map_err(|e| {
            error!(error = ?e, "Failed to build RSA decoding key from Google JWKS");
            IdTokenError::internal("Invalid signing key")
        })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(expected_aud);
    validation.set_issuer(expected_iss);

    let token_data =
        decode::<GoogleIdTokenClaims>(id_token, &decoding_key, &validation).map_err(|e| {
            warn!(error = ?e, "Google id_token verification failed");
            IdTokenError::invalid("Invalid id_token")
        })?;

    if let Some(expected) = expected_nonce {
        match &token_data.claims.nonce {
            Some(actual) if actual == expected => { /* bound */ }
            other => {
                warn!(
                    ?other,
                    "id_token nonce missing or mismatched — rejecting login"
                );
                return Err(IdTokenError::invalid("Invalid id_token"));
            }
        }
    }

    Ok(token_data.claims)
}

#[derive(Debug)]
struct IdTokenError {
    response: ErrorResponse,
    unknown_signer: bool,
}

impl IdTokenError {
    fn unknown_signer() -> Self {
        Self {
            response: ErrorResponse::new(ErrorCode::InvalidToken)
                .with_message("Untrusted id_token signer"),
            unknown_signer: true,
        }
    }

    fn malformed(msg: &str) -> Self {
        Self {
            response: ErrorResponse::new(ErrorCode::InvalidToken).with_message(msg),
            unknown_signer: false,
        }
    }

    fn invalid(msg: &str) -> Self {
        Self {
            response: ErrorResponse::new(ErrorCode::InvalidToken).with_message(msg),
            unknown_signer: false,
        }
    }

    fn internal(msg: &str) -> Self {
        Self {
            response: ErrorResponse::new(ErrorCode::InternalServerError).with_message(msg),
            unknown_signer: false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
fn verify_id_token_with_keys(
    id_token: &str,
    keys: &[GoogleJwkKey],
    expected_aud: &str,
    expected_iss: &[&str],
    expected_nonce: Option<&str>,
) -> Result<GoogleIdTokenClaims, ErrorResponse> {
    verify_id_token_with_keys_core(id_token, keys, &[expected_aud], expected_iss, expected_nonce)
        .map_err(|e| e.response)
}

async fn fetch_google_jwks() -> Result<Vec<GoogleJwkKey>, ErrorResponse> {

    {
        let guard = JWKS_CACHE.read().await;
        if let Some(cached) = guard.as_ref() {
            let fresh = SystemTime::now()
                .duration_since(cached.fetched_at)
                .map(|elapsed| elapsed < JWKS_TTL)
                .unwrap_or(false);
            if fresh {
                return Ok(cached.keys.clone());
            }
        }
    }

    fetch_google_jwks_bypass_cache().await
}

async fn fetch_google_jwks_bypass_cache() -> Result<Vec<GoogleJwkKey>, ErrorResponse> {
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        // Bound the fetch so a wedged Google endpoint can't pin the login handler (CWE-400).
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| {
            error!(error = ?e, "Failed to build JWKS HTTP client");
            ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
        })?;

    let resp = http_client.get(GOOGLE_JWKS_URL).send().await.map_err(|e| {
        error!(error = ?e, "Failed to fetch Google JWKS");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
    })?;

    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|e| {
        error!(error = ?e, "Failed to read JWKS body");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
    })?;

    if !status.is_success() {
        error!(status = %status, "Google JWKS endpoint returned non-2xx");
        return Err(
            ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("JWKS fetch failed")
        );
    }

    let parsed: GoogleJwkSet = serde_json::from_slice(&bytes).map_err(|e| {
        error!(error = ?e, "Failed to parse Google JWKS JSON");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("Malformed JWKS")
    })?;

    let keys = parsed.keys;
    let mut guard = JWKS_CACHE.write().await;
    *guard = Some(CachedJwks {
        fetched_at: SystemTime::now(),
        keys: keys.clone(),
    });

    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use jsonwebtoken::{encode, EncodingKey, Header};

    const TEST_RSA_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCi4XBzEby8h8is\n\
W9DK/OoeT4YhoVlnnWaJ1PJos5Mq/ZQpNB7GkpQYew5EUc3WThzpeeEYH6kkykva\n\
Pn9DIu9kG4lV2P/Yu8G5o0HvZvSx2OASisV6XLjZUKd0xJmK2Ss9m/lNpU9pqFF9\n\
ze/TOgN43jbbmNYBD+7WqdOurNmeTyTF5rrd7Ww1IxcsRUhfg+jP5ozkx8xFF7cb\n\
X4Ljnu8oYkZS+oi2Qjcp3IHnIXk+O4J/7QecKJDqjIM0Hv50912Bad/UEtVlXYZ7\n\
ntCCgBGoSifyorwPoI6vEDOSABOt81MfLWFi0H2wG0+FLwbb82x/oF9ZO/KbcNVB\n\
0X6yTVIrAgMBAAECggEAfeOaQwW5h0m3WygXx1wVI1o5hHKtpDzujKdOuIfavkaB\n\
phsHklimKAmsLDfBzNpQ1E+EH14RIENOvx7Aw8YTmp8B1Z1DmWL8xxsckglUJMVH\n\
4mzpVrqlkCkbVE/DkKJrHlIYLOAQ8cvLOF3b97kGB/xQEAgfl3CWG8nkt0QXapfr\n\
CrGZeIofTZRhaBUx9/4ttVjjghJtbIdr4JFGBC+3ZMZ5NbuNGDISMQnOCvUBNj1W\n\
PSBS57ToLZkz80x6zQ+vjTPuo/J3yKx/GaX+H7ECTZfM8JjS7vPTyUJvBrmOyhf7\n\
tFv4XWhRR784GcRENH3kFkMtB5B51UWJB4cBTJVWmQKBgQDVzCd6gnPS1UUfXor9\n\
RTKBeRJBC9ZoZUyKPPry4Pjb8sYBmnW4HWRL1MlC0ycuQX7ZFTPdc+mONf6RsLjd\n\
5MHykr1632tio+8IWVwum39TDmq5uQAXeSpaqAMOAkH7QRQNPymK9zEMqFibFPHm\n\
8wXYWnyE3lvWXoeWs896rTLhzwKBgQDDCEyhijYak4kga5TFslTPwxz36S/R0vZc\n\
DtRdM0EdDqJ62RfPpz/spAikiHtuhpT3HVqZZomeQ8M1sbYpaP4EjI7J8fmVBBJ5\n\
y/kT5uA6Jy1I38V8zxKe4jtdMzs1olMJJP+k542tpclOXjbgyBoTgUXmleqzk3bG\n\
YB4Pqpps5QKBgBGMLxVUDbOZQ5IejWPaQRn1WPUzxoZNAio6dRJoOqS62VuaVN0m\n\
tGuw7E/qysV2JLYmklozwFCmx90nVxUHSI/jUV/7ZHH1KJJT20gMBThI76OMtqA2\n\
lq5YKeAFeWro3X901rEMNt9mFdessWoWOj2Wt6+kHH+MxK4u1fGos4trAoGAR9Xn\n\
u9xXf0R2Tp2xh3ve50ObiOi391X37gJ8T/PP+O7qA8uwjIiy7+ufT1MB+7zQY5DJ\n\
TRVKfSPCZCWXzfrhDTXkZhedcTi1wWzSynTQhDrn4B6j9AuldSYo7XQwS9oFMaoS\n\
C2BKe/pDgn0LQ5IQoLyNzZfMgeY/6mN+zxBsns0CgYEAiDb8ssmD+oi7RPFp7Wcz\n\
gyKHV5IglzjfQJ8zl4NkTZrV+ivXkhfVyIFYAT+PEh5ZgevUD01DSJTKvF7UiqaS\n\
Fx8SA361H10YXKqf8gUpNfjl/ixAJzV+ROeAUqCzK+liGuXkFCXbs7Ey0OPQfu5h\n\
JX3Efq+lIpLs6bXvFyKzuRc=\n-----END PRIVATE KEY-----";

    const TEST_N: &str = "ouFwcxG8vIfIrFvQyvzqHk-GIaFZZ51midTyaLOTKv2UKTQexpKUGHsORFHN1k4c6XnhGB-pJMpL2j5_QyLvZBuJVdj_2LvBuaNB72b0sdjgEorFely42VCndMSZitkrPZv5TaVPaahRfc3v0zoDeN4225jWAQ_u1qnTrqzZnk8kxea63e1sNSMXLEVIX4Poz-aM5MfMRRe3G1-C457vKGJGUvqItkI3KdyB5yF5PjuCf-0HnCiQ6oyDNB7-dPddgWnf1BLVZV2Ge57QgoARqEon8qK8D6COrxAzkgATrfNTHy1hYtB9sBtPhS8G2_Nsf6BfWTvym3DVQdF-sk1SKw";
    const TEST_E: &str = "AQAB";
    const TEST_KID: &str = "test-key-1";
    const TEST_AUD: &str = "test-client-id.apps.googleusercontent.com";

    fn test_keys() -> Vec<GoogleJwkKey> {
        vec![GoogleJwkKey {
            kid: Some(TEST_KID.to_string()),
            n: TEST_N.to_string(),
            e: TEST_E.to_string(),
        }]
    }

    fn sign_token(claims: &serde_json::Value, kid: Option<&str>) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = kid.map(|s| s.to_string());
        let key = EncodingKey::from_rsa_pem(TEST_RSA_PEM.as_bytes()).unwrap();
        encode(&header, claims, &key).unwrap()
    }

    fn valid_claims() -> serde_json::Value {
        let exp = (Utc::now() + chrono::Duration::hours(1)).timestamp();
        serde_json::json!({
            "sub": "google-sub-123",
            "email": "user@example.com",
            "email_verified": true,
            "aud": TEST_AUD,
            "iss": "https://accounts.google.com",
            "exp": exp,
            "iat": Utc::now().timestamp(),
        })
    }

    #[test]
    fn id_token_verifies_with_matching_signature_and_claims() {
        let token = sign_token(&valid_claims(), Some(TEST_KID));
        let claims =
            verify_id_token_with_keys(&token, &test_keys(), TEST_AUD, &GOOGLE_ISSUERS, None)
                .unwrap();
        assert_eq!(claims.sub, "google-sub-123");
        assert_eq!(claims.email, "user@example.com");
        assert_eq!(claims.email_verified, Some(true));
    }

    #[test]
    fn id_token_rejected_when_tampered() {
        let token = sign_token(&valid_claims(), Some(TEST_KID));
        let parts: Vec<&str> = token.rsplitn(2, '.').collect();
        let mut sig_bytes = parts[0].as_bytes().to_vec();
        if let Some(b) = sig_bytes.last_mut() {
            *b = if *b == b'A' { b'B' } else { b'A' };
        }
        let sig = String::from_utf8(sig_bytes).unwrap();
        let tampered = format!("{}.{}", parts[1], sig);
        let result =
            verify_id_token_with_keys(&tampered, &test_keys(), TEST_AUD, &GOOGLE_ISSUERS, None);
        assert!(result.is_err(), "tampered token must be rejected");
    }

    #[test]
    fn id_token_rejected_for_wrong_audience() {
        let token = sign_token(&valid_claims(), Some(TEST_KID));
        let result = verify_id_token_with_keys(
            &token,
            &test_keys(),
            "other-client-id",
            &GOOGLE_ISSUERS,
            None,
        );
        assert!(result.is_err(), "wrong-audience token must be rejected");
    }

    #[test]
    fn id_token_rejected_for_unknown_kid() {
        let token = sign_token(&valid_claims(), Some("not-a-known-kid"));
        let result =
            verify_id_token_with_keys(&token, &test_keys(), TEST_AUD, &GOOGLE_ISSUERS, None);
        assert!(
            result.is_err(),
            "token signed by unknown kid must be rejected"
        );
    }

    #[test]
    fn id_token_rejected_when_expired() {
        let mut claims = valid_claims();
        let past = (Utc::now() - chrono::Duration::hours(2)).timestamp();
        claims["exp"] = serde_json::json!(past);
        let token = sign_token(&claims, Some(TEST_KID));
        let result =
            verify_id_token_with_keys(&token, &test_keys(), TEST_AUD, &GOOGLE_ISSUERS, None);
        assert!(result.is_err(), "expired token must be rejected");
    }

    #[test]
    fn unknown_kid_is_the_only_retry_worthy_failure() {
        let token = sign_token(&valid_claims(), Some("not-a-known-kid"));
        let err =
            verify_id_token_with_keys_core(&token, &test_keys(), &[TEST_AUD], &GOOGLE_ISSUERS, None)
                .expect_err("unknown kid must error");
        assert!(
            err.unknown_signer,
            "unknown kid must be flagged retry-worthy"
        );

        let token = sign_token(&valid_claims(), Some(TEST_KID));
        let parts: Vec<&str> = token.rsplitn(2, '.').collect();
        let mut sig_bytes = parts[0].as_bytes().to_vec();
        if let Some(b) = sig_bytes.last_mut() {
            *b = if *b == b'A' { b'B' } else { b'A' };
        }
        let tampered = format!("{}.{}", parts[1], String::from_utf8(sig_bytes).unwrap());
        let err = verify_id_token_with_keys_core(
            &tampered,
            &test_keys(),
            &[TEST_AUD],
            &GOOGLE_ISSUERS,
            None,
        )
        .expect_err("tampered token must error");
        assert!(
            !err.unknown_signer,
            "tampered token must NOT trigger a re-fetch"
        );

        let token = sign_token(&valid_claims(), Some(TEST_KID));
        let err = verify_id_token_with_keys_core(
            &token,
            &test_keys(),
            &["other-client-id"],
            &GOOGLE_ISSUERS,
            None,
        )
        .expect_err("wrong-audience token must error");
        assert!(
            !err.unknown_signer,
            "wrong-audience token must NOT trigger a re-fetch"
        );
    }

    #[test]
    fn stale_jwks_then_refresh_recovers_rotated_kid() {
        let token = sign_token(&valid_claims(), Some(TEST_KID));

        let stale_keys: Vec<GoogleJwkKey> = vec![];
        let first =
            verify_id_token_with_keys_core(&token, &stale_keys, &[TEST_AUD], &GOOGLE_ISSUERS, None);
        assert!(
            first
                .as_ref()
                .err()
                .map(|e| e.unknown_signer)
                .unwrap_or(false),
            "stale JWKS must fail with the retry-worthy unknown_signer flag"
        );

        let refreshed_keys = test_keys();
        let second = verify_id_token_with_keys_core(
            &token,
            &refreshed_keys,
            &[TEST_AUD],
            &GOOGLE_ISSUERS,
            None,
        )
        .expect("refreshed JWKS must verify the rotated-kid token");
        assert_eq!(second.sub, "google-sub-123");
    }

    #[test]
    fn id_token_nonce_must_match_when_expected() {
        let mut claims = valid_claims();
        claims["nonce"] = serde_json::json!("the-nonce-we-sent");
        let token = sign_token(&claims, Some(TEST_KID));

        let ok = verify_id_token_with_keys(
            &token,
            &test_keys(),
            TEST_AUD,
            &GOOGLE_ISSUERS,
            Some("the-nonce-we-sent"),
        )
        .expect("token echoing the expected nonce must verify");
        assert_eq!(ok.nonce.as_deref(), Some("the-nonce-we-sent"));

        let err = verify_id_token_with_keys(
            &token,
            &test_keys(),
            TEST_AUD,
            &GOOGLE_ISSUERS,
            Some("a-different-nonce"),
        );
        assert!(err.is_err(), "mismatched nonce must be rejected");

        let ok_none =
            verify_id_token_with_keys(&token, &test_keys(), TEST_AUD, &GOOGLE_ISSUERS, None)
                .expect("no-nonce-expected flow must still verify");
        assert_eq!(ok_none.sub, "google-sub-123");

        let token_without_nonce = sign_token(&valid_claims(), Some(TEST_KID));
        let err = verify_id_token_with_keys(
            &token_without_nonce,
            &test_keys(),
            TEST_AUD,
            &GOOGLE_ISSUERS,
            Some("the-nonce-we-sent"),
        );
        assert!(
            err.is_err(),
            "token missing the nonce claim must be rejected when a nonce is expected"
        );
    }
}
