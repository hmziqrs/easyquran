use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};

use super::provider::{
    BillingError, BillingProvider, CheckoutSession, ParsedWebhook, SubscriptionInfo, WebhookEvent,
};

use crate::state::build_http_client;

pub struct MercadoPagoProvider {
    pub access_token: SecretString,
    pub webhook_secret: SecretString,
    pub base_url: String,
    pub http_client: reqwest::Client,
}

fn extract_query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == name {
            let val = kv.next()?;
            return Some(url_decode(val));
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                if let Some(b) = hex_nibble(bytes[i + 1])
                    .and_then(|hi| hex_nibble(bytes[i + 2]).map(|lo| (hi << 4) | lo))
                {
                    out.push(b);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn ts_unit_to_secs(ts: i64) -> i64 {
    if ts > 10_000_000_000 {
        ts / 1000
    } else {
        ts
    }
}

fn resource_id(value: &serde_json::Value) -> Option<String> {
    if let Some(id) = value.as_str().filter(|id| !id.is_empty()) {
        return Some(id.to_string());
    }
    value.as_u64().map(|id| id.to_string())
}

fn canonical_resource_id(id: &str) -> String {
    if id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return id.to_ascii_lowercase();
    }
    id.to_string()
}

impl MercadoPagoProvider {
    pub fn new(access_token: String, webhook_secret: String) -> Self {
        Self {
            access_token: access_token.into(),
            webhook_secret: webhook_secret.into(),
            base_url: std::env::var("MERCADO_PAGO_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.mercadopago.com".to_string()),
            http_client: build_http_client(),
        }
    }

    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = client;
        self
    }
}

// Manual redacting Debug — do not replace with #[derive(Debug)] (would leak access_token and webhook_secret into logs).
impl std::fmt::Debug for MercadoPagoProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MercadoPagoProvider")
            .field("access_token", &"<redacted>")
            .field("webhook_secret", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl BillingProvider for MercadoPagoProvider {
    fn provider_name(&self) -> &'static str {
        "mercado_pago"
    }

    async fn create_checkout(
        &self,
        plan_slug: &str,
        customer_email: &str,
        user_id: i32,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<CheckoutSession, BillingError> {
        let client = self.http_client.clone();

        // notification_url MUST NOT be derived from success_url — Mercado Pago POSTs webhooks to it, so a caller-controlled value enables provider-mediated SSRF; use only the operator-configured public base.
        let notification_url = std::env::var("MERCADO_PAGO_WEBHOOK_URL")
            .ok()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .or_else(|| {
                std::env::var("CONSUMER_SITE_URL").ok().map(|base| {
                    format!(
                        "{}/billing/v1/webhook/mercado_pago",
                        base.trim_end_matches('/')
                    )
                })
            });

        let mut body = serde_json::json!({
            "items": [
                {
                    "title": format!("Plan: {}", plan_slug),
                    "quantity": 1,
                    "unit_price": plan_slug.parse::<f64>().unwrap_or(99.90),
                    "currency_id": "BRL",
                }
            ],
            "payer": {
                "email": customer_email,
            },
            "back_urls": {
                "success": success_url,
                "failure": cancel_url,
                "pending": success_url,
            },
            "auto_return": "approved",
            "external_reference": user_id.to_string(),
        });
        if let Some(url) = notification_url {
            body["notification_url"] = serde_json::Value::String(url);
        }

        let resp = client
            .post(format!("{}/checkout/preferences", self.base_url))
            .header(
                "Authorization",
                format!("Bearer {}", self.access_token.expose_secret()),
            )
            .header("Content-Type", "application/json")
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

        Ok(CheckoutSession {
            session_id: data["id"].as_str().unwrap_or_default().to_string(),
            checkout_url: data["init_point"].as_str().unwrap_or_default().to_string(),
        })
    }

    async fn cancel_subscription(
        &self,
        provider_subscription_id: &str,
        _immediately: bool,
    ) -> Result<(), BillingError> {
        let client = self.http_client.clone();
        let url = format!("{}/preapproval/{}", self.base_url, provider_subscription_id);

        let resp = client
            .put(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.access_token.expose_secret()),
            )
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "status": "cancelled" }))
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
        let client = self.http_client.clone();
        let url = format!("{}/preapproval/{}", self.base_url, provider_subscription_id);

        let resp = client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.access_token.expose_secret()),
            )
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

        let current_end = data["next_payment_date"]
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
        self.verify_webhook_at(event, chrono::Utc::now().timestamp())
            .await
    }

    async fn create_portal_session(
        &self,
        provider_customer_id: &str,
        return_url: &str,
    ) -> Result<String, BillingError> {
        Ok(format!(
            "https://www.mercadopago.com.br/subscriptions#c/{}/{}",
            provider_customer_id,
            urlencoding::encode(return_url)
        ))
    }
}

