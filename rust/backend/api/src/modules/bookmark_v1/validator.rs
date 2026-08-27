use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// The four offline-sync mutation kinds. Dotted wire names ("bookmark.upsert")
/// carry per-variant `rename`s — serde's rename_all can't express them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationKind {
    #[serde(rename = "bookmark.upsert")]
    BookmarkUpsert,
    #[serde(rename = "bookmark.delete")]
    BookmarkDelete,
    #[serde(rename = "folder.upsert")]
    FolderUpsert,
    #[serde(rename = "folder.delete")]
    FolderDelete,
}

/// Client-minted ids are UUIDv4: 8-4-4-4-12 hex groups, 36 chars total.
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

/// Flat wire struct for one queued mutation: `kind` picks the semantics, the
/// kind-specific fields are required (cross-checked in the controller — the
/// derive cannot express per-variant requiredness on a flat struct).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncMutation {
    pub kind: MutationKind,
    #[validate(custom(function = valid_client_uuid))]
    pub id: String,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(custom(function = valid_client_uuid))]
    pub folder_id: Option<String>,
    #[validate(range(min = 1, max = 114))]
    pub surah: Option<i32>,
    #[validate(range(min = 1, max = 286))]
    pub ayah: Option<i32>,
    /// RFC3339, enforced by chrono's serde deserializer.
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1BookmarkSyncPayload {
    /// FIFO queue from the offline client; applied in order. Absent = pure pull.
    /// `nested` runs each item's own field validations (range/length/uuid);
    /// without it a Vec's items are never validated.
    #[validate(length(max = 200), nested)]
    pub mutations: Option<Vec<SyncMutation>>,
}
