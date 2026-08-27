use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

/// Response item for the bookmark sync snapshot (camelCase wire format).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderListItem {
    pub id: String,
    pub name: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl From<super::Model> for FolderListItem {
    fn from(m: super::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}
