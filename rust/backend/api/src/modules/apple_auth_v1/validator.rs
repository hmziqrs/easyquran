use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AppleCallbackQuery {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AppleExchangeRequest {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

// Mobile (native Sign in with Apple) flow: the app obtains an identity_token natively and posts it here — no web redirect/code/state round-trip.
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AppleTokenRequest {
    #[validate(length(min = 1))]
    pub identity_token: String,
}

// Apple sends `email_verified` / `is_private_email` as the STRINGS "true"/"false", not booleans — keep them as Option<String> or deserialization/verification breaks.
#[derive(Debug, Clone, Deserialize)]
pub struct AppleIdTokenClaims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<String>,
    #[serde(default)]
    pub is_private_email: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
}

impl AppleIdTokenClaims {
    pub fn is_email_verified(&self) -> bool {
        self.email_verified.as_deref() == Some("true")
    }
}
