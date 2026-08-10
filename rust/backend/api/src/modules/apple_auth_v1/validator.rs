use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

/// Apple Sign in with Apple web callback query (W8d).
///
/// Accepts the success shape (`code`+`state`, both non-empty) OR the Apple cancellation/error
/// shape (`error`/`error_description`). The controller branches on [`Self::is_error`] and
/// redirects to the opaque frontend failure path. Validation stays strict for the success shape.
#[derive(Debug, Deserialize, Serialize)]
pub struct AppleCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    // Apple reports cancel/error via the OAuth2 standard `error`/`error_description` params.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

impl AppleCallbackQuery {
    /// Apple signalled cancellation or an error (`?error=user_cancelled`, etc.).
    pub fn is_error(&self) -> bool {
        self.error.is_some() || self.error_description.is_some()
    }

    pub fn code(&self) -> Result<String, crate::error::ErrorResponse> {
        self.code
            .clone()
            .filter(|c| !c.is_empty())
            .ok_or_else(|| crate::error::ErrorResponse::new(crate::error::ErrorCode::InvalidInput))
    }

    pub fn state(&self) -> Result<String, crate::error::ErrorResponse> {
        self.state
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| crate::error::ErrorResponse::new(crate::error::ErrorCode::InvalidInput))
    }

    fn is_success(&self) -> bool {
        matches!(self.code.as_deref(), Some(c) if !c.is_empty())
            && matches!(self.state.as_deref(), Some(s) if !s.is_empty())
    }
}

impl Validate for AppleCallbackQuery {
    fn validate(&self) -> Result<(), ValidationErrors> {
        if self.is_success() || self.is_error() {
            Ok(())
        } else {
            let mut errs = ValidationErrors::new();
            errs.add(
                "callback",
                ValidationError::new("callback")
                    .with_message(Cow::Borrowed("missing OAuth code+state or provider error")),
            );
            Err(errs)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn success(code: &str, state: &str) -> AppleCallbackQuery {
        AppleCallbackQuery {
            code: Some(code.to_string()),
            state: Some(state.to_string()),
            error: None,
            error_description: None,
        }
    }

    fn error_shape(error: Option<&str>, desc: Option<&str>) -> AppleCallbackQuery {
        AppleCallbackQuery {
            code: None,
            state: None,
            error: error.map(str::to_string),
            error_description: desc.map(str::to_string),
        }
    }

    #[test]
    fn accepts_success_shape() {
        let query = success("cabc", "sxyz");
        assert!(query.validate().is_ok());
        assert!(!query.is_error());
    }

    #[test]
    fn accepts_apple_cancellation() {
        let query = error_shape(Some("user_cancelled"), None);
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn accepts_apple_error_with_description() {
        let query = error_shape(Some("invalid_request"), Some("bad state"));
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn rejects_empty_success() {
        let query = success("", "");
        assert!(query.validate().is_err());
        assert!(!query.is_error());
    }

    #[test]
    fn rejects_partial_success() {
        let query = success("c", "");
        assert!(query.validate().is_err());
    }

    #[test]
    fn rejects_empty_query() {
        let query = AppleCallbackQuery {
            code: None,
            state: None,
            error: None,
            error_description: None,
        };
        assert!(query.validate().is_err());
    }
}
