use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use rux_provider_core::ProviderRegistry;
use rux_request_gate::{BlockScope, RateLimitStore};
use sea_orm::DatabaseConnection;
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, instrument, warn};

use crate::db::sea_models::email_suppression::{
    Entity as SuppressionEntity, Model as SuppressionRow, SuppressionUpsert,
};
use crate::services::abuse_limiter::{self, AbuseLimiterConfig, LimiterDecision};
use crate::services::audit::{self, AuditAction, AuditEvent};
use crate::utils::telemetry;
use ruxlog_types::enums::SuppressionReason;

use super::provider::{
    is_transactional, MailError, MailProvider, OutboundEmail, ParsedMailEvent, SendReceipt,
    WebhookEvent, TEMPLATE_ENFORCEMENT_NOTICE, TEMPLATE_NEWSLETTER, TEMPLATE_PASSWORD_RESET,
    TEMPLATE_VERIFICATION,
};

pub const MAIL_RCPT_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 5,
    temp_block_range: 10 * 60,
    temp_block_duration: 60 * 60,
    block_retry_limit: 30,
    block_range: 24 * 60 * 60,
    block_duration: 24 * 60 * 60,
};

/// Transactional mail (password reset / verification) is exempt from the tight
/// marketing cap but not uncapped: 30/hour per recipient stops a reset-mail
/// flood (~1440/day) against a victim's mailbox while leaving legitimate
/// account-recovery flows untouched.
pub const MAIL_RCPT_TRANSACTIONAL_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 30,
    temp_block_range: 60 * 60,
    temp_block_duration: 60 * 60,
    block_retry_limit: 90,
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

/// Hourly cap that admits exactly `allowed` sends per hour. The limiter blocks the
/// attempt that reaches `temp_block_attempts`, so the threshold is `allowed + 1`.
/// A tripped cap blocks for the rest of the hour. The long tier never fires.
const fn hourly_cap(allowed: usize) -> AbuseLimiterConfig {
    AbuseLimiterConfig {
        temp_block_attempts: allowed + 1,
        temp_block_range: 60 * 60,
        temp_block_duration: 60 * 60,
        block_retry_limit: usize::MAX,
        block_range: 60 * 60,
        block_duration: 60 * 60,
    }
}

/// Global cap on password-reset mail, separate from verification so one cannot
/// starve the other (A29).
pub const GLOBAL_RESET_CAP: usize = 500;
/// Global cap on verification mail (A29).
pub const GLOBAL_VERIFY_CAP: usize = 500;
/// Per recipient domain, every template class (A29).
pub const DOMAIN_CAP: usize = 100;
/// Per recipient and template: a repeat inside this many seconds is throttled (A29).
pub const SEND_COOLDOWN_SECS: usize = 60;

/// Bounce guard (A29): pause all sends when more than `GUARD_MAX_BOUNCE_PCT` percent
/// of the hour's sends bounced. Needs `GUARD_MIN_SAMPLE` sends first, so one early
/// bounce cannot pause the site.
pub const GUARD_MIN_SAMPLE: u64 = 20;
pub const GUARD_MAX_BOUNCE_PCT: u64 = 5;
pub const GUARD_PAUSE_SECS: usize = 15 * 60;
/// Store key for the pause. Read with `ban_status`, set with `set_block`.
pub const GUARD_PAUSE_KEY: &str = "mail:guard:paused";

/// Recipient suppression applies to bounces for this long after the last bounce.
const BOUNCE_SUPPRESSION_COOLDOWN_SECS: i64 = 24 * 60 * 60;

#[derive(Clone)]
pub struct MailRouterLimits {
    pub rcpt: AbuseLimiterConfig,
    pub rcpt_transactional: AbuseLimiterConfig,
    pub provider: AbuseLimiterConfig,
    pub global_reset: AbuseLimiterConfig,
    pub global_verify: AbuseLimiterConfig,
    pub domain: AbuseLimiterConfig,
    pub cooldown_secs: usize,
    pub dedup_ttl_secs: usize,
    pub soft_cooldown_secs: i64,
}

