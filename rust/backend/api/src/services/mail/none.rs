//! No-op mail provider (`MAIL_PROVIDER=none`).
//!
//! For deployments that don't send mail yet — e.g. a content-only Quran reader
//! before accounts/email are wired. `send` succeeds without delivering anything,
//! so the always-on [`MailRouter`](super::router::MailRouter) boots with ZERO
//! mail credentials (no SMTP host/user/pass, no Cloudflare keys).
//!
//! Swap to `MAIL_PROVIDER=cloudflare` (or `smtp`) when mail is actually needed.

use async_trait::async_trait;

use super::provider::{MailError, MailProvider, OutboundEmail, SendReceipt};

/// Mail provider that drops every message. Used when no mail backend is
/// configured; the router still runs, so mail-dependent callers don't crash —
/// they just silently no-op (with a warning) until a real provider is set.
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
