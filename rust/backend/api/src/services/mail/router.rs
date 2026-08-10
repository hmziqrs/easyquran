use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rux_provider_core::ProviderRegistry;
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use tracing::{debug, instrument, warn};

use crate::db::sea_models::email_suppression::{Entity as SuppressionEntity, SuppressionUpsert};
use crate::services::abuse_limiter::{self, AbuseLimiterConfig, LimiterDecision};
use crate::utils::telemetry;
use ruxlog_types::enums::SuppressionReason;

use super::provider::{
    is_transactional, MailError, MailProvider, OutboundEmail, ParsedMailEvent, SendReceipt,
    WebhookEvent, TEMPLATE_NEWSLETTER,
};

pub const MAIL_RCPT_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 5,
    temp_block_range: 10 * 60,
    temp_block_duration: 60 * 60,
    block_retry_limit: 30,
    block_range: 24 * 60 * 60,
    block_duration: 24 * 60 * 60,
};

pub const MAIL_PROVIDER_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 50,
    temp_block_range: 60,
    temp_block_duration: 5 * 60,
    block_retry_limit: 500,
    block_range: 60 * 60,
    block_duration: 60 * 60,
};

#[derive(Clone)]
pub struct MailRouterLimits {
    pub rcpt: AbuseLimiterConfig,
    pub provider: AbuseLimiterConfig,
    pub dedup_ttl_secs: usize,
    pub soft_cooldown_secs: i64,
}

impl Default for MailRouterLimits {
    fn default() -> Self {
        Self {
            rcpt: MAIL_RCPT_CFG,
            provider: MAIL_PROVIDER_CFG,
            dedup_ttl_secs: 300,
            soft_cooldown_secs: 24 * 60 * 60,
        }
    }
}

pub struct MailRouter {
    registry: ProviderRegistry<dyn MailProvider>,
    gate_store: Arc<rux_request_gate::InMemoryStore>,
    db: DatabaseConnection,
    limits: MailRouterLimits,
    rate_limit_enabled: bool,
}

impl MailRouter {
    pub fn new(
        providers: std::collections::HashMap<String, Arc<dyn MailProvider>>,
        default_provider: String,
        gate_store: Arc<rux_request_gate::InMemoryStore>,
        db: DatabaseConnection,
        limits: MailRouterLimits,
        rate_limit_enabled: bool,
    ) -> Self {
        Self {
            registry: ProviderRegistry::new(providers, default_provider),
            gate_store,
            db,
            limits,
            rate_limit_enabled,
        }
    }

    pub fn provider_names(&self) -> Vec<&str> {
        self.registry.provider_names()
    }

    fn get_provider(&self, name: &str) -> Result<&Arc<dyn MailProvider>, MailError> {
        self.registry.get(name).map_err(MailError::from)
    }

    /// Best-effort redaction of the rate-limit key for logs. The recipient key
    /// embeds the full mailbox (`mail:send:rcpt:{email}`); collapse it to the
    /// recipient domain. Provider keys (`mail:send:provider:{name}`) carry no PII
    /// and pass through verbatim. Matches the module's domain-only PII policy.
    fn redact_rate_key(key: &str) -> String {
        if let Some(rest) = key.strip_prefix("mail:send:rcpt:") {
            let domain = rest.split('@').nth(1).unwrap_or("unknown");
            format!("mail:send:rcpt:<redacted>@{domain}")
        } else {
            key.to_string()
        }
    }

    async fn lookup_suppressed(&self, recipient: &str) -> Result<bool, ()> {
        let row = match SuppressionEntity::find_by_recipient(&self.db, recipient).await {
            Ok(m) => m,
            Err(e) => {
                // PII: log only the recipient domain, never the full address.
                let domain = recipient.split('@').nth(1).unwrap_or("unknown");
                warn!(error = %e, %domain, "suppression lookup failed (fail-open)");
                return Err(());
            }
        };
        let Some(row) = row else {
            return Ok(false);
        };
        let now = Utc::now().timestamp();
        let within_cooldown = now - row.last_seen.timestamp() <= self.limits.soft_cooldown_secs;
        Ok(row.permanent || (row.reason == SuppressionReason::Bounce && within_cooldown))
    }

