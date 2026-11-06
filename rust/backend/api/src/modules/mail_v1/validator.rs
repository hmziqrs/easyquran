use serde::Deserialize;

use ruxlog_types::enums::SuppressionReason;

#[derive(Debug, Deserialize, Default)]
pub struct V1ListSuppressionsQuery {
    #[serde(default)]
    pub page: Option<u64>,
    pub reason: Option<SuppressionReason>,
    pub permanent: Option<bool>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct V1DeleteSuppression {
    pub recipient: String,
}

/// Manual suppression (A29): the reason is required, so every row says why it exists.
#[derive(Debug, Deserialize)]
pub struct V1CreateSuppression {
    pub recipient: String,
    pub reason: SuppressionReason,
    #[serde(default)]
    pub permanent: bool,
    pub diagnostic: Option<String>,
}

/// Admin review of a webhook-sourced suppression (A29).
#[derive(Debug, Deserialize)]
pub struct V1ReviewSuppression {
    pub recipient: String,
}
