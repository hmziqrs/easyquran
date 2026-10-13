use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Offline-sync mutation kinds. Dotted wire names carry per-variant
/// `rename`s — serde's rename_all can't express them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationKind {
    #[serde(rename = "session.upsert")]
    SessionUpsert,
    #[serde(rename = "session.delete")]
    SessionDelete,
    #[serde(rename = "progress.upsert")]
    ProgressUpsert,
}

/// Client-minted ids: dashed-hex UUID, 36 chars.
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

fn one_of(value: &str, allowed: &[&str], code: &'static str) -> Result<(), ValidationError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(code)
            .with_message(format!("must be one of: {}", allowed.join(", ")).into()))
    }
}

pub const ORIGINS: &[&str] = &["index", "search", "bookmark", "card", "snippet", "link"];
pub const DEVICES: &[&str] = &["phone", "tablet", "desktop"];
pub const END_REASONS: &[&str] = &["completed", "restarted", "dismissed"];

fn valid_origin(v: &str) -> Result<(), ValidationError> {
    one_of(v, ORIGINS, "invalid_origin")
}

fn valid_device(v: &str) -> Result<(), ValidationError> {
    one_of(v, DEVICES, "invalid_device")
}

fn valid_end_reason(v: &str) -> Result<(), ValidationError> {
    one_of(v, END_REASONS, "invalid_end_reason")
}

/// Bounded id charset (catalogue ids, card ids, snippet slugs): ≤ 64 chars of
/// `[A-Za-z0-9._-]`. Bounds the row, never the Quran data.
fn valid_ref(id: &str) -> Result<(), ValidationError> {
    let charset = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    if !id.is_empty() && id.len() <= 64 && id.chars().all(charset) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_ref")
            .with_message("must be 1–64 chars of [A-Za-z0-9._-]".into()))
    }
}

/// `S:A` verse key shape; the controller checks it is a real verse.
pub fn parse_verse_key(key: &str) -> Option<(i32, i32)> {
    let (s, a) = key.split_once(':')?;
    let digits = |p: &str| !p.is_empty() && p.len() <= 3 && p.chars().all(|c| c.is_ascii_digit());
    if !digits(s) || !digits(a) {
        return None;
    }
    Some((s.parse().ok()?, a.parse().ok()?))
}

fn valid_verse_key(key: &str) -> Result<(), ValidationError> {
    parse_verse_key(key)
        .map(|_| ())
        .ok_or_else(|| ValidationError::new("invalid_verse_key").with_message("must be S:A".into()))
}

/// Scope shapes (docs/reading-model.md §1): `surah|page|juz|hizb:N`,
/// `snippet:slug`, `range:S:A-S:A`. Only the SHAPE is checked here; numeric
/// bounds of verse ranges run in the controller against the Quran meta.
pub fn valid_scope(scope: &str) -> Result<(), ValidationError> {
    let invalid =
        || Err(ValidationError::new("invalid_scope").with_message("unknown scope shape".into()));
    let Some((kind, rest)) = scope.split_once(':') else {
        return invalid();
    };
    let number = |p: &str, max: u32| p.parse::<u32>().is_ok_and(|n| (1..=max).contains(&n));
    let ok = match kind {
        "surah" => number(rest, 114),
        "page" => number(rest, 604),
        "juz" => number(rest, 30),
        "hizb" => number(rest, 60),
        "snippet" => valid_ref(rest).is_ok(),
        "range" => rest
            .split_once('-')
            .is_some_and(|(a, b)| parse_verse_key(a).is_some() && parse_verse_key(b).is_some()),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        invalid()
    }
}

/// Flat wire struct for one queued mutation: `kind` picks the semantics; the
/// kind-specific fields are required (cross-checked in the controller — the
/// derive cannot express per-variant requiredness on a flat struct).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncMutation {
    pub kind: MutationKind,
    /// Thread id for session.upsert / session.delete.
    #[validate(custom(function = valid_client_uuid))]
    pub id: Option<String>,
    #[validate(custom(function = valid_origin))]
    pub origin: Option<String>,
    #[validate(custom(function = valid_ref))]
    pub origin_ref: Option<String>,
    #[validate(custom(function = valid_scope))]
    pub scope: Option<String>,
    #[validate(custom(function = valid_verse_key))]
    pub bound_end: Option<String>,
    #[validate(range(min = 1, max = 114))]
    pub surah: Option<i32>,
    #[validate(range(min = 1, max = 286))]
    pub ayah: Option<i32>,
    #[validate(range(min = 1, max = 10_000))]
    pub local_page: Option<i32>,
    #[validate(range(min = 0.0, max = 1.0))]
    pub ratio: Option<f64>,
    #[validate(custom(function = valid_ref))]
    pub source_id: Option<String>,
    #[validate(custom(function = valid_device))]
    pub device: Option<String>,
    pub started_at: Option<DateTime<FixedOffset>>,
    pub ended_at: Option<DateTime<FixedOffset>>,
    #[validate(custom(function = valid_end_reason))]
    pub end_reason: Option<String>,
    /// progress.upsert only.
    #[validate(range(min = 1, max = 286))]
    pub furthest_ayah: Option<i32>,
    /// RFC3339, enforced by chrono's serde deserializer.
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1ReadingSessionSyncPayload {
    /// REQUIRED: a mismatch is always a stale tab / account switch, rejected
    /// before any write.
    pub expected_user_id: i32,
    #[validate(length(max = 200), nested)]
    pub mutations: Option<Vec<SyncMutation>>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1ArchiveQuery {
    /// Opaque cursor from the previous page's `next` (`updatedAt|id`).
    #[validate(length(max = 80))]
    pub before: Option<String>,
    #[validate(range(min = 1, max = 50))]
    pub limit: Option<u64>,
}
