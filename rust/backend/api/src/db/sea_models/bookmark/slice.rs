use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

/// Response item for the bookmark sync snapshot (camelCase wire format).
/// Range ends are `null` on the wire unless they differ from the start
/// (stored `end = start` ⇄ wire `null` end).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkListItem {
    pub id: String,
    pub folder_id: String,
    pub source_id: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    pub end_surah: Option<i32>,
    pub end_ayah: Option<i32>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl From<super::Model> for BookmarkListItem {
    fn from(m: super::Model) -> Self {
        let is_range = m.end_surah != m.surah || m.end_ayah != m.ayah;
        Self {
            id: m.id,
            folder_id: m.folder_id,
            source_id: m.source_id,
            surah: m.surah,
            ayah: m.ayah,
            end_surah: is_range.then_some(m.end_surah),
            end_ayah: is_range.then_some(m.end_ayah),
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}
