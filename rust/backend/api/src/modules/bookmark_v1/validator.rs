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

/// Session ids are client-minted too (UUIDv4 or the UUIDv5 account default —
/// both pass the dashed-hex shape, §9.1). The '' global sentinel is allowed
/// so a legacy-shaped push is not rejected outright.
fn valid_session_id(id: &str) -> Result<(), ValidationError> {
    if id.is_empty() || valid_client_uuid(id).is_ok() {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_session_id")
            .with_message("sessionId must be a dashed hex UUID or empty".into()))
    }
}

/// Folder kind is a name-only label (§11.7): a closed literal set, no goal
/// metadata — anything else is rejected, not defaulted.
fn valid_folder_kind(kind: &str) -> Result<(), ValidationError> {
    if matches!(kind, "generic" | "daily" | "learning") {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_folder_kind")
            .with_message("kind must be generic, daily, or learning".into()))
    }
}

/// `source_id` is a translation catalogue id: ≤ 64 chars of `[a-z0-9._-]`
/// (§9.1 validation caps — bounds the row, never the Quran data).
fn valid_source_id(id: &str) -> Result<(), ValidationError> {
    let charset = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-');
    if id.len() <= 64 && id.chars().all(charset) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_source_id")
            .with_message("sourceId must be ≤64 chars of [a-z0-9._-]".into()))
    }
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
    /// Folder study-set label. Wire name is `folderKind` — `kind` is already
    /// the mutation-kind discriminator on this flat struct; the SNAPSHOT item
    /// carries it as plain `kind` (there is no collision there).
    #[validate(custom(function = valid_folder_kind))]
    pub folder_kind: Option<String>,
    pub position: Option<i32>,
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
    /// '' / absent = global (the sentinel, never a bare NULL in the key).
    /// A scoped row names its session by the client-minted UUID (§9.1).
    #[validate(custom(function = valid_session_id))]
    pub session_id: Option<String>,
    #[validate(custom(function = valid_source_id))]
    pub source_id: Option<String>,
    /// RFC3339, enforced by chrono's serde deserializer.
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1BookmarkSyncPayload {
    pub expected_user_id: Option<i32>,
    #[validate(length(max = 200), nested)]
    pub mutations: Option<Vec<SyncMutation>>,
}
