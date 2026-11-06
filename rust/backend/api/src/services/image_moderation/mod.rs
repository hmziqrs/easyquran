//! Upload screening (A37). Two checks run on every media upload, before dedup:
//!
//! 1. [`ImageModerator`]: classifier verdict from the configured provider (HTTP).
//! 2. [`KnownHashMatcher`]: hash-match against known abusive material. Trait only; no
//!    vendor is bound in this build. Nothing is wired in production until the owner
//!    registers a provider (O29).
//!
//! Both fail closed: a provider error rejects the upload. A rejected image never
//! reaches dedup or storage.

use std::collections::HashMap;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::config::settings::EnvClass;

/// `media_v1` is nested unconditionally in `router.rs`. If that changes, pass the new
/// truth to [`moderation_boot_gate`].
pub const MEDIA_UPLOAD_MOUNTED: bool = true;

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

/// Hash-match against known abusive material. `Ok(true)` means a known match.
/// Implementations live outside this build until a provider is chosen (O29).
#[async_trait]
pub trait KnownHashMatcher: Send + Sync {
    async fn matches(&self, bytes: &[u8]) -> Result<bool, ModerationError>;
}

/// Why an upload was refused before dedup. The client sees one generic message for
/// `Unsafe` and `KnownHash` so a probe cannot tell which check fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRejection {
    Unsafe,
    KnownHash,
    Unavailable,
}

/// Runs moderation, then hash-match. Both are optional; an absent check passes.
/// Moderation runs first so a flagged image never reaches the hash-match provider.
pub async fn screen_upload<M, H>(
    moderator: Option<&M>,
    hashes: Option<&H>,
    bytes: &[u8],
    ctype: &str,
) -> Result<(), ScreenRejection>
where
    M: ImageModerator + ?Sized,
    H: KnownHashMatcher + ?Sized,
{
    if let Some(moderator) = moderator {
        match moderator.classify(bytes, ctype).await {
            Ok(verdict) if verdict.safe => {}
            Ok(verdict) => {
                warn!(scores = ?verdict.scores, "Upload flagged by image moderation");
                return Err(ScreenRejection::Unsafe);
            }
            Err(err) => {
                warn!(error = %err, "Image moderation provider error; failing closed");
                return Err(ScreenRejection::Unavailable);
            }
        }
    }
    if let Some(hashes) = hashes {
        match hashes.matches(bytes).await {
            Ok(false) => {}
            Ok(true) => return Err(ScreenRejection::KnownHash),
            Err(err) => {
                warn!(error = %err, "Known-hash provider error; failing closed");
                return Err(ScreenRejection::Unavailable);
            }
        }
    }
    Ok(())
}

/// Production must not boot with media upload mounted and no moderator. Other
/// environments may boot without one.
pub fn moderation_boot_gate(
    env: EnvClass,
    media_upload_mounted: bool,
    moderator_configured: bool,
) -> Result<(), &'static str> {
    if !media_upload_mounted || moderator_configured {
        return Ok(());
    }
    if matches!(env, EnvClass::Production) {
        return Err(
            "APP_ENV=production with media upload enabled requires IMAGE_MODERATION_ENABLED=true \
             and IMAGE_MODERATION_URL set",
        );
    }
    Ok(())
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[derive(Clone, Copy)]
    enum FakeVerdict {
        Safe,
        Unsafe,
        Down,
    }

    struct FakeModerator(FakeVerdict);

    #[async_trait]
    impl ImageModerator for FakeModerator {
        async fn classify(
            &self,
            _bytes: &[u8],
            _ctype: &str,
        ) -> Result<ModerationVerdict, ModerationError> {
            match self.0 {
                FakeVerdict::Safe => Ok(ModerationVerdict {
                    safe: true,
                    scores: HashMap::new(),
                }),
                FakeVerdict::Unsafe => Ok(ModerationVerdict {
                    safe: false,
                    scores: HashMap::new(),
                }),
                FakeVerdict::Down => Err(ModerationError::Parse("down".to_string())),
            }
        }
    }

    struct FakeHashes {
        hit: bool,
        down: bool,
        calls: AtomicUsize,
    }

    impl FakeHashes {
        fn new(hit: bool, down: bool) -> Self {
            Self {
                hit,
                down,
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl KnownHashMatcher for FakeHashes {
        async fn matches(&self, _bytes: &[u8]) -> Result<bool, ModerationError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.down {
                return Err(ModerationError::Parse("down".to_string()));
            }
            Ok(self.hit)
        }
    }

    const BYTES: &[u8] = b"not really an image";

    #[tokio::test]
    async fn no_checks_configured_passes() {
        let r = screen_upload::<FakeModerator, FakeHashes>(None, None, BYTES, "image/png").await;
        assert_eq!(r, Ok(()));
    }

    #[tokio::test]
    async fn safe_verdict_and_no_hash_hit_passes() {
        let m = FakeModerator(FakeVerdict::Safe);
        let h = FakeHashes::new(false, false);
        let r = screen_upload(Some(&m), Some(&h), BYTES, "image/png").await;
        assert_eq!(r, Ok(()));
        assert_eq!(h.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unsafe_verdict_rejects_before_hash_match() {
        let m = FakeModerator(FakeVerdict::Unsafe);
        let h = FakeHashes::new(false, false);
        let r = screen_upload(Some(&m), Some(&h), BYTES, "image/png").await;
        assert_eq!(r, Err(ScreenRejection::Unsafe));
        assert_eq!(
            h.calls.load(Ordering::SeqCst),
            0,
            "hash provider must not see flagged bytes"
        );
    }

    #[tokio::test]
    async fn known_hash_hit_rejects() {
        let h = FakeHashes::new(true, false);
        let r = screen_upload::<FakeModerator, _>(None, Some(&h), BYTES, "image/png").await;
        assert_eq!(r, Err(ScreenRejection::KnownHash));
    }

    #[tokio::test]
    async fn moderator_error_fails_closed() {
        let m = FakeModerator(FakeVerdict::Down);
        let r = screen_upload::<_, FakeHashes>(Some(&m), None, BYTES, "image/png").await;
        assert_eq!(r, Err(ScreenRejection::Unavailable));
    }

    #[tokio::test]
    async fn hash_provider_error_fails_closed() {
        let h = FakeHashes::new(false, true);
        let r = screen_upload::<FakeModerator, _>(None, Some(&h), BYTES, "image/png").await;
        assert_eq!(r, Err(ScreenRejection::Unavailable));
    }

    #[test]
    fn production_without_moderator_refuses_boot() {
        assert!(moderation_boot_gate(EnvClass::Production, true, false).is_err());
    }

    #[test]
    fn production_with_moderator_boots() {
        assert!(moderation_boot_gate(EnvClass::Production, true, true).is_ok());
    }

    #[test]
    fn non_production_without_moderator_boots() {
        assert!(moderation_boot_gate(EnvClass::NonProduction, true, false).is_ok());
        assert!(moderation_boot_gate(EnvClass::Unset, true, false).is_ok());
    }

    #[test]
    fn production_without_media_mount_boots() {
        assert!(moderation_boot_gate(EnvClass::Production, false, false).is_ok());
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