    async fn check_rate(&self, recipient: &str) -> Result<(), MailError> {
        self.enforce(&format!("mail:send:rcpt:{recipient}"), self.limits.rcpt)
            .await?;
        self.enforce(
            &format!("mail:send:provider:{}", self.registry.default_provider()),
            self.limits.provider,
        )
        .await?;
        Ok(())
    }

    async fn enforce(&self, key: &str, cfg: AbuseLimiterConfig) -> Result<(), MailError> {
        match rux_request_gate::check(&self.gate_store, &abuse_limiter::TelemetryHooks, key, cfg)
            .await
        {
            Ok(LimiterDecision::Allowed { .. }) => Ok(()),
            Ok(LimiterDecision::Blocked {
                retry_after_secs, ..
            }) => {
                telemetry::mail_router_metrics().throttled.add(1, &[]);
                Err(MailError::Throttled { retry_after_secs })
            }
            Err(e) => {
                // PII: the rcpt key embeds the full recipient address — log only a
                // redacted form (recipient domain) so ops still see which limiter
                // fired without leaking the mailbox.
                warn!(
                    error = %e,
                    rate_key = %Self::redact_rate_key(key),
                    "mail rate limiter Redis error (fail-closed 503)"
                );
                Err(MailError::LimiterUnavailable)
            }
        }
    }

    async fn release_dedup(&self, key: Option<&str>) {
        if let Some(key) = key {
            rux_request_gate::release_dedup(&self.gate_store, key).await;
        }
    }

    fn dedup_key(&self, msg: &OutboundEmail) -> String {
        let mut h = Sha256::new();
        h.update(msg.to.as_bytes());
        h.update(msg.subject.as_bytes());
        if let Some(b) = &msg.html {
            h.update(b.as_bytes());
        }
        if let Some(b) = &msg.text {
            h.update(b.as_bytes());
        }
        let digest = hex::encode(h.finalize());
        format!("mail:dedup:{}:{digest}", msg.template.unwrap_or("default"))
    }

    async fn record_sync_bounces(&self, provider_name: &str, bounces: &[String]) {
        let router_metrics = telemetry::mail_router_metrics();
        for rcpt in bounces {
            let canonical = canonicalize_recipient(rcpt).unwrap_or_else(|_| rcpt.clone());
            let upsert = SuppressionUpsert {
                reason: SuppressionReason::Bounce,
                source: Some(format!("{provider_name}-send-sync")),
                diagnostic: None,
                permanent: true,
            };
            match SuppressionEntity::upsert(&self.db, &canonical, upsert).await {
                Ok(_) => router_metrics.bounced_sync.add(1, &[]),
                Err(e) => warn!(error = %e, "failed to upsert sync-bounce suppression"),
            }
        }
    }
}

