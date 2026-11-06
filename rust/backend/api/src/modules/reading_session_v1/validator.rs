use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Offline-sync mutation kinds. Dotted wire names carry per-variant
/// `rename`s — serde's rename_all can't express them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationKind {
    #[serde(rename = "session.upsert")]
    SessionUpsert,
    #[serde(rename = "progress.upsert")]
    ProgressUpsert,
}

/// Thread ids: dashed-hex UUID v7, variant `10xx`. The embedded timestamp comes
/// from the device clock and is never checked: a wrong clock must not lose
/// reading data.
fn valid_thread_id(id: &str) -> Result<(), ValidationError> {
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
    let bytes = id.as_bytes();
    if bytes[14] != b'7' {
        return Err(invalid("id must be a UUID v7"));
    }
    if !matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B') {
        return Err(invalid("id must have the RFC 9562 variant"));
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
pub const STATES: &[&str] = &["open", "ended", "deleted"];

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

/// Scope shapes: `surah|page|juz|hizb:N`,
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

fn valid_stamp(stamp: &str) -> Result<(), ValidationError> {
    if eq_sync_core::is_valid_stamp(stamp) {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_stamp").with_message("stamp fails the format".into()))
    }
}

fn valid_state(v: &str) -> Result<(), ValidationError> {
    one_of(v, STATES, "invalid_state")
}

/// One row to write. Field groups travel with their stamp: a group whose stamp is
/// absent is no change, never a clear, so an older client cannot erase what it
/// does not know about (contracts/sync.md).
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SyncMutation {
    pub kind: MutationKind,
    /// Thread id for session.upsert.
    #[validate(custom(function = valid_thread_id))]
    pub id: Option<String>,
    #[validate(custom(function = valid_stamp))]
    pub identity_stamp: Option<String>,
    #[validate(custom(function = valid_origin))]
    pub origin: Option<String>,
    #[validate(custom(function = valid_ref))]
    pub origin_ref: Option<String>,
    #[validate(custom(function = valid_scope))]
    pub scope: Option<String>,
    #[validate(custom(function = valid_verse_key))]
    pub bound_end: Option<String>,
    pub started_at: Option<DateTime<FixedOffset>>,
    #[validate(custom(function = valid_stamp))]
    pub cursor_stamp: Option<String>,
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
    #[validate(custom(function = valid_ref))]
    pub device_id: Option<String>,
    #[validate(custom(function = valid_stamp))]
    pub lifecycle_stamp: Option<String>,
    #[validate(custom(function = valid_state))]
    pub state: Option<String>,
    pub ended_at: Option<DateTime<FixedOffset>>,
    #[validate(custom(function = valid_end_reason))]
    pub end_reason: Option<String>,
    /// progress.upsert only.
    #[validate(range(min = 1, max = 286))]
    pub furthest_ayah: Option<i32>,
    /// Display only. Stamps decide conflicts.
    pub updated_at: Option<DateTime<FixedOffset>>,
}

/// The sender's protocol. Sent in the body so the CORS table never changes.
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
pub struct V1ReadingSessionSyncPayload {
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

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1ArchiveQuery {
    /// Opaque cursor from the previous page's `next` (`cursorStamp|id`).
    #[validate(length(max = 80))]
    pub before: Option<String>,
    #[validate(range(min = 1, max = 50))]
    pub limit: Option<u64>,
}
