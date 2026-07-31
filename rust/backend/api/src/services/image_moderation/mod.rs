use std::collections::HashMap;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationVerdict {
    pub safe: bool,
    #[serde(default)]
    pub scores: HashMap<String, f32>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModerationError {
    #[error("moderation HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("moderation provider returned status {0}")]
    Status(reqwest::StatusCode),
    #[error("moderation provider returned malformed response: {0}")]
    Parse(String),
}

#[async_trait]
pub trait ImageModerator: Send + Sync {
    async fn classify(
        &self,
        bytes: &[u8],
        ctype: &str,
    ) -> Result<ModerationVerdict, ModerationError>;
}

#[derive(Debug, Clone, Default)]
pub struct NoOpModerator;

#[async_trait]
impl ImageModerator for NoOpModerator {
    async fn classify(
        &self,
        _bytes: &[u8],
        _ctype: &str,
    ) -> Result<ModerationVerdict, ModerationError> {
        Ok(ModerationVerdict {
            safe: true,
            scores: HashMap::new(),
        })
    }
}

#[derive(Clone)]
pub struct HttpModerator {
    http: reqwest::Client,
    url: String,
    api_key: SecretString,
}

impl std::fmt::Debug for HttpModerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpModerator")
            .field("url", &self.url)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl HttpModerator {
    pub fn new(http: reqwest::Client, url: String, api_key: String) -> Self {
        Self {
            http,
            url,
            api_key: SecretString::new(api_key.into()),
        }
    }
}

#[async_trait]
impl ImageModerator for HttpModerator {
    async fn classify(
        &self,
        bytes: &[u8],
        ctype: &str,
    ) -> Result<ModerationVerdict, ModerationError> {
        let resp = self
            .http
            .post(&self.url)
            .bearer_auth(self.api_key.expose_secret())
            .header(reqwest::header::CONTENT_TYPE, ctype)
            .body(bytes.to_vec())
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            warn!(
                status = %status,
                body = %body,
                "Image moderation provider returned a non-success status"
            );
            return Err(ModerationError::Status(status));
        }

        let verdict: ModerationVerdict = resp
            .json()
            .await
            .map_err(|e| ModerationError::Parse(e.to_string()))?;

        Ok(verdict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_moderator_always_safe() {
        let noop = NoOpModerator;
        let bytes = b"not really an image";
        let verdict = noop.classify(bytes, "image/png").await.unwrap();
        assert!(verdict.safe);
        assert!(verdict.scores.is_empty());
    }

    #[test]
    fn moderation_verdict_deserializes_provider_shape() {
        let raw = r#"{"safe": false, "scores": {"nsfw": 0.98, "safe": 0.02}}"#;
        let verdict: ModerationVerdict = serde_json::from_str(raw).unwrap();
        assert!(!verdict.safe);
        assert_eq!(verdict.scores.get("nsfw").copied(), Some(0.98));
    }

    #[test]
    fn moderation_verdict_defaults_missing_scores() {
        let raw = r#"{"safe": true}"#;
        let verdict: ModerationVerdict = serde_json::from_str(raw).unwrap();
        assert!(verdict.safe);
        assert!(verdict.scores.is_empty());
    }

    #[test]
    fn http_moderator_debug_redacts_api_key() {
        let moderator = HttpModerator::new(
            reqwest::Client::new(),
            "https://moderator.example/classify".to_string(),
            "super-secret-bearer-token".to_string(),
        );
        let rendered = format!("{:?}", moderator);
        assert!(
            !rendered.contains("super-secret-bearer-token"),
            "api_key leaked into Debug output: {}",
            rendered
        );
        assert!(
            rendered.contains("<redacted>"),
            "redaction marker missing: {}",
            rendered
        );
        assert!(
            rendered.contains("moderator.example"),
            "url missing from Debug output: {}",
            rendered
        );
    }
}
