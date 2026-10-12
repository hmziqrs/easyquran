use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// The four offline-sync mutation kinds (§9.2). Dotted wire names carry
/// per-variant `rename`s — serde's rename_all can't express them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationKind {
    #[serde(rename = "session.upsert")]
    SessionUpsert,
    #[serde(rename = "session.delete")]
    SessionDelete,
    #[serde(rename = "position.upsert")]
    PositionUpsert,
    #[serde(rename = "progress.upsert")]
    ProgressUpsert,
}

/// Session ids are client-minted UUIDv4 (or the UUIDv5 account default —
/// both pass the dashed-hex shape, §9.1). Unlike bookmarks there is no ''
/// sentinel here: every mutation names a real session.
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

/// Closed literal set (§9.1): "shared" is a seam kind with no server-side
/// sharing semantics; the sharedWith list itself never reaches the wire.
fn valid_session_kind(kind: &str) -> Result<(), ValidationError> {
    if matches!(kind, "daily" | "weekly" | "free" | "shared") {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_session_kind")
            .with_message("sessionKind must be daily, weekly, free, or shared".into()))
    }
}

/// Catalogue-id charset bound (§9.1): ≤ 64 chars of `[a-z0-9._-]`. Bounds the
/// row, never the Quran data.
fn valid_source_id(id: &str) -> Result<(), ValidationError> {
    let charset = |c: char| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
    };
    if id.len() <= 64 && id.chars().all(charset) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_source_id")
            .with_message("sourceId must be ≤64 chars of [a-z0-9._-]".into()))
    }
}

fn valid_ratio(ratio: f64) -> Result<(), ValidationError> {
    if (0.0..=1.0).contains(&ratio) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_ratio")
            .with_message("ratio must be within 0..=1".into()))
    }
}

/// Flat wire struct for one queued mutation: `kind` picks the semantics; the
/// kind-specific fields are required (cross-checked in the controller — the
/// derive cannot express per-variant requiredness on a flat struct). The
/// position's `verseKey` is NOT on the wire: the server derives it from
/// surah/ayah so the two can never disagree.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncMutation {
    pub kind: MutationKind,
    /// Session id for session.upsert / session.delete.
    #[validate(custom(function = valid_client_uuid))]
    pub id: Option<String>,
    /// Session id for position.upsert / progress.upsert.
    #[validate(custom(function = valid_client_uuid))]
    pub session_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    /// Session kind; wire name `sessionKind` — `kind` is already the
    /// mutation-kind discriminator on this flat struct.
    #[validate(custom(function = valid_session_kind))]
    pub session_kind: Option<String>,
    #[validate(custom(function = valid_source_id))]
    pub translation_id: Option<String>,
    /// RFC3339, enforced by chrono's serde deserializer.
    pub archived_at: Option<DateTime<FixedOffset>>,
    #[validate(range(min = 1, max = 114))]
    pub surah: Option<i32>,
    /// Flat bound only; the per-surah ayah-count check runs in the controller
    /// against the Quran meta (the flat 1..=286 alone lets 1:8 through).
    #[validate(range(min = 1, max = 286))]
    pub ayah: Option<i32>,
    #[validate(range(min = 1, max = 286))]
    pub furthest_ayah: Option<i32>,
    #[validate(range(min = 1))]
    pub local_page: Option<i32>,
    #[validate(custom(function = valid_ratio))]
    pub ratio: Option<f64>,
    #[validate(custom(function = valid_source_id))]
    pub source_id: Option<String>,
    /// RFC3339, enforced by chrono's serde deserializer.
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1ReadingSessionSyncPayload {
    /// REQUIRED for this module (§9.2): no pre-R2-08 legacy clients exist for
    /// reading-session sync, so a mismatch is always rejected outright.
    pub expected_user_id: i32,
    #[validate(length(max = 200), nested)]
    pub mutations: Option<Vec<SyncMutation>>,
}