impl MercadoPagoProvider {
    pub async fn verify_webhook_at(
        &self,
        event: WebhookEvent,
        now_secs: i64,
    ) -> Result<ParsedWebhook, BillingError> {
        let sig_header = super::webhook_util::header_str(&event.headers, "x-signature")
            .ok_or_else(|| {
                BillingError::WebhookVerification("Missing x-signature header".into())
            })?;

        let mut ts: Option<&str> = None;
        let mut v1: Option<&str> = None;
        for part in sig_header.split(',') {
            let mut kv = part.splitn(2, '=');
            match kv.next().map(str::trim) {
                Some("ts") => ts = kv.next().map(str::trim),
                Some("v1") => v1 = kv.next().map(str::trim),
                _ => {}
            }
        }
        let ts =
            ts.ok_or_else(|| BillingError::WebhookVerification("x-signature missing ts=".into()))?;
        let v1 =
            v1.ok_or_else(|| BillingError::WebhookVerification("x-signature missing v1=".into()))?;

        let ts_raw: i64 = ts.parse().map_err(|_| {
            BillingError::WebhookVerification("Mercado Pago ts not an integer".into())
        })?;
        let ts_secs = ts_unit_to_secs(ts_raw);
        if !super::webhook_util::timestamp_fresh(ts_secs, now_secs) {
            return Err(BillingError::WebhookVerification(format!(
                "Mercado Pago timestamp outside tolerance (ts_raw={ts_raw}, ts_secs={ts_secs})"
            )));
        }

        let payload_str = std::str::from_utf8(&event.payload)
            .map_err(|e| BillingError::WebhookVerification(e.to_string()))?;

        let query = event.query.as_deref().ok_or_else(|| {
            BillingError::WebhookVerification(
                "Mercado Pago webhook missing URL query string (data.id)".into(),
            )
        })?;
        let data_id = extract_query_param(query, "data.id").ok_or_else(|| {
            BillingError::WebhookVerification("Mercado Pago webhook query missing data.id".into())
        })?;
        // MP signs data.id lowercased ONLY when alphanumeric (non-alphanumeric values signed as-is); don't simplify to unconditional lowercase — it breaks signatures for non-alphanumeric data.id values.
        let data_id_signed = canonical_resource_id(&data_id);
        let x_request_id = super::webhook_util::header_str(&event.headers, "x-request-id")
            .ok_or_else(|| {
                BillingError::WebhookVerification("Missing x-request-id header".into())
            })?;
        let manifest = format!(
            "id:{};request-id:{};ts:{};",
            data_id_signed, x_request_id, ts
        );
        if !super::webhook_util::verify_hmac_sha256_hex(
            self.webhook_secret.expose_secret().as_bytes(),
            manifest.as_bytes(),
            v1,
        ) {
            return Err(BillingError::WebhookVerification(
                "Mercado Pago signature mismatch".into(),
            ));
        }

        let notification: serde_json::Value = serde_json::from_str(payload_str)
            .map_err(|e| BillingError::WebhookVerification(e.to_string()))?;
        let body_id = resource_id(&notification["data"]["id"]).ok_or_else(|| {
            BillingError::WebhookVerification(
                "Mercado Pago notification missing resource id".into(),
            )
        })?;
        if canonical_resource_id(&body_id) != data_id_signed {
            return Err(BillingError::WebhookVerification(
                "Mercado Pago notification resource id mismatch".into(),
            ));
        }
        let native_event = notification["type"].as_str().unwrap_or_default();
        let resource_path = match native_event {
            "payment" => Some("v1/payments"),
            "preapproval" | "subscription_preapproval" => Some("preapproval"),
            _ => None,
        };
        let Some(resource_path) = resource_path else {
            return Ok(ParsedWebhook {
                event_type: "mercado_pago.ignored".into(),
                customer_id: String::new(),
                subscription_id: None,
                payment_id: None,
                checkout_session_id: None,
                current_period_end: None,
                subscription_status: None,
                user_id: None,
                amount_cents: None,
                currency: None,
                data: serde_json::json!({"type": "mercado_pago.ignored"}),
            });
        };
        if data_id_signed.is_empty()
            || data_id_signed.len() > 128
            || !data_id_signed
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        {
            return Err(BillingError::WebhookVerification(
                "Invalid Mercado Pago resource id".into(),
            ));
        }
        // The signature covers the resource id, request id and timestamp, not the JSON body.
        let response = self
            .http_client
            .get(format!(
                "{}/{resource_path}/{data_id_signed}",
                self.base_url.trim_end_matches('/')
            ))
            .bearer_auth(self.access_token.expose_secret())
            .send()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?
            .error_for_status()
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;
        let resource: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BillingError::ProviderApi(e.to_string()))?;
        if resource_id(&resource["id"])
            .as_deref()
            .map(canonical_resource_id)
            .as_deref()
            != Some(data_id_signed.as_str())
        {
            return Err(BillingError::WebhookVerification(
                "Mercado Pago API resource id mismatch".into(),
            ));
        }
        let payment = native_event == "payment";
        let event_type = if payment {
            match resource["status"].as_str() {
                Some("approved") => super::provider::canonical::CHECKOUT_COMPLETED,
                Some("refunded") => "payment.refunded",
                Some("charged_back") => "payment.charged_back",
                _ => super::provider::canonical::PAYMENT_PENDING,
            }
        } else {
            super::provider::canonical::SUBSCRIPTION_UPDATED
        }
        .to_string();
        let subscription_id = if payment {
            resource_id(&resource["preapproval_id"])
        } else {
            resource_id(&resource["id"])
        };
        let checkout_session_id =
            resource_id(&resource["preference_id"]).or_else(|| subscription_id.clone());
        let payment_id = if payment {
            resource_id(&resource["id"])
        } else {
            None
        };
        let customer_id = resource_id(&resource["payer"]["id"])
            .or_else(|| resource_id(&resource["payer_id"]))
            .unwrap_or_default();
        let billing_details = if payment {
            &resource
        } else {
            &resource["auto_recurring"]
        };
        let data = serde_json::json!({"type": native_event, "data": resource});

