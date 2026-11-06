use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Evidence table (A24). No FK to users and no IP or device column: an account
/// delete keeps the row, and intake records nothing that identifies a device.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub reporter_user_id: Option<i32>,
    pub target_kind: String,
    pub target_id: String,
    pub reason: String,
    pub priority: String,
    pub details: Option<String>,
    pub contact_email: Option<String>,
    pub snapshot: Option<Json>,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub expires_at: i64,
    pub retention_class: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Closed set of report reasons. Serialized snake_case on the wire and in the row.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportReason {
    Spam,
    Harassment,
    Hate,
    Sexual,
    Violence,
    Copyright,
    Csam,
    Threat,
    Other,
}

impl ReportReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Harassment => "harassment",
            Self::Hate => "hate",
            Self::Sexual => "sexual",
            Self::Violence => "violence",
            Self::Copyright => "copyright",
            Self::Csam => "csam",
            Self::Threat => "threat",
            Self::Other => "other",
        }
    }

    /// P0 classes: captured at intake and alerted first (A26 consumes the priority).
    pub fn is_p0(self) -> bool {
        matches!(self, Self::Csam | Self::Threat)
    }

    pub fn priority(self) -> &'static str {
        if self.is_p0() {
            "p0"
        } else {
            "p1"
        }
    }
}

/// What a report points at. Post and comment are database rows the snapshot can
/// capture. Translation is one verse in an immutable translation database: the
/// target id is `<source_id>:<surah>:<ayah>` and no row is captured.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportTargetKind {
    Post,
    Comment,
    Translation,
}

impl ReportTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::Comment => "comment",
            Self::Translation => "translation",
        }
    }
}
