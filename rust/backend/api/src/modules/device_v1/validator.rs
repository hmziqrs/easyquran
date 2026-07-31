use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1RegisterDevicePayload {
    #[validate(length(min = 1, max = 4096))]
    pub token: String,
    #[validate(length(min = 1, max = 64))]
    pub platform: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct V1DeleteDevicePayload {
    #[validate(length(min = 1, max = 4096))]
    pub token: String,
}