        Ok(ParsedWebhook {
            event_type,
            customer_id,
            subscription_id,
            payment_id,
            checkout_session_id,
            current_period_end: super::provider::period_end_to_unix(
                data["data"].get("next_payment_date"),
            ),
            subscription_status: data["data"]["status"].as_str().map(String::from),
            user_id: data["data"]["external_reference"]
                .as_str()
                .and_then(|s| s.parse().ok()),
            amount_cents: billing_details["transaction_amount"]
                .as_f64()
                .map(|f| (f * 100.0) as i64),
            currency: billing_details["currency_id"].as_str().map(String::from),
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mercado_pago_provider_name() {
        let provider = MercadoPagoProvider::new("token".into(), "whsec".into());
        assert_eq!(provider.provider_name(), "mercado_pago");
    }

    #[test]
    fn test_mercado_pago_new() {
        let provider = MercadoPagoProvider::new("APP_USR-abc123".into(), "whsec_def".into());
        assert_eq!(provider.access_token.expose_secret(), "APP_USR-abc123");
        assert_eq!(provider.webhook_secret.expose_secret(), "whsec_def");
        assert_eq!(provider.base_url, "https://api.mercadopago.com");
    }

    #[test]
    fn test_mercado_pago_custom_base_url() {
        let provider = MercadoPagoProvider::new("token".into(), "wh".into())
            .with_base_url("http://localhost:9999".into());
        assert_eq!(provider.base_url, "http://localhost:9999");
    }

    use crate::services::billing::webhook_util;

    const TEST_DATA_ID: &str = "1234567890";
    const TEST_REQUEST_ID: &str = "abc-123";

    #[tokio::test]
    async fn webhook_uses_authoritative_resource_instead_of_unsigned_body_fields() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/payments/1234567890"))
            .and(header("Authorization", "Bearer token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1234567890_i64, "status": "pending", "payer": {"id": 42},
                "external_reference": "7", "transaction_amount": 1, "currency_id": "BRL"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let provider =
            MercadoPagoProvider::new("token".into(), "whsec".into()).with_base_url(server.uri());
        let event = signed_mp(
            br#"{"type":"payment","data":{"id":"1234567890","status":"approved","payer_id":"victim","external_reference":"999","preapproval_id":"victim_checkout","transaction_amount":9999}}"#,
            1_700_000_000, "whsec", TEST_DATA_ID,
        );
        let parsed = provider
            .verify_webhook_at(event, 1_700_000_000)
            .await
            .unwrap();
        assert_eq!(parsed.event_type, "payment.pending");
        assert_eq!(parsed.user_id, Some(7));
        assert_eq!(parsed.customer_id, "42");
        assert_eq!(parsed.amount_cents, Some(100));
        assert_eq!(parsed.checkout_session_id, None);
        assert_ne!(parsed.subscription_id.as_deref(), Some("victim_checkout"));
    }

    #[tokio::test]
    async fn webhook_rejects_unsigned_resource_id_substitution() {
        let provider = MercadoPagoProvider::new("token".into(), "whsec".into());
        let event = signed_mp(
            br#"{"type":"payment","data":{"id":"other_resource"}}"#,
            1_700_000_000,
            "whsec",
            TEST_DATA_ID,
        );
        assert!(matches!(
            provider.verify_webhook_at(event, 1_700_000_000).await,
            Err(BillingError::WebhookVerification(_))
        ));
    }

    #[tokio::test]
    async fn unknown_unsigned_topic_cannot_become_a_refund_event() {
        let provider = MercadoPagoProvider::new("token".into(), "whsec".into());
        let event = signed_mp(
            br#"{"type":"payment.refunded","data":{"id":"1234567890","external_reference":"999"}}"#,
            1_700_000_000,
            "whsec",
            TEST_DATA_ID,
        );
        let parsed = provider
            .verify_webhook_at(event, 1_700_000_000)
            .await
            .unwrap();
        assert_eq!(parsed.event_type, "mercado_pago.ignored");
        assert_eq!(parsed.user_id, None);
    }

    fn signed_mp(payload: &[u8], ts_raw: i64, secret: &str, data_id: &str) -> WebhookEvent {
        let ts_str = ts_raw.to_string();
        let signed_data_id = if data_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            data_id.to_ascii_lowercase()
        } else {
            data_id.to_string()
        };
        let manifest = format!("id:{signed_data_id};request-id:{TEST_REQUEST_ID};ts:{ts_str};");
        let v1 = webhook_util::hmac_sha256_hex(secret.as_bytes(), manifest.as_bytes());
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-signature",
            format!("ts={ts_str},v1={v1}").parse().unwrap(),
        );
        headers.insert("x-request-id", TEST_REQUEST_ID.parse().unwrap());
        WebhookEvent {
            provider: "mercado_pago".into(),
            payload: payload.to_vec(),
            headers,
            query: Some(format!("data.id={data_id}")),
        }
    }

    async fn mock_resource_provider(
        resource: serde_json::Value,
        kind: &str,
    ) -> (wiremock::MockServer, MercadoPagoProvider) {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        let id = canonical_resource_id(&resource_id(&resource["id"]).unwrap());
        Mock::given(method("GET"))
            .and(path(format!("/{kind}/{id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(resource))
            .mount(&server)
            .await;
        let provider =
            MercadoPagoProvider::new("token".into(), "whsec".into()).with_base_url(server.uri());
        (server, provider)
    }

    async fn mock_payment_provider(id: &str) -> (wiremock::MockServer, MercadoPagoProvider) {
        mock_resource_provider(
            serde_json::json!({"id": id, "status": "approved"}),
            "v1/payments",
        )
        .await
    }

    #[tokio::test]
    async fn verify_webhook_normalizes_authoritative_payment_status() {
        for (status, expected) in [
            ("approved", "checkout.session.completed"),
            ("pending", "payment.pending"),
            ("rejected", "payment.pending"),
            ("refunded", "payment.refunded"),
            ("charged_back", "payment.charged_back"),
        ] {
            let (_server, provider) = mock_resource_provider(
                serde_json::json!({
                    "id": TEST_DATA_ID, "status": status, "preapproval_id": "preap_1",
                }),
                "v1/payments",
            )
            .await;
            let event = signed_mp(
                br#"{"type":"payment","data":{"id":"1234567890"}}"#,
                1_700_000_000,
                "whsec",
                TEST_DATA_ID,
            );
            let parsed = provider
                .verify_webhook_at(event, 1_700_000_000)
                .await
                .unwrap();
            assert_eq!(parsed.event_type, expected, "status={status}");
            assert_eq!(parsed.subscription_id.as_deref(), Some("preap_1"));
        }
    }

    #[tokio::test]
    async fn verify_webhook_normalizes_authoritative_subscription() {
        let (_server, provider) = mock_resource_provider(
            serde_json::json!({
                "id": TEST_DATA_ID, "status": "authorized", "payer_id": 7,
                "next_payment_date": "2026-12-31T00:00:00Z", "external_reference": "42",
                "auto_recurring": {"transaction_amount": 99.9, "currency_id": "BRL"},
            }),
            "preapproval",
        )
        .await;
        let event = signed_mp(
            br#"{"type":"subscription_preapproval","data":{"id":"1234567890"}}"#,
            1_700_000_000,
            "whsec",
            TEST_DATA_ID,
        );
        let parsed = provider
            .verify_webhook_at(event, 1_700_000_000)
            .await
            .unwrap();
        assert_eq!(parsed.event_type, "customer.subscription.updated");
        assert_eq!(parsed.subscription_id.as_deref(), Some(TEST_DATA_ID));
        assert_eq!(parsed.subscription_status.as_deref(), Some("authorized"));
        assert_eq!(parsed.user_id, Some(42));
        assert_eq!(parsed.amount_cents, Some(9990));
        assert_eq!(parsed.currency.as_deref(), Some("BRL"));
    }

    #[tokio::test]
    async fn verify_webhook_uses_official_manifest() {
        let (_server, provider) = mock_payment_provider(TEST_DATA_ID).await;
        let now_ms: i64 = 1_700_000_000_000;
        let now_secs = now_ms / 1000;
        let body = br#"{"type":"payment","data":{"id":"1234567890"}}"#;

        let evt = signed_mp(body, now_ms, "whsec", TEST_DATA_ID);
        provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect("spec-correct manifest must verify");

        let ts_str = now_ms.to_string();
        let mut legacy = Vec::with_capacity(ts_str.len() + body.len());
        legacy.extend_from_slice(ts_str.as_bytes());
        legacy.extend_from_slice(body);
        let legacy_v1 = webhook_util::hmac_sha256_hex(b"whsec", &legacy);
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            "x-signature",
            format!("ts={ts_str},v1={legacy_v1}").parse().unwrap(),
        );
        h.insert("x-request-id", TEST_REQUEST_ID.parse().unwrap());
        let evt = WebhookEvent {
            provider: "mercado_pago".into(),
            payload: body.to_vec(),
            headers: h,
            query: Some(format!("data.id={TEST_DATA_ID}")),
        };
        let err = provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect_err("legacy manifest must be rejected");
        assert!(err.to_string().to_lowercase().contains("mismatch"));

        let mut evt = signed_mp(body, now_ms, "whsec", TEST_DATA_ID);
        evt.query = None;
        provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect_err("missing query must fail closed");

        let mut evt = signed_mp(body, now_ms, "whsec", TEST_DATA_ID);
        evt.headers.remove("x-request-id");
        provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect_err("missing x-request-id must fail closed");

        let mut evt = signed_mp(body, now_ms, "whsec", TEST_DATA_ID);
        evt.query = Some("data.id=0000000000".into());
        provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect_err("data.id mismatch must fail closed");
    }

    #[tokio::test]
    async fn verify_webhook_accepts_seconds_shaped_ts() {
        let (_server, provider) = mock_payment_provider(TEST_DATA_ID).await;
        const TS_SECS: i64 = 1_704_908_010;
        let now_secs = TS_SECS + 60;
        assert!(
            (now_secs - TS_SECS).abs() <= 300,
            "now must be within 300s of the doc ts for a deterministic freshness window"
        );
        let body = br#"{"type":"payment","data":{"id":"1234567890"}}"#;

        let evt = signed_mp(body, TS_SECS, "whsec", TEST_DATA_ID);
        provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect("seconds-shaped ts (doc value) must verify");

        let stale_now = TS_SECS + 300 + 1; // one second past the tolerance
        let evt = signed_mp(body, TS_SECS, "whsec", TEST_DATA_ID);
        let err = provider
            .verify_webhook_at(evt, stale_now)
            .await
            .expect_err("stale seconds-shaped ts must be rejected");
        assert!(
            err.to_string().to_lowercase().contains("outside tolerance"),
            "stale ts should be rejected as outside tolerance, got: {err}"
        );
    }

    #[tokio::test]
    async fn verify_webhook_lowercases_alphanumeric_data_id() {
        let (_server, provider) = mock_payment_provider("PAYIDABC123").await;
        let now_ms: i64 = 1_700_000_000_000;
        let now_secs = now_ms / 1000;
        const UPPER_DATA_ID: &str = "PAYIDABC123";
        let body = br#"{"type":"payment","data":{"id":"PAYIDABC123"}}"#;

        let evt = signed_mp(body, now_ms, "whsec", UPPER_DATA_ID);
        assert_eq!(evt.query.as_deref(), Some("data.id=PAYIDABC123"));
        provider
            .verify_webhook_at(evt, now_secs)
            .await
            .expect("verifier must lowercase the uppercase data.id query to match MP's lowercased signature");

        let mut evt2 = signed_mp(body, now_ms, "whsec", UPPER_DATA_ID);
        evt2.query = Some("data.id=ZZZ999".into());
        provider
            .verify_webhook_at(evt2, now_secs)
            .await
            .expect_err("an unrelated data.id must fail closed");
    }

    #[test]
    fn ts_unit_to_secs_disambiguates_by_magnitude() {
        assert_eq!(ts_unit_to_secs(1_704_908_010), 1_704_908_010);
        assert_eq!(ts_unit_to_secs(1_704_908_010_000), 1_704_908_010);
        assert_eq!(ts_unit_to_secs(10_000_000_000), 10_000_000_000);
        assert_eq!(ts_unit_to_secs(10_000_000_001), 10_000_000);
        assert_eq!(ts_unit_to_secs(0), 0);
        assert_eq!(ts_unit_to_secs(-1), -1);
    }

    #[test]
    fn extract_query_param_finds_data_id() {
        assert_eq!(
            extract_query_param("data.id=1234567890&type=payment", "data.id").as_deref(),
            Some("1234567890")
        );
        assert_eq!(
            extract_query_param("data.id=abc", "data.id").as_deref(),
            Some("abc")
        );
        assert_eq!(
            extract_query_param("data.id=foo%20bar%2Bbaz&type=payment", "data.id").as_deref(),
            Some("foo bar+baz")
        );
        assert!(extract_query_param("type=payment", "data.id").is_none());
    }
}
