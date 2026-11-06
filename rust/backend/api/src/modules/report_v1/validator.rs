use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::db::sea_models::report::{ReportReason, ReportTargetKind};

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1SubmitReportPayload {
    pub target_kind: ReportTargetKind,
    /// Row id for post and comment; `<source_id>:<surah>:<ayah>` for translation.
    #[validate(length(min = 1, max = 64))]
    pub target_id: String,
    pub reason: ReportReason,
    #[validate(length(max = 2000))]
    pub details: Option<String>,
    #[validate(email, length(max = 320))]
    pub contact_email: Option<String>,
}
