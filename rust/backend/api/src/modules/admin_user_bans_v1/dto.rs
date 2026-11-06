use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// Closed reason codes for an account ban (A19). Adding a code is a contract change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    Spam,
    Harassment,
    CredentialAbuse,
    PolicyViolation,
    Other,
}

impl ReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Harassment => "harassment",
            Self::CredentialAbuse => "credential_abuse",
            Self::PolicyViolation => "policy_violation",
            Self::Other => "other",
        }
    }
}

/// Required corroborator on every ban decision (A19). `none` is an explicit
/// "no corroborator found", recorded so the decision is auditable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corroborator {
    CredentialReuse,
    ContentMatch,
    Ip,
    #[serde(rename = "none")]
    NoCorroborator,
}

impl Corroborator {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CredentialReuse => "credential_reuse",
            Self::ContentMatch => "content_match",
            Self::Ip => "ip",
            Self::NoCorroborator => "none",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBanPayload {
    pub user_id: i32,
    pub reason_code: ReasonCode,
    #[serde(default)]
    pub reason_text: Option<String>,
    pub corroborator: Corroborator,
    pub expires_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenewBanPayload {
    pub expires_at: DateTime<FixedOffset>,
    /// Must be `true`. Records that one operator made the call and reviewed it.
    pub single_operator_control: bool,
    pub self_review_note: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BanCreated {
    pub ban_id: i32,
    pub notice_id: i64,
    pub user_id: i32,
    pub expires_at: i64,
    /// Provider-confirmed notice outcome; `null` when the record write failed (unknown).
    pub delivery_state: Option<String>,
}
