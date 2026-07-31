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
    pub webpush: Option<serde_json::Value>,
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

        // Map 404/UNREGISTERED to the `Unregistered` sentinel — the fan-out
        // loop prunes the device row on this variant. Do not collapse.
        if status == 404 || body.to_uppercase().contains("UNREGISTERED") {
            return Err(FcmError::Unregistered);
        }

        Err(FcmError::Api(status, body))
    }
}
