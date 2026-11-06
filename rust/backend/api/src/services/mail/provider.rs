use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OutboundEmail {
    pub to: String,
    pub subject: String,
    pub html: Option<String>,
    pub text: Option<String>,
    pub template: Option<&'static str>,
}

pub const TEMPLATE_VERIFICATION: &str = "verification";
pub const TEMPLATE_PASSWORD_RESET: &str = "password_reset";
pub const TEMPLATE_NEWSLETTER: &str = "newsletter";
/// Account-enforcement notice (A21). Enforcement-scoped: it is sent even to an
/// address with a complaint suppression, and it is dropped (with a `suppressed`
/// result) for bounce or manual suppression.
pub const TEMPLATE_ENFORCEMENT_NOTICE: &str = "enforcement_notice";

pub fn is_transactional(template: Option<&str>) -> bool {
    matches!(
        template,
        Some(TEMPLATE_VERIFICATION)
            | Some(TEMPLATE_PASSWORD_RESET)
            | Some(TEMPLATE_ENFORCEMENT_NOTICE)
    )
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendReceipt {
    #[serde(default)]
    pub delivered: u32,
    #[serde(default)]
    pub queued: u32,
    #[serde(default)]
    pub permanent_bounces: Vec<String>,
    /// The router dropped the message because the recipient is suppressed. Nothing
    /// reached a provider. Transactional callers that must not reveal suppression
    /// (verification, reset) ignore it; enforcement records it as `suppressed`.
    #[serde(default)]
    pub suppressed: bool,
}

impl SendReceipt {
    /// True only when a provider accepted the message (delivered or queued).
    pub fn accepted_by_provider(&self) -> bool {
        !self.suppressed && self.delivered + self.queued > 0
    }
}

pub use rux_provider_core::WebhookEvent;

pub mod canonical {
    pub const BOUNCED: &str = "email.bounced";
    pub const COMPLAINED: &str = "email.complained";
    pub const DELIVERED: &str = "email.delivered";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMailEvent {
    pub event_type: String,
    pub recipient: String,
    pub message_id: Option<String>,
    pub diagnostic: Option<String>,
    pub permanent: bool,
    pub ts: Option<i64>,
    pub data: serde_json::Value,
}

#[async_trait]
pub trait MailProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;

    async fn send(&self, msg: OutboundEmail) -> Result<SendReceipt, MailError>;

    async fn verify_webhook(&self, event: WebhookEvent) -> Result<ParsedMailEvent, MailError> {
        let _ = event;
        Err(MailError::WebhookVerification(format!(
            "provider '{}' has no inbound webhook",
            self.provider_name()
        )))
    }
}

impl rux_provider_core::Provider for dyn MailProvider {}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("mail configuration error: {0}")]
    Config(String),

    #[error("mail provider API error: {0}")]
    ProviderApi(String),

    #[error("mail HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("rate limited; retry in {retry_after_secs}s")]
    Throttled { retry_after_secs: u64 },

    #[error("mail rate limiter unavailable")]
    LimiterUnavailable,

    #[error("recipient suppressed")]
    Suppressed,

    #[error("invalid recipient: {0}")]
    InvalidRecipient(String),

    #[error("webhook verification failed: {0}")]
    WebhookVerification(String),

    #[error("{0}")]
    Other(String),
}

impl From<rux_provider_core::FrameworkError> for MailError {
    fn from(err: rux_provider_core::FrameworkError) -> Self {
        match err {
            rux_provider_core::FrameworkError::ProviderNotRegistered(name) => {
                MailError::Config(format!("mail provider '{name}' not initialized"))
            }
            rux_provider_core::FrameworkError::Config(msg) => MailError::Config(msg),
            rux_provider_core::FrameworkError::ProviderApi(msg) => MailError::ProviderApi(msg),
            rux_provider_core::FrameworkError::Other(msg) => MailError::Other(msg),
        }
    }
}
