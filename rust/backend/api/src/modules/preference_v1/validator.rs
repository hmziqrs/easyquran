use serde::{Deserialize, Serialize};
use validator::Validate;

/// Longest stored value. Settings are small; the home layout is the largest.
pub const VALUE_MAX_BYTES: usize = 16 * 1024;
const KEY_MAX: usize = 64;

/// Synced keys: `settings.*`, the home layout, the reading bookmark. Anything else
/// is refused, so this route can never become general storage.
pub fn valid_key(key: &str) -> bool {
    if key.is_empty() || key.len() > KEY_MAX {
        return false;
    }
    if key == "home.layout" || key == "reading.bookmark" {
        return true;
    }
    let Some(rest) = key.strip_prefix("settings.") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub protocol: u32,
    #[validate(length(max = 16))]
    pub platform: Option<String>,
    #[validate(length(max = 32))]
    pub app_version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1PreferenceSyncPayload {
    /// REQUIRED: a mismatch is always a stale tab / account switch, rejected
    /// before any write.
    pub expected_user_id: i32,
    /// Rows parse one at a time, so one bad row never refuses the others.
    #[validate(length(max = 200))]
    pub mutations: Option<Vec<serde_json::Value>>,
    /// Last `rev` this client applied. 0 is a full pull.
    #[validate(range(min = 0))]
    pub since: Option<i64>,
    #[validate(nested)]
    pub client: Option<ClientInfo>,
}
