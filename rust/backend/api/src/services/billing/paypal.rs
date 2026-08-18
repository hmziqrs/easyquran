use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::provider::{
    BillingError, BillingProvider, CheckoutSession, ParsedWebhook, SubscriptionInfo, WebhookEvent,
};

use crate::state::build_http_client;

const TOKEN_REFRESH_MARGIN_SECS: u64 = 60;
const TOKEN_MIN_TTL_SECS: u64 = 30;
const TOKEN_FALLBACK_TTL_SECS: u64 = 300;

struct CachedAccessToken {
    token: String,
    expires_at: Instant,
}

fn token_ttl_from_expires_in(expires_in_secs: Option<u64>) -> Duration {
    let ttl = expires_in_secs.unwrap_or(TOKEN_FALLBACK_TTL_SECS);
    Duration::from_secs(
        ttl.saturating_sub(TOKEN_REFRESH_MARGIN_SECS)
            .max(TOKEN_MIN_TTL_SECS),
    )
}

pub struct PayPalProvider {
    pub client_id: String,
    pub client_secret: SecretString,
    pub webhook_secret: SecretString,
    pub webhook_id: Option<String>,
    pub base_url: String,
    pub http_client: reqwest::Client,
    access_token_cache: Mutex<Option<CachedAccessToken>>,
}

impl PayPalProvider {
    pub fn new(client_id: String, client_secret: String, webhook_secret: String) -> Self {
        Self {
            client_id,
            client_secret: client_secret.into(),
            webhook_secret: webhook_secret.into(),
            webhook_id: None,
            base_url: std::env::var("PAYPAL_API_BASE_URL")
                .unwrap_or_else(|_| "https://api-m.paypal.com".to_string()),
            http_client: build_http_client(),
            access_token_cache: Mutex::new(None),
        }
    }

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    pub fn with_webhook_id(mut self, webhook_id: String) -> Self {
        self.webhook_id = Some(webhook_id);
        self
    }

    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = client;
        self
    }
}

impl PayPalProvider {
    async fn get_access_token(&self) -> Result<String, BillingError> {
        // Unauthenticated webhook verification hits this too — serve from cache
        // until expiry so junk traffic cannot mint an outbound OAuth call per request.
        {
            let cache = self.access_token_cache.lock().map_err(|e| {
                tracing::error!(error = %e, "paypal access token cache lock poisoned");
                BillingError::Other("paypal access token cache lock poisoned".to_string())
            })?;
            if let Some(cached) = cache.as_ref() {
                let still_valid = Instant::now() + Duration::from_secs(TOKEN_REFRESH_MARGIN_SECS)
                    < cached.expires_at;
                if still_valid {
                    return Ok(cached.token.clone());
                }
            }
        }

        let client = self.http_client.clone();
        let resp = client
            .post(format!("{}/v1/oauth2/token", self.base_url))
            .header("Accept", "application/json")
            .header("Accept-Language", "en_US")
            .form(&[("grant_type", "client_credentials")])
            .basic_auth(&self.client_id, Some(self.client_secret.expose_secret()))
            .send()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BillingError::ProviderApi(format!("Auth failed: {}", body)));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        let token = data["access_token"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| BillingError::ProviderApi("No access_token in response".to_string()))?;
        let ttl = token_ttl_from_expires_in(data["expires_in"].as_u64());

        let mut cache = self.access_token_cache.lock().map_err(|e| {
            tracing::error!(error = %e, "paypal access token cache lock poisoned");
            BillingError::Other("paypal access token cache lock poisoned".to_string())
        })?;
        *cache = Some(CachedAccessToken {
            token: token.clone(),
            expires_at: Instant::now() + ttl,
        });

        Ok(token)
    }

    async fn fetch_subscription_period_end(&self, subscription_id: &str) -> Option<i64> {
        let token = self.get_access_token().await.ok()?;
        let client = self.http_client.clone();
        let url = format!(
            "{}/v1/billing/subscriptions/{}",
            self.base_url, subscription_id
        );
        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let data: serde_json::Value = resp.json().await.ok()?;
        super::provider::period_end_to_unix(
            data.get("billing_info")
                .and_then(|b| b.get("next_billing_time")),
        )
    }
}