impl Default for MailRouterLimits {
    fn default() -> Self {
        Self {
            rcpt: MAIL_RCPT_CFG,
            rcpt_transactional: MAIL_RCPT_TRANSACTIONAL_CFG,
            provider: MAIL_PROVIDER_CFG,
            global_reset: hourly_cap(GLOBAL_RESET_CAP),
            global_verify: hourly_cap(GLOBAL_VERIFY_CAP),
            domain: hourly_cap(DOMAIN_CAP),
            cooldown_secs: SEND_COOLDOWN_SECS,
            dedup_ttl_secs: 300,
            soft_cooldown_secs: BOUNCE_SUPPRESSION_COOLDOWN_SECS,
        }
    }
}

/// Sends and bounces seen in the current UTC hour, for the bounce guard.
#[derive(Default)]
struct GuardWindow {
    hour: i64,
    sent: u64,
    bounced: u64,
}

impl GuardWindow {
    fn roll(&mut self, hour: i64) {
        if self.hour != hour {
            *self = Self {
                hour,
                ..Self::default()
            };
        }
    }
}

fn current_hour() -> i64 {
    Utc::now().timestamp() / 3600
}

pub struct MailRouter {
    registry: ProviderRegistry<dyn MailProvider>,
    gate_store: Arc<rux_request_gate::InMemoryStore>,
    db: DatabaseConnection,
    limits: MailRouterLimits,
    rate_limit_enabled: bool,
    guard: Mutex<GuardWindow>,
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
            guard: Mutex::new(GuardWindow::default()),
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

    /// The suppression row for a recipient, if any. A lookup error is `Err(())`.
    async fn lookup_suppression(&self, recipient: &str) -> Result<Option<SuppressionRow>, ()> {
        match SuppressionEntity::find_by_recipient(&self.db, recipient).await {
            Ok(row) => Ok(row),
            Err(e) => {
                // PII: log only the recipient domain, never the full address.
                let domain = recipient.split('@').nth(1).unwrap_or("unknown");
                warn!(error = %e, %domain, "suppression lookup failed");
                Err(())
            }
        }
    }

    /// Whether a row blocks this template. Complaint rows never stop enforcement
    /// notices (A21). Rows with `blocks_transactional = false` (webhook source, not
    /// yet reviewed) never stop transactional mail (A29). Marketing mail stops on
    /// any active row.
    fn suppression_blocks(&self, row: &SuppressionRow, template: Option<&str>, now: i64) -> bool {
        let within_cooldown = now - row.last_seen.timestamp() <= self.limits.soft_cooldown_secs;
        let active = row.permanent || (row.reason == SuppressionReason::Bounce && within_cooldown);
        if !active {
            return false;
        }
        if template == Some(TEMPLATE_ENFORCEMENT_NOTICE)
            && row.reason == SuppressionReason::Complaint
        {
            return false;
        }
        if is_transactional(template) && !row.blocks_transactional {
            return false;
        }
        true
    }

    fn global_cap(&self, template: Option<&str>) -> Option<(&'static str, AbuseLimiterConfig)> {
        match template {
            Some(TEMPLATE_PASSWORD_RESET) => {
                Some(("mail:send:global:reset", self.limits.global_reset))
            }
            Some(TEMPLATE_VERIFICATION) => {
                Some(("mail:send:global:verify", self.limits.global_verify))
            }
            _ => None,
        }
    }

    /// Refuses every send while the bounce guard holds the pause (A29).
    async fn check_paused(&self) -> Result<(), MailError> {
        let Ok(Some(ban)) = self.gate_store.ban_status(GUARD_PAUSE_KEY).await else {
            return Ok(());
        };
        let retry = (ban.block_until_at - Utc::now().timestamp()).max(1) as u64;
        telemetry::mail_router_metrics().throttled.add(1, &[]);
        Err(MailError::Throttled {
            retry_after_secs: retry,
        })
    }

