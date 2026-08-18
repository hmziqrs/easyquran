use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use ruxlog_types::enums::SubscriptionStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub session_id: String,
    pub checkout_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub provider_subscription_id: String,
    pub status: String,
    pub current_period_end: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub cancel_at_period_end: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    pub provider_payment_id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
}

pub use rux_provider_core::WebhookEvent;

pub mod canonical {
    pub const CHECKOUT_COMPLETED: &str = "checkout.session.completed";
    pub const SUBSCRIPTION_UPDATED: &str = "customer.subscription.updated";
    pub const SUBSCRIPTION_DELETED: &str = "customer.subscription.deleted";
    pub const PAYMENT_SUCCEEDED: &str = "invoice.payment_succeeded";
    pub const PAYMENT_CONFIRMED: &str = "payment.confirmed";
    pub const PAYMENT_PENDING: &str = "payment.pending";
}

/// Refund/chargeback/dispute natives that provider normalization passes through
/// unmapped (Stripe, PayPal) or arrives verbatim from providers with their own
/// spelling (Airwallex `refund.created`, Lemon Squeezy `order_refunded`); the
/// controller revokes the linked entitlement for these.
pub fn is_refund_or_dispute_event_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "charge.refunded"
            | "charge.dispute.created"
            | "refund.created"
            | "order_refunded"
            | "PAYMENT.SALE.REFUNDED"
            | "PAYMENT.CAPTURE.REFUNDED"
            | "PAYMENT.SALE.REVERSED"
    )
}

pub fn canonical_subscription_status(raw: Option<&str>) -> Option<SubscriptionStatus> {
    let s = raw?.trim().to_ascii_lowercase();
    Some(match s.as_str() {
        "active"
        | "activated"
        | "subscription_active"
        | "incomplete_active"
        | "running"
        | "authorized" => SubscriptionStatus::Active,
        "trialing" | "trialling" | "trial" | "in_trial" | "pending_trial" | "on_trial" => {
            SubscriptionStatus::Trialing
        }
        "past_due" | "pastdue" | "unpaid" | "problem" | "suspended" | "paused" | "on_hold"
        | "incomplete" => SubscriptionStatus::PastDue,
        "canceled" | "cancelled" | "subscription_cancelled" | "revoked" => {
            SubscriptionStatus::Canceled
        }
        "expired" | "halted" | "failed" | "completed" | "ended" => SubscriptionStatus::Expired,
        _ => return None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedWebhook {
    pub event_type: String,
    pub customer_id: String,
    pub subscription_id: Option<String>,
    pub payment_id: Option<String>,
    pub current_period_end: Option<i64>,
    pub checkout_session_id: Option<String>,
    pub subscription_status: Option<String>,
    pub user_id: Option<i32>,
    pub amount_cents: Option<i64>,
    pub currency: Option<String>,
    pub data: serde_json::Value,
}

pub fn period_end_to_unix(value: Option<&serde_json::Value>) -> Option<i64> {
    let v = value?;
    if let Some(n) = v.as_i64() {
        return Some(if n > 1_000_000_000_000 { n / 1000 } else { n });
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if let Ok(n) = trimmed.parse::<i64>() {
            return Some(if n > 1_000_000_000_000 { n / 1000 } else { n });
        }
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
            return Some(dt.timestamp());
        }
    }
    None
}

#[async_trait]
pub trait BillingProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;

    async fn create_checkout(
        &self,
        plan_slug: &str,
        customer_email: &str,
        user_id: i32,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<CheckoutSession, BillingError>;

    #[allow(clippy::too_many_arguments)]
    async fn create_post_checkout(
        &self,
        _post_id: i32,
        _amount_cents: i32,
        _currency: &str,
        _customer_email: &str,
        _user_id: i32,
        _success_url: &str,
        _cancel_url: &str,
    ) -> Result<CheckoutSession, BillingError> {
        Err(BillingError::Config(format!(
            "per-post checkout not supported by provider '{}'",
            self.provider_name()
        )))
    }

    async fn cancel_subscription(
        &self,
        provider_subscription_id: &str,
        immediately: bool,
    ) -> Result<(), BillingError>;

    async fn get_subscription(
        &self,
        provider_subscription_id: &str,
    ) -> Result<SubscriptionInfo, BillingError>;

    async fn verify_webhook(&self, event: WebhookEvent) -> Result<ParsedWebhook, BillingError>;

    async fn create_portal_session(
        &self,
        provider_customer_id: &str,
        return_url: &str,
    ) -> Result<String, BillingError>;
}

impl rux_provider_core::Provider for dyn BillingProvider {}

#[derive(Debug, thiserror::Error)]
pub enum BillingError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Provider API error: {0}")]
    ProviderApi(String),

    #[error("Webhook verification failed: {0}")]
    WebhookVerification(String),

    #[error("Subscription not found: {0}")]
    SubscriptionNotFound(String),

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("{0}")]
    Other(String),
}

