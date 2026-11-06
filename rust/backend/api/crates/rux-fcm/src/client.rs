use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::auth::ServiceAccount;
use crate::error::FcmError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FcmMessage {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<Notification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apns: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webpush: Option<serde_json::Value>,
}

/// APNs block for an iOS alert: `alert` push type and priority 10 so the banner
/// shows at once, with the default sound. FCM relays it to APNs unchanged.
pub fn apns_alert() -> serde_json::Value {
    serde_json::json!({
        "headers": { "apns-push-type": "alert", "apns-priority": "10" },
        "payload": { "aps": { "sound": "default" } }
    })
}

pub struct FcmClient {
    pub http: reqwest::Client,
    pub project_id: String,
    pub sa: Arc<ServiceAccount>,
}

impl FcmClient {
    pub fn new(sa: ServiceAccount, project_id: String, http: reqwest::Client) -> Self {
        Self {
            http,
            project_id,
            sa: Arc::new(sa),
        }
    }

    pub async fn send(&self, msg: FcmMessage) -> Result<String, FcmError> {
        let access = self.sa.mint_access_token(&self.http).await?;
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.project_id
        );

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&access.token)
            .json(&serde_json::json!({ "message": msg }))
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();

        if status == 200 || status == 201 {
            let parsed: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| FcmError::Api(status, format!("parse response: {e}")))?;
            return parsed
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| FcmError::Api(status, "missing 'name' in response".to_string()));
        }

        Err(classify_failure(status, body))
    }
}

/// Map a non-success FCM reply to its error class. Order matters: `UNREGISTERED`
/// is checked first and keeps its own sentinel (pruned at once). `SENDER_ID_MISMATCH`
/// and `INVALID_ARGUMENT` get their own classes, so callers can count strikes.
pub(crate) fn classify_failure(status: u16, body: String) -> FcmError {
    let upper = body.to_uppercase();
    if status == 404 || upper.contains("UNREGISTERED") {
        return FcmError::Unregistered;
    }
    if upper.contains("SENDER_ID_MISMATCH") {
        return FcmError::SenderMismatch;
    }
    if status == 400 || upper.contains("INVALID_ARGUMENT") {
        return FcmError::InvalidArgument;
    }
    FcmError::Api(status, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unregistered_prunes_at_once_on_404_or_body() {
        assert!(matches!(
            classify_failure(404, String::new()),
            FcmError::Unregistered
        ));
        let body = r#"{"error":{"status":"NOT_FOUND","details":[{"errorCode":"UNREGISTERED"}]}}"#;
        assert!(matches!(
            classify_failure(404, body.into()),
            FcmError::Unregistered
        ));
    }

    #[test]
    fn invalid_argument_and_sender_mismatch_get_their_own_classes() {
        let bad = r#"{"error":{"code":400,"status":"INVALID_ARGUMENT"}}"#;
        assert!(matches!(
            classify_failure(400, bad.into()),
            FcmError::InvalidArgument
        ));
        let mismatch = r#"{"error":{"code":403,"status":"PERMISSION_DENIED","details":[{"errorCode":"SENDER_ID_MISMATCH"}]}}"#;
        assert!(matches!(
            classify_failure(403, mismatch.into()),
            FcmError::SenderMismatch
        ));
    }

    #[test]
    fn other_failures_stay_api_errors() {
        assert!(matches!(
            classify_failure(500, "boom".into()),
            FcmError::Api(500, _)
        ));
        assert!(matches!(
            classify_failure(429, "quota".into()),
            FcmError::Api(429, _)
        ));
    }
}
