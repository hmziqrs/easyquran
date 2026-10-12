use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Closed event set for the first-party reading pipeline (Feature 6.4 emit
/// points): `ayah.read` = a verse finished reading (carries `seconds`),
/// `ayah.open` = a verse opened (no duration). The wire literals are the
/// stored `kind` values — never free-form client strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    #[serde(rename = "ayah.read")]
    AyahRead,
    #[serde(rename = "ayah.open")]
    AyahOpen,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::AyahRead => "ayah.read",
            EventKind::AyahOpen => "ayah.open",
        }
    }
}

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

/// One reading event. `ayah` carries a FLAT bound only — the per-surah
/// ayah-count check runs in the controller against the Quran meta (the flat
/// 1..=286 alone lets 1:8 through, Feature 6.3). `at` is RFC3339 (enforced by
/// chrono's serde deserializer) and clamped server-side.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ReadingEventItem {
    pub kind: EventKind,
    #[validate(range(min = 1, max = 114))]
    pub surah: i32,
    #[validate(range(min = 1, max = 286))]
    pub ayah: i32,
    #[validate(custom(function = valid_source_id))]
    pub source_id: Option<String>,
    /// Dwell time in seconds; only meaningful on `ayah.read`. Capped at one
    /// hour per event — a stuck tab must not write day-long durations.
    #[validate(range(min = 0, max = 3600))]
    pub seconds: Option<i32>,
    #[validate(custom(function = valid_local_day))]
    pub local_day: String,
    pub at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct V1EventsPayload {
    /// REQUIRED (same contract as reading_session_v1, §9.2): no legacy
    /// clients exist for this module, so an account mismatch is always a
    /// stale-tab/account-switch, rejected before any write.
    pub expected_user_id: i32,
    /// Batched + capped; each batch applies wholly or not at all.
    #[validate(length(max = 100), nested)]
    pub events: Option<Vec<ReadingEventItem>>,
}
