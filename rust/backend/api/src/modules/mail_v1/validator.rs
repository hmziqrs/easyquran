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

#[derive(Debug, Deserialize)]
pub struct V1CreateSuppression {
    pub recipient: String,
    #[serde(default)]
    pub reason: Option<SuppressionReason>,
    #[serde(default)]
    pub permanent: bool,
    pub diagnostic: Option<String>,
}

impl V1CreateSuppression {
    pub fn reason_or_default(&self) -> SuppressionReason {
        self.reason.unwrap_or(SuppressionReason::Manual)
    }
}
