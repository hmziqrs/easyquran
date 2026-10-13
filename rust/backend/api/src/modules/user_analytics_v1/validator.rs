use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Client-supplied device-local day (EC-36): strict `YYYY-MM-DD`, parsed not
/// pattern-matched so `2026-13-01` dies here. The server never derives day
/// boundaries from its own clock or timezone.
fn valid_local_day(day: &str) -> Result<(), ValidationError> {
    if NaiveDate::parse_from_str(day, "%Y-%m-%d").is_ok() {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_local_day")
            .with_message("localDay must be a valid YYYY-MM-DD date".into()))
    }
}

/// Catalogue-id charset bound: ≤ 64 chars of `[a-z0-9._-]`. Bounds the
/// row, never the Quran data.
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

/// Client-minted ids: dashed-hex UUID, 36 chars.
fn valid_client_uuid(id: &str) -> Result<(), ValidationError> {
    let ok = id.len() == 36
        && id.chars().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        });
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_uuid").with_message("must be a dashed hex UUID".into()))
    }
}

/// One verse read (docs/reading-model.md §4). `id` is client-minted so a
/// retried batch is idempotent. `ayah` carries a FLAT bound only — the
/// per-surah ayah-count check runs in the controller against the Quran meta.
/// `at` is RFC3339 (enforced by chrono's serde deserializer) and clamped
/// server-side.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ReadingEventItem {
    #[validate(custom(function = valid_client_uuid))]
    pub id: String,
    /// The reading thread the verse was read in, when known.
    #[validate(custom(function = valid_client_uuid))]
    pub session_id: Option<String>,
    #[validate(range(min = 1, max = 114))]
    pub surah: i32,
    #[validate(range(min = 1, max = 286))]
    pub ayah: i32,
    #[validate(custom(function = valid_source_id))]
    pub source_id: Option<String>,
    /// Active reading time attributed to this verse. Capped at one hour — a
    /// stuck tab must not write day-long durations.
    #[validate(range(min = 0, max = 3_600_000))]
    pub ms: i64,
    #[validate(custom(function = valid_local_day))]
    pub local_day: String,
    pub at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1EventsPayload {
    /// REQUIRED: an account mismatch is always a stale tab / account switch,
    /// rejected before any write.
    pub expected_user_id: i32,
    /// Batched + capped; each batch applies wholly or not at all.
    #[validate(length(max = 100), nested)]
    pub events: Option<Vec<ReadingEventItem>>,
}
