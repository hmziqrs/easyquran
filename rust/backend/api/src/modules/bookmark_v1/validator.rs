use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Row kinds. Dotted wire names ("bookmark.upsert")
/// carry per-variant `rename`s — serde's rename_all can't express them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationKind {
    #[serde(rename = "bookmark.upsert")]
    BookmarkUpsert,
    #[serde(rename = "folder.upsert")]
    FolderUpsert,
}

/// Client-minted ids are UUIDs (v7, or the v5 default folder): 8-4-4-4-12
/// hex groups, 36 chars total.
fn valid_client_uuid(id: &str) -> Result<(), ValidationError> {
    fn invalid(detail: &'static str) -> ValidationError {
        ValidationError::new("invalid_uuid").with_message(detail.into())
    }
    if id.len() != 36 {
        return Err(invalid("id must be 36 characters"));
    }
    for (i, c) in id.chars().enumerate() {
        let ok = match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        };
        if !ok {
            return Err(invalid("id must be a dashed hex UUID"));
        }
    }
    Ok(())
}

/// `source_id` is a translation catalogue id: ≤ 64 chars of `[a-z0-9._-]`
/// (bounds the row, never the Quran data).
fn valid_source_id(id: &str) -> Result<(), ValidationError> {
    let charset =
        |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-');
    if id.len() <= 64 && id.chars().all(charset) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_source_id")
            .with_message("sourceId must be ≤64 chars of [a-z0-9._-]".into()))
    }
}

fn valid_stamp(stamp: &str) -> Result<(), ValidationError> {
    if eq_sync_core::is_valid_stamp(stamp) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_stamp").with_message("stamp fails the format".into()))
    }
}

/// One row to write (contracts/sync.md). A bookmark is one whole-row register. A
/// folder has three, each with its stamp; an absent one is no change, never a clear.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncMutation {
    pub kind: MutationKind,
    #[validate(custom(function = valid_client_uuid))]
    pub id: String,
    #[validate(custom(function = valid_stamp))]
    pub name_stamp: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(custom(function = valid_stamp))]
    pub position_stamp: Option<String>,
    pub position: Option<i32>,
    #[validate(custom(function = valid_stamp))]
    pub deleted_stamp: Option<String>,
    /// Whole-row register stamp for bookmark.upsert.
    #[validate(custom(function = valid_stamp))]
    pub stamp: Option<String>,
    pub deleted: Option<bool>,
    /// Target folder for bookmark.upsert. Absent, unknown or foreign ids land in
    /// the account's default folder.
    #[validate(custom(function = valid_client_uuid))]
    pub folder_id: Option<String>,
    #[validate(range(min = 1, max = 114))]
    pub surah: Option<i32>,
    #[validate(range(min = 1, max = 286))]
    pub ayah: Option<i32>,
    /// Range end (Feature 7). Absent/null = single ayah (stored end = start).
    /// Cross-field checks — end ≥ start lexicographically, per-surah ayah
    /// counts, span cap 50 — run in the controller against the Quran meta
    /// (EC-39); the derive can only bound each field flatly.
    #[validate(range(min = 1, max = 114))]
    pub end_surah: Option<i32>,
    #[validate(range(min = 1, max = 286))]
    pub end_ayah: Option<i32>,
    #[validate(custom(function = valid_source_id))]
    pub source_id: Option<String>,
    /// Display only. Stamps decide conflicts.
    pub updated_at: Option<DateTime<FixedOffset>>,
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
pub struct V1BookmarkSyncPayload {
    /// REQUIRED: a mismatch is always a stale tab / account switch, rejected
    /// before any write.
    pub expected_user_id: i32,
    /// Rows parse one at a time, so one bad row never refuses the others.
    #[validate(length(max = 200))]
    pub mutations: Option<Vec<serde_json::Value>>,
    /// Last `rev` this client applied. 0 is a full pull.
    #[validate(range(min = 0))]
    pub since: Option<i64>,
    /// Page cursor inside a full pull.
    #[validate(range(min = -1))]
    pub after: Option<i64>,
    #[validate(nested)]
    pub client: Option<ClientInfo>,
}