    async fn check_rate(
        &self,
        recipient: &str,
        template: Option<&str>,
        transactional: bool,
    ) -> Result<(), MailError> {
        self.check_paused().await?;
        if let Some((key, cfg)) = self.global_cap(template) {
            self.enforce(key, cfg).await?;
        }
        let domain = recipient.split('@').nth(1).unwrap_or("unknown");
        self.enforce(&format!("mail:send:domain:{domain}"), self.limits.domain)
            .await?;
        // Transactional mail gets its own (looser) bucket: a shared key would
        // mix two configs into one attempt deque and let a reset-mail burst
        // burn the marketing budget (or vice versa).
        let (rcpt_key, rcpt_cfg) = if transactional {
            (
                format!("mail:send:rcpt:txn:{recipient}"),
                self.limits.rcpt_transactional,
            )
        } else {
            (format!("mail:send:rcpt:{recipient}"), self.limits.rcpt)
        };
        self.enforce(&rcpt_key, rcpt_cfg).await?;
        // Provider-wide circuit breaker is marketing-only; transactional sends are per-user limited at the controller.
        if !transactional {
            self.enforce(
                &format!("mail:send:provider:{}", self.registry.default_provider()),
                self.limits.provider,
            )
            .await?;
        }
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

    /// Per-recipient, per-template send cooldown key (A29). It uses the dedup
    /// primitive, not the abuse limiter, so it never becomes a lockout lever.
    fn cooldown_key(msg: &OutboundEmail) -> String {
        format!(
            "mail:send:cooldown:{}:{}",
            msg.template.unwrap_or("default"),
            msg.to
        )
    }

    fn record_sent(&self) {
        let mut w = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        w.roll(current_hour());
        w.sent += 1;
    }

    /// Counts one bounce toward the hour's rate and pauses sends when it crosses
    /// the threshold. Called for send-time bounces and for webhook bounces (A29).
    pub async fn note_bounce(&self) {
        let (sent, bounced, tripped) = {
            let mut w = self.guard.lock().unwrap_or_else(|e| e.into_inner());
            w.roll(current_hour());
            w.bounced += 1;
            let tripped =
                w.sent >= GUARD_MIN_SAMPLE && w.bounced * 100 > w.sent * GUARD_MAX_BOUNCE_PCT;
            (w.sent, w.bounced, tripped)
        };
        if !tripped {
            return;
        }
        let already = matches!(
            self.gate_store.ban_status(GUARD_PAUSE_KEY).await,
            Ok(Some(_))
        );
        if let Err(e) = self
            .gate_store
            .set_block(
                GUARD_PAUSE_KEY,
                BlockScope::Temp,
                Duration::from_secs(GUARD_PAUSE_SECS as u64),
            )
            .await
        {
            warn!(error = %e, "bounce guard could not set pause");
            return;
        }
        if already {
            return;
        }
        warn!(sent, bounced, "mail bounce guard paused sends");
        if let Err(e) = audit::record(
            &self.db,
            AuditEvent {
                actor_id: None,
                action: AuditAction::MailGuardPause,
                resource_id: "mail:send".to_string(),
                metadata: json!({
                    "sent": sent,
                    "bounced": bounced,
                    "pause_secs": GUARD_PAUSE_SECS,
                }),
            },
        )
        .await
        {
            warn!(error = %e, "audit write failed for bounce guard pause");
        }
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
                blocks_transactional: true,
            };
            match SuppressionEntity::upsert(&self.db, &canonical, upsert).await {
                Ok(_) => router_metrics.bounced_sync.add(1, &[]),
                Err(e) => warn!(error = %e, "failed to upsert sync-bounce suppression"),
            }
            self.note_bounce().await;
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

        // Fail closed on a lookup error for every template (A29). Enforcement
        // notices keep their complaint exemption (A21).
        let suppressed = match self.lookup_suppression(&msg.to).await {
            Ok(Some(row)) => self.suppression_blocks(&row, msg.template, Utc::now().timestamp()),
            Ok(None) => false,
            Err(()) => {
                router_metrics.suppression_check_failed.add(1, &[]);
                true
            }
        };
        if suppressed {
            router_metrics.suppressed.add(1, &[]);
            if transactional {
                // Anti-enumeration: MUST return Ok and look identical to a real send — erroring would leak that the account is suppressed.
                // The receipt flag is internal: callers that must not reveal suppression ignore it.
                debug!(template = ?msg.template, "suppressed transactional send dropped");
                return Ok(SendReceipt {
                    suppressed: true,
                    ..SendReceipt::default()
                });
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

        // Enforcement notices skip the cooldown: each one is an admin action on a
        // case, and an admin resend must go out at once (A21).
        let cooldown_key =
            (msg.template != Some(TEMPLATE_ENFORCEMENT_NOTICE)).then(|| Self::cooldown_key(&msg));
        if let Some(key) = &cooldown_key {
            if !rux_request_gate::dedup_nx(&self.gate_store, key, self.limits.cooldown_secs).await {
                self.release_dedup(dedup_claim.as_deref()).await;
                router_metrics.throttled.add(1, &[]);
                return Err(MailError::Throttled {
                    retry_after_secs: self.limits.cooldown_secs as u64,
                });
            }
        }

        // Transactional sends are also per-user limited at the controller; here they pass under the looser transactional bucket (30/hour) instead of no cap at all.
        if self.rate_limit_enabled {
            if let Err(e) = self.check_rate(&msg.to, msg.template, transactional).await {
                self.release_dedup(dedup_claim.as_deref()).await;
                self.release_dedup(cooldown_key.as_deref()).await;
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
                self.release_dedup(cooldown_key.as_deref()).await;
                return Err(e);
            }
        };

        metrics.emails_sent.add(1, &[]);
        metrics
            .send_duration
            .record(start.elapsed().as_millis() as f64, &[]);
        tracing::Span::current().record("result", "success");
        self.record_sent();

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
    use crate::db::sea_models::email_suppression::Entity as Suppression;
    use crate::services::mail::provider::{TEMPLATE_PASSWORD_RESET, TEMPLATE_VERIFICATION};
    use migration::{Migrator, MigratorTrait};
    use sea_orm::ConnectionTrait;

    async fn router_with(db: DatabaseConnection, limits: MailRouterLimits) -> MailRouter {
        use crate::services::mail::none::NoOpMailProvider;
        use std::collections::HashMap;

        let providers: HashMap<String, Arc<dyn MailProvider>> = [(
            "none".to_string(),
            Arc::new(NoOpMailProvider) as Arc<dyn MailProvider>,
        )]
        .into_iter()
        .collect();
        MailRouter::new(
            providers,
            "none".to_string(),
            Arc::new(rux_request_gate::InMemoryStore::default()),
            db,
            limits,
            true,
        )
    }

    async fn migrated_db() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite connects");
        Migrator::up(&db, None).await.expect("migrations run");
        db
    }

    /// Cooldown off, so repeat sends to one address exercise the hourly caps.
    async fn capped_router() -> MailRouter {
        let limits = MailRouterLimits {
            cooldown_secs: 0,
            ..MailRouterLimits::default()
        };
        router_with(migrated_db().await, limits).await
    }

    fn txn_msg(to: &str) -> OutboundEmail {
        OutboundEmail {
            to: to.to_string(),
            subject: "Password reset verification code".to_string(),
            html: None,
            text: None,
            template: Some(TEMPLATE_PASSWORD_RESET),
            list_unsubscribe: None,
        }
    }

    fn verify_msg(to: &str) -> OutboundEmail {
        OutboundEmail {
            subject: "Email verification code".to_string(),
            template: Some(TEMPLATE_VERIFICATION),
            ..txn_msg(to)
        }
    }

    #[tokio::test]
    async fn transactional_sends_cap_at_30_per_hour() {
        let router = capped_router().await;

        for _ in 0..29 {
            assert!(
                router.send(txn_msg("victim@example.com")).await.is_ok(),
                "transactional sends below the hourly threshold must pass"
            );
        }
        // The 30th attempt within the hour trips the temp block.
        match router.send(txn_msg("victim@example.com")).await {
            Err(MailError::Throttled { .. }) => {}
            other => panic!("expected Throttled on the 30th send, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn transactional_sends_do_not_burn_the_marketing_rcpt_bucket() {
        let router = capped_router().await;

        for _ in 0..10 {
            assert!(router.send(txn_msg("victim@example.com")).await.is_ok());
        }
        let mut m = txn_msg("victim@example.com");
        m.template = Some(TEMPLATE_NEWSLETTER);
        m.subject = "Weekly".to_string();
        assert!(
            router.send(m).await.is_ok(),
            "marketing bucket must stay separate — a shared key would temp-block at 5 attempts"
        );
    }

    #[tokio::test]
    async fn registration_burst_cannot_exceed_global_verify_cap() {
        let router = capped_router().await;
        let mut sent = 0usize;
        let mut throttled = 0usize;
        // 10,000 distinct addresses across 500 domains: per-domain and per-recipient
        // caps stay out of the way, so only the global verification cap can refuse.
        for i in 0..10_000usize {
            let to = format!("user{i}@d{}.test", i % 500);
            match router.send(verify_msg(&to)).await {
                Ok(_) => sent += 1,
                Err(MailError::Throttled { .. }) => throttled += 1,
                Err(other) => panic!("unexpected error: {other:?}"),
            }
        }
        assert_eq!(sent, GLOBAL_VERIFY_CAP);
        assert_eq!(throttled, 10_000 - GLOBAL_VERIFY_CAP);
    }

    #[tokio::test]
    async fn domain_cap_applies_to_every_template() {
        let router = capped_router().await;
        let mut sent = 0usize;
        for i in 0..(DOMAIN_CAP + 5) {
            if router
                .send(verify_msg(&format!("a{i}@shared.test")))
                .await
                .is_ok()
            {
                sent += 1;
            }
        }
        assert_eq!(sent, DOMAIN_CAP);
    }

    #[tokio::test]
    async fn repeat_send_inside_cooldown_is_throttled() {
        let router = router_with(migrated_db().await, MailRouterLimits::default()).await;
        assert!(router.send(verify_msg("once@x.test")).await.is_ok());
        match router.send(verify_msg("once@x.test")).await {
            Err(MailError::Throttled { retry_after_secs }) => {
                assert_eq!(retry_after_secs, SEND_COOLDOWN_SECS as u64);
            }
            other => panic!("expected cooldown Throttled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bounce_guard_pauses_above_five_percent_and_audits_once() {
        let router = capped_router().await;
        for i in 0..40usize {
            assert!(router
                .send(verify_msg(&format!("ok{i}@guard.test")))
                .await
                .is_ok());
        }
        // 2 of 40 is exactly 5 percent: not above the threshold.
        router.note_bounce().await;
        router.note_bounce().await;
        assert!(router
            .gate_store
            .ban_status(GUARD_PAUSE_KEY)
            .await
            .unwrap()
            .is_none());
        // The third and fourth bounces cross it.
        router.note_bounce().await;
        router.note_bounce().await;
        assert!(router
            .gate_store
            .ban_status(GUARD_PAUSE_KEY)
            .await
            .unwrap()
            .is_some());
        match router.send(verify_msg("paused@guard.test")).await {
            Err(MailError::Throttled { .. }) => {}
            other => panic!("expected paused Throttled, got {other:?}"),
        }
        let row = router
            .db
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM audit_logs WHERE action = 'mail.guard_pause'".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let n: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(n, 1, "pause writes one audit row, not one per extra bounce");
    }

    #[tokio::test]
    async fn webhook_suppression_blocks_marketing_not_transactional_until_reviewed() {
        let router = capped_router().await;
        Suppression::upsert(
            &router.db,
            "victim@webhook.test",
            SuppressionUpsert {
                reason: SuppressionReason::Complaint,
                source: Some("mail-webhook".to_string()),
                diagnostic: None,
                permanent: true,
                blocks_transactional: false,
            },
        )
        .await
        .unwrap();

        let receipt = router
            .send(verify_msg("victim@webhook.test"))
            .await
            .expect("transactional mail passes an unreviewed webhook suppression");
        assert!(!receipt.suppressed);

        let mut news = txn_msg("victim@webhook.test");
        news.template = Some(TEMPLATE_NEWSLETTER);
        assert!(matches!(
            router.send(news).await,
            Err(MailError::Suppressed)
        ));

        Suppression::review(&router.db, "victim@webhook.test")
            .await
            .unwrap();
        let receipt = router
            .send(verify_msg("victim@webhook.test"))
            .await
            .unwrap();
        assert!(receipt.suppressed, "reviewed row blocks transactional mail");
    }

    #[tokio::test]
    async fn lookup_failure_fails_closed_for_every_template() {
        // No migrations: the suppression table is missing, so the lookup errors.
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let router = router_with(db, MailRouterLimits::default()).await;
        assert!(matches!(
            router.send(txn_msg("closed@x.test")).await,
            Ok(SendReceipt {
                suppressed: true,
                ..
            })
        ));
        let mut news = txn_msg("closed@x.test");
        news.template = Some(TEMPLATE_NEWSLETTER);
        assert!(matches!(
            router.send(news).await,
            Err(MailError::Suppressed)
        ));
    }

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
