use async_trait::async_trait;

use super::provider::{MailError, MailProvider, OutboundEmail, SendReceipt};

#[derive(Default)]
pub struct NoOpMailProvider;

#[async_trait]
impl MailProvider for NoOpMailProvider {
    fn provider_name(&self) -> &'static str {
        "none"
    }

    async fn send(&self, msg: OutboundEmail) -> Result<SendReceipt, MailError> {
        tracing::warn!(
            recipient = %msg.to,
            subject = %msg.subject,
            "mail dropped: MAIL_PROVIDER=none (no mail backend configured)"
        );
        Ok(SendReceipt::default())
    }
}