impl std::fmt::Debug for PayPalProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayPalProvider")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("webhook_secret", &"<redacted>")
            .field("webhook_id", &self.webhook_id)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl BillingProvider for PayPalProvider {
    fn provider_name(&self) -> &'static str {
        "paypal"
    }

    async fn create_checkout(
        &self,
        plan_slug: &str,
        customer_email: &str,
        user_id: i32,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<CheckoutSession, BillingError> {
        let token = self.get_access_token().await?;
        let client = self.http_client.clone();

        // session_id must be the subscription id (`I-…`): BILLING.SUBSCRIPTION.ACTIVATED echoes it back and the checkout intent only recovers on that match.
        let body = serde_json::json!({
            "plan_id": plan_slug,
            "custom_id": user_id.to_string(),
            "subscriber": { "email_address": customer_email },
            "application_context": {
                "brand_name": "Ruxlog",
                "user_action": "SUBSCRIBE_NOW",
                "return_url": success_url,
                "cancel_url": cancel_url,
                "shipping_preference": "NO_SHIPPING",
            },
        });

        let resp = client
            .post(format!("{}/v1/billing/subscriptions", self.base_url))
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .header("PayPal-Request-Id", uuid::Uuid::new_v4().to_string())
            .json(&body)
            .send()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BillingError::ProviderApi(body));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        let checkout_url = data["links"]
            .as_array()
            .and_then(|links| {
                links
                    .iter()
                    .find(|link| link["rel"].as_str() == Some("approve"))
                    .and_then(|link| link["href"].as_str().map(String::from))
            })
            .unwrap_or_default();

        Ok(CheckoutSession {
            session_id: data["id"].as_str().unwrap_or_default().to_string(),
            checkout_url,
        })
    }

    async fn cancel_subscription(
        &self,
        provider_subscription_id: &str,
        immediately: bool,
    ) -> Result<(), BillingError> {
        let token = self.get_access_token().await?;
        let client = self.http_client.clone();
        let url = format!(
            "{}/v1/billing/subscriptions/{}/cancel",
            self.base_url, provider_subscription_id
        );

        let reason = if immediately {
            "Cancelled immediately by admin"
        } else {
            "Cancelled at end of billing cycle"
        };

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "reason": reason }))
            .send()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BillingError::ProviderApi(body));
        }

        Ok(())
    }

    async fn get_subscription(
        &self,
        provider_subscription_id: &str,
    ) -> Result<SubscriptionInfo, BillingError> {
        let token = self.get_access_token().await?;
        let client = self.http_client.clone();
        let url = format!(
            "{}/v1/billing/subscriptions/{}",
            self.base_url, provider_subscription_id
        );

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BillingError::ProviderApi(body));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        let current_end = data["billing_info"]["next_billing_time"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());

        Ok(SubscriptionInfo {
            provider_subscription_id: data["id"].as_str().unwrap_or_default().to_string(),
            status: data["status"].as_str().unwrap_or_default().to_string(),
            current_period_end: current_end,
            cancel_at_period_end: false,
        })
    }

    async fn verify_webhook(&self, event: WebhookEvent) -> Result<ParsedWebhook, BillingError> {
        // PayPal webhooks are cert-signed, verified via their verify-webhook-signature API, not a local HMAC (webhook_secret can't validate them).
        let webhook_id = self.webhook_id.as_ref().ok_or_else(|| {
            BillingError::WebhookVerification(
                "PAYPAL_WEBHOOK_ID not configured; cannot verify PayPal webhook".into(),
            )
        })?;

        let header =
            |name: &str| super::webhook_util::header_str(&event.headers, name).unwrap_or_default();
        let transmission_id = header("PAYPAL-TRANSMISSION-ID");
        let transmission_time = header("PAYPAL-TRANSMISSION-TIME");
        let cert_url = header("PAYPAL-CERT-URL");
        let auth_algo = header("PAYPAL-AUTH-ALGO");
        let transmission_sig = header("PAYPAL-TRANSMISSION-SIG");

        if transmission_id.is_empty()
            || transmission_time.is_empty()
            || cert_url.is_empty()
            || auth_algo.is_empty()
            || transmission_sig.is_empty()
        {
            return Err(BillingError::WebhookVerification(
                "PayPal webhook missing required transmission headers".into(),
            ));
        }

        let payload_str = std::str::from_utf8(&event.payload)
            .map_err(|e| BillingError::WebhookVerification(e.to_string()))?;
        let webhook_event: serde_json::Value = serde_json::from_str(payload_str)
            .map_err(|e| BillingError::WebhookVerification(e.to_string()))?;

        let access_token = self.get_access_token().await?;

        let verify_body = serde_json::json!({
            "transmission_id": transmission_id,
            "transmission_time": transmission_time,
            "cert_url": cert_url,
            "auth_algo": auth_algo,
            "transmission_sig": transmission_sig,
            "webhook_id": webhook_id,
            "webhook_event": webhook_event,
        });

        let client = self.http_client.clone();
        let resp = client
            .post(format!(
                "{}/v1/notifications/verify-webhook-signature",
                self.base_url
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&verify_body)
            .send()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BillingError::WebhookVerification(format!(
                "PayPal verify-webhook-signature call failed: {body}"
            )));
        }

        let vdata: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BillingError::WebhookVerification(e.to_string()))?;
        let status = vdata["verification_status"].as_str().unwrap_or("");
        if status != "SUCCESS" {
            return Err(BillingError::WebhookVerification(format!(
                "PayPal webhook verification_status={status}"
            )));
        }

        let native_event = webhook_event["event_type"].as_str().unwrap_or_default();
        let event_type = match native_event {
            "BILLING.SUBSCRIPTION.ACTIVATED" => super::provider::canonical::CHECKOUT_COMPLETED,
            "BILLING.SUBSCRIPTION.CANCELLED" => super::provider::canonical::SUBSCRIPTION_DELETED,
            "BILLING.SUBSCRIPTION.UPDATED"
            | "BILLING.SUBSCRIPTION.EXPIRED"
            | "BILLING.SUBSCRIPTION.SUSPENDED" => super::provider::canonical::SUBSCRIPTION_UPDATED,
            "PAYMENT.SALE.COMPLETED" | "PAYMENT.CAPTURE.COMPLETED" => {
                super::provider::canonical::PAYMENT_SUCCEEDED
            }
            other => other,
        }
        .to_string();
        let resource = &webhook_event["resource"];
        let resource_id = resource["id"].as_str().map(String::from);
        let billing_agreement_id = resource["billing_agreement_id"].as_str().map(String::from);

        // SALE/CAPTURE resource.id is the sale (`S-…`), not the sub; the recurring sub is billing_agreement_id (`I-…`) — using the sale id breaks dispatch. Refund/reversal events carry the same shape.
        let is_sale = matches!(
            native_event,
            "PAYMENT.SALE.COMPLETED"
                | "PAYMENT.CAPTURE.COMPLETED"
                | "PAYMENT.SALE.REFUNDED"
                | "PAYMENT.CAPTURE.REFUNDED"
                | "PAYMENT.SALE.REVERSED"
        );
        let subscription_id = if is_sale {
            billing_agreement_id.clone()
        } else {
            resource_id.clone()
        };

        // SALE events have no inline next_billing_time; fetch the linked sub so current_period_end refreshes — failures degrade to None (fail-closed).
        let inline_period_end = super::provider::period_end_to_unix(
            resource
                .get("billing_info")
                .and_then(|b| b.get("next_billing_time")),
        );
        let current_period_end = match inline_period_end {
            Some(ts) => Some(ts),
            None => match subscription_id.as_deref() {
                Some(sub_id) if !sub_id.is_empty() => {
                    self.fetch_subscription_period_end(sub_id).await
                }
                _ => None,
            },
        };

        Ok(ParsedWebhook {
            event_type,
            customer_id: resource["subscriber"]["payer_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            subscription_id,
            payment_id: resource_id.clone(),
            checkout_session_id: resource_id,
            current_period_end,
            subscription_status: resource["status"].as_str().map(String::from),
            user_id: resource["custom_id"].as_str().and_then(|s| s.parse().ok()),
            amount_cents: resource["amount"]["total"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok().map(|f| (f * 100.0) as i64))
                .or_else(|| {
                    resource["amount"]["value"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                }),
            currency: resource["amount"]["currency_code"]
                .as_str()
                .map(String::from),
            data: webhook_event,
        })
    }

    async fn create_portal_session(
        &self,
        provider_customer_id: &str,
        return_url: &str,
    ) -> Result<String, BillingError> {
        let portal_base = if self.base_url.contains("sandbox") {
            "https://www.sandbox.paypal.com"
        } else {
            "https://www.paypal.com"
        };
        Ok(format!(
            "{portal_base}/myaccount/autopay/connect/{}?return_url={}",
            provider_customer_id,
            urlencoding::encode(return_url)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paypal_provider_name() {
        let provider = PayPalProvider::new("cid".into(), "secret".into(), "whsec".into());
        assert_eq!(provider.provider_name(), "paypal");
    }

    #[test]
    fn test_paypal_new() {
        let provider = PayPalProvider::new(
            "AWx_test_client_id".into(),
            "test_secret_key".into(),
            "whsec_test".into(),
        );
        assert_eq!(provider.client_id, "AWx_test_client_id");
        assert_eq!(provider.client_secret.expose_secret(), "test_secret_key");
        assert_eq!(provider.webhook_secret.expose_secret(), "whsec_test");
        assert_eq!(provider.base_url, "https://api-m.paypal.com");
    }

    #[test]
    fn test_paypal_custom_base_url() {
        let provider = PayPalProvider::new("c".into(), "s".into(), "w".into())
            .with_base_url("http://localhost:9999".into());
        assert_eq!(provider.base_url, "http://localhost:9999");
    }

    #[test]
    fn token_ttl_applies_refresh_margin_and_floor() {
        use std::time::Duration;

        assert_eq!(
            token_ttl_from_expires_in(Some(32_400)),
            Duration::from_secs(32_340)
        );
        assert_eq!(
            token_ttl_from_expires_in(Some(45)),
            Duration::from_secs(TOKEN_MIN_TTL_SECS)
        );
        assert_eq!(
            token_ttl_from_expires_in(None),
            Duration::from_secs(TOKEN_FALLBACK_TTL_SECS - TOKEN_REFRESH_MARGIN_SECS)
        );
    }

    #[tokio::test]
    async fn unexpired_cached_token_is_served_without_network() {
        let provider = PayPalProvider::new("c".into(), "s".into(), "w".into())
            .with_base_url("http://localhost:9".into());
        provider
            .access_token_cache
            .lock()
            .unwrap()
            .replace(CachedAccessToken {
                token: "cached_token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(3600),
            });
        let token = provider.get_access_token().await.expect("cache hit");
        assert_eq!(token, "cached_token");
    }

    #[tokio::test]
    async fn expired_cached_token_bypasses_to_provider() {
        let provider = PayPalProvider::new("c".into(), "s".into(), "w".into())
            .with_base_url("http://localhost:9".into());
        provider
            .access_token_cache
            .lock()
            .unwrap()
            .replace(CachedAccessToken {
                token: "stale_token".to_string(),
                expires_at: Instant::now(),
            });
        // Nothing listens on localhost:9 — a connection error proves the stale
        // entry was bypassed instead of served.
        assert!(provider.get_access_token().await.is_err());
    }
}