#[async_trait]
impl MailProvider for MailRouter {
    fn provider_name(&self) -> &'static str {
        "router"
    }

    #[instrument(skip(self, msg), fields(provider = %self.registry.default_provider(), recipient_domain, result))]
    async fn send(&self, msg: OutboundEmail) -> Result<SendReceipt, MailError> {
        let metrics = telemetry::mail_metrics();
        let router_metrics = telemetry::mail_router_metrics();
        let start = std::time::Instant::now();

        let mut msg = msg;
        msg.to = canonicalize_recipient(&msg.to)?;
        let recipient_domain = msg.to.split('@').nth(1).unwrap_or("unknown");
        tracing::Span::current().record("recipient_domain", recipient_domain);

        let transactional = is_transactional(msg.template);

        let suppressed = match self.lookup_suppressed(&msg.to).await {
            Ok(true) => true,
            Ok(false) => false,
            Err(()) => {
                router_metrics.suppression_check_failed.add(1, &[]);
                false
            }
        };
        if suppressed {
            router_metrics.suppressed.add(1, &[]);
            if transactional {
                // Anti-enumeration: MUST return Ok and look identical to a real send — erroring would leak that the account is suppressed.
                debug!(template = ?msg.template, "suppressed transactional send dropped");
                return Ok(SendReceipt::default());
            }
            return Err(MailError::Suppressed);
        }

        // Dedup must run before rate-limiting, else a duplicate blast burns provider quota and self-DoSes all non-transactional mail.
        let dedup_claim = if msg.template == Some(TEMPLATE_NEWSLETTER) {
            let key = self.dedup_key(&msg);
            if rux_request_gate::dedup_nx(&self.gate_store, &key, self.limits.dedup_ttl_secs).await
            {
                Some(key)
            } else {
                router_metrics.deduped.add(1, &[]);
                debug!("duplicate newsletter send suppressed");
                return Ok(SendReceipt::default());
            }
        } else {
            None
        };

        // Transactional sends are rate-limited at the controller (per user_id); exempt here or they get double-limited under a different key.
        if self.rate_limit_enabled && !transactional {
            if let Err(e) = self.check_rate(&msg.to).await {
                self.release_dedup(dedup_claim.as_deref()).await;
                return Err(e);
            }
        }

        let provider = self.get_provider(self.registry.default_provider())?;
        let provider_name = provider.provider_name();
        let receipt = match provider.send(msg).await {
            Ok(r) => r,
            Err(e) => {
                metrics.emails_failed.add(1, &[]);
                tracing::Span::current().record("result", "failure");
                self.release_dedup(dedup_claim.as_deref()).await;
                return Err(e);
            }
        };

        metrics.emails_sent.add(1, &[]);
        metrics
            .send_duration
            .record(start.elapsed().as_millis() as f64, &[]);
        tracing::Span::current().record("result", "success");

        if !receipt.permanent_bounces.is_empty() {
            self.record_sync_bounces(provider_name, &receipt.permanent_bounces)
                .await;
        }

        Ok(receipt)
    }

    async fn verify_webhook(&self, event: WebhookEvent) -> Result<ParsedMailEvent, MailError> {
        let provider = self
            .registry
            .get_for_webhook(&event)
            .map_err(MailError::from)?;
        provider.verify_webhook(event).await
    }
}

pub fn canonicalize_recipient(input: &str) -> Result<String, MailError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(MailError::InvalidRecipient(
            "recipient is empty".to_string(),
        ));
    }
    if trimmed.contains('\r') || trimmed.contains('\n') {
        return Err(MailError::InvalidRecipient(
            "recipient must not contain line breaks".to_string(),
        ));
    }
    if trimmed.contains(',')
        || trimmed.contains(';')
        || trimmed.contains('<')
        || trimmed.contains('>')
    {
        return Err(MailError::InvalidRecipient(
            "recipient must be a single mailbox without display name".to_string(),
        ));
    }
    let lower = trimmed.to_lowercase();
    match lower.split_once('@') {
        Some((local, domain))
            if !local.is_empty() && !domain.is_empty() && !domain.contains('@') =>
        {
            Ok(lower)
        }
        _ => Err(MailError::InvalidRecipient(
            "recipient is not a valid email address".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_trims_and_lowercases() {
        assert_eq!(
            canonicalize_recipient("  Foo@Example.COM ").unwrap(),
            "foo@example.com"
        );
    }

    #[test]
    fn canonicalize_rejects_header_injection() {
        assert!(canonicalize_recipient("a@b.com\r\nBcc: x@y.com").is_err());
        assert!(canonicalize_recipient("a\n@b.com").is_err());
    }

    #[test]
    fn canonicalize_rejects_multiple_or_displayname() {
        assert!(canonicalize_recipient("a@b.com,c@d.com").is_err());
        assert!(canonicalize_recipient("Name <a@b.com>").is_err());
        assert!(canonicalize_recipient("a@b.com;d@c.com").is_err());
    }

    #[test]
    fn canonicalize_rejects_malformed() {
        assert!(canonicalize_recipient("noatsign").is_err());
        assert!(canonicalize_recipient("@b.com").is_err());
        assert!(canonicalize_recipient("a@").is_err());
        assert!(canonicalize_recipient("a@@b.com").is_err());
        assert!(canonicalize_recipient("   ").is_err());
    }
}
