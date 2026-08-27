use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

/// Response item for the bookmark sync snapshot (camelCase wire format).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkListItem {
    pub id: String,
    pub folder_id: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl From<super::Model> for BookmarkListItem {
    fn from(m: super::Model) -> Self {
        Self {
            id: m.id,
            folder_id: m.folder_id,
            surah: m.surah,
            ayah: m.ayah,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}
