use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::FcmError;

const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const TOKEN_TTL_SECS: i64 = 3600;
const REFRESH_LEAD_SECS: i64 = 300;

#[derive(Debug, Deserialize)]
struct RawServiceAccount {
    private_key: String,
    private_key_id: String,
    client_email: String,
    project_id: String,
}

pub struct ServiceAccount {
    pub client_email: String,
    pub project_id: String,
    // Long-lived signing secret — wrapped in SecretString; the struct deliberately
    // omits derive(Debug) so an accidental {:?} can't leak it.
    private_key: SecretString,
    private_key_id: String,
    cache: RwLock<CachedToken>,
}

#[derive(Default)]
struct CachedToken {
    token: Option<String>,
    expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct AccessToken {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    iat: i64,
    exp: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default = "default_expires_in")]
    expires_in: i64,
}

fn default_expires_in() -> i64 {
    TOKEN_TTL_SECS
}

impl ServiceAccount {
    pub fn from_path(path: &str) -> Result<Self, FcmError> {
        let bytes = std::fs::read(path)
            .map_err(|e| FcmError::MissingConfig(format!("read service account {}: {e}", path)))?;
        let json = String::from_utf8(bytes)
            .map_err(|e| FcmError::MissingConfig(format!("service account not utf-8: {e}")))?;
        Self::from_json_str(&json)
    }

    pub fn from_json_str(json: &str) -> Result<Self, FcmError> {
        let raw: RawServiceAccount = serde_json::from_str(json)
            .map_err(|e| FcmError::Auth(format!("invalid service account JSON: {e}")))?;
        Ok(Self::from_raw(raw))
    }

    fn from_raw(raw: RawServiceAccount) -> Self {
        Self {
            client_email: raw.client_email,
            project_id: raw.project_id,
            private_key: SecretString::new(raw.private_key.into()),
            private_key_id: raw.private_key_id,
            cache: RwLock::new(CachedToken::default()),
        }
    }

    pub async fn mint_access_token(&self, http: &reqwest::Client) -> Result<AccessToken, FcmError> {
        let now = Utc::now().timestamp();

        {
            let guard = self.cache.read().await;
            if let Some(token) = guard.token.as_ref() {
                if guard.expires_at - now > REFRESH_LEAD_SECS {
                    return Ok(AccessToken {
                        token: token.clone(),
                        expires_at: guard.expires_at,
                    });
                }
            }
        }

        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.private_key_id.clone());

        let claims = Claims {
            iss: self.client_email.clone(),
            scope: FCM_SCOPE.to_string(),
            aud: TOKEN_URL.to_string(),
            iat: now,
            exp: now + TOKEN_TTL_SECS,
        };

        let key = EncodingKey::from_rsa_pem(self.private_key.expose_secret().as_bytes())
            .map_err(|e| FcmError::Auth(format!("invalid private key: {e}")))?;
        let assertion = encode(&header, &claims, &key)
            .map_err(|e| FcmError::Auth(format!("jwt encode: {e}")))?;

        let form = format!(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}"
        );

        let resp = http
            .post(TOKEN_URL)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(FcmError::Auth(format!(
                "token endpoint returned {}: {}",
                status.as_u16(),
                body
            )));
        }

        let parsed: TokenResponse = serde_json::from_str(&body)
            .map_err(|e| FcmError::Auth(format!("token response parse: {e}")))?;

        let expires_at = now + parsed.expires_in;
        let token = AccessToken {
            token: parsed.access_token.clone(),
            expires_at,
        };

        {
            let mut guard = self.cache.write().await;
            *guard = CachedToken {
                token: Some(parsed.access_token),
                expires_at,
            };
        }

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_json_str_rejects_invalid_json() {
        let err = ServiceAccount::from_json_str("not json").expect_err("must reject");
        assert!(matches!(err, FcmError::Auth(_)));
    }

    #[test]
    fn from_json_str_parses_required_fields() {
        let json = serde_json::json!({
            "private_key": "-----BEGIN PRIVATE KEY-----\nxx\n-----END PRIVATE KEY-----\n",
            "private_key_id": "kid-123",
            "client_email": "fcm@test.iam.gserviceaccount.com",
            "project_id": "test-project",
        })
        .to_string();
        let sa = ServiceAccount::from_json_str(&json).expect("must parse");
        assert_eq!(sa.client_email, "fcm@test.iam.gserviceaccount.com");
        assert_eq!(sa.project_id, "test-project");
        assert_eq!(sa.private_key_id, "kid-123");
        let dbg = format!("{:?}", &sa.private_key);
        assert!(
            !dbg.contains("BEGIN PRIVATE KEY"),
            "private key leaked into Debug: {}",
            dbg
        );
    }
}
