use serde::{Deserialize, Serialize};

#[cfg_attr(
    feature = "backend",
    derive(sea_orm::DeriveActiveEnum, strum::EnumIter)
)]
#[cfg_attr(feature = "backend", sea_orm(rs_type = "String", db_type = "Text"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuppressionReason {
    #[serde(rename = "bounce")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "bounce"))]
    Bounce,
    #[serde(rename = "complaint")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "complaint"))]
    Complaint,
    #[serde(rename = "manual")]
    #[cfg_attr(feature = "backend", sea_orm(string_value = "manual"))]
    Manual,
}
