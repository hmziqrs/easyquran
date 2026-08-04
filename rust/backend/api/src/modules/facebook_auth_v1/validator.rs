use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct FacebookCallbackQuery {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct FacebookExchangeRequest {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

// Mobile (Facebook Login SDK) flow: the app obtains a user access_token natively and posts it here — no web redirect/code exchange round-trip.
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct FacebookTokenRequest {
    #[validate(length(min = 1))]
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookUserInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}