impl From<rux_provider_core::FrameworkError> for BillingError {
    fn from(err: rux_provider_core::FrameworkError) -> Self {
        match err {
            rux_provider_core::FrameworkError::ProviderNotRegistered(name) => {
                BillingError::Config(format!("Provider '{}' not initialized", name))
            }
            rux_provider_core::FrameworkError::Config(msg) => BillingError::Config(msg),
            rux_provider_core::FrameworkError::ProviderApi(msg) => BillingError::ProviderApi(msg),
            rux_provider_core::FrameworkError::Other(msg) => BillingError::Other(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_subscription_status, is_refund_or_dispute_event_type, period_end_to_unix,
        SubscriptionStatus,
    };
    use serde_json::json;

    #[test]
    fn refund_dispute_event_types_are_detected_across_providers() {
        for raw in [
            "charge.refunded",
            "charge.dispute.created",
            "refund.created",
            "order_refunded",
            "PAYMENT.SALE.REFUNDED",
            "PAYMENT.CAPTURE.REFUNDED",
            "PAYMENT.SALE.REVERSED",
        ] {
            assert!(is_refund_or_dispute_event_type(raw), "raw={raw}");
        }
    }

    #[test]
    fn non_refund_event_types_are_not_flagged() {
        for raw in [
            "checkout.session.completed",
            "customer.subscription.deleted",
            "invoice.payment_succeeded",
            "payment.confirmed",
            "PAYMENT.SALE.COMPLETED",
            "order_created",
        ] {
            assert!(!is_refund_or_dispute_event_type(raw), "raw={raw}");
        }
    }

    #[test]
    fn period_end_handles_epoch_seconds_int() {
        assert_eq!(
            period_end_to_unix(Some(&json!(1_700_000_000))),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn period_end_normalizes_milliseconds_int() {
        assert_eq!(
            period_end_to_unix(Some(&json!(1_700_000_000_000i64))),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn period_end_handles_epoch_string() {
        assert_eq!(
            period_end_to_unix(Some(&json!("1700000000"))),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn period_end_handles_rfc3339_string() {
        let v = json!("2023-11-14T22:13:20Z");
        assert_eq!(period_end_to_unix(Some(&v)), Some(1_700_000_000));
    }

    #[test]
    fn period_end_none_for_garbage_or_missing() {
        assert_eq!(period_end_to_unix(None), None);
        assert_eq!(period_end_to_unix(Some(&json!("not-a-date"))), None);
        assert_eq!(period_end_to_unix(Some(&json!(null))), None);
    }

    #[test]
    fn canonical_status_folds_active_vocabulary() {
        for raw in [
            "active",
            "ACTIVE", // case-insensitive
            "activated",
            "subscription_active",
            "incomplete_active",
            "running",
            "authorized", // Mercado Pago preapproval (audit F#11)
        ] {
            assert_eq!(
                canonical_subscription_status(Some(raw)),
                Some(SubscriptionStatus::Active),
                "raw={raw}"
            );
        }
    }

    #[test]
    fn canonical_status_folds_trialing_vocabulary() {
        for raw in [
            "trialing",
            "trialling",
            "trial",
            "in_trial",
            "pending_trial",
            "on_trial",
        ] {
            assert_eq!(
                canonical_subscription_status(Some(raw)),
                Some(SubscriptionStatus::Trialing),
                "raw={raw}"
            );
        }
    }

    #[test]
    fn canonical_status_folds_past_due_vocabulary() {
        for raw in [
            "past_due",
            "pastdue",
            "unpaid",
            "problem",
            "suspended",
            "paused",
            "on_hold",
            "incomplete",
        ] {
            assert_eq!(
                canonical_subscription_status(Some(raw)),
                Some(SubscriptionStatus::PastDue),
                "raw={raw}"
            );
        }
    }

    #[test]
    fn canonical_status_folds_canceled_vocabulary() {
        for raw in ["canceled", "cancelled", "subscription_cancelled", "revoked"] {
            assert_eq!(
                canonical_subscription_status(Some(raw)),
                Some(SubscriptionStatus::Canceled),
                "raw={raw}"
            );
        }
    }

    #[test]
    fn canonical_status_folds_expired_vocabulary() {
        for raw in ["expired", "halted", "failed", "completed", "ended"] {
            assert_eq!(
                canonical_subscription_status(Some(raw)),
                Some(SubscriptionStatus::Expired),
                "raw={raw}"
            );
        }
    }

    #[test]
    fn canonical_status_none_for_unrecognized_or_missing() {
        assert_eq!(canonical_subscription_status(Some("authenticated")), None);
        assert_eq!(canonical_subscription_status(Some("nonsense")), None);
        assert_eq!(canonical_subscription_status(None), None);
    }
}
