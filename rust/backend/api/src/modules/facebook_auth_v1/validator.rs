use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

/// Facebook OAuth web callback query (W8d).
///
/// Accepts the success shape (`code`+`state`, both non-empty) OR Facebook's cancellation/error
/// shape. Facebook uses both the OAuth2-standard `error` and its own Graph-redirect params
/// (`error_code`/`error_message`/`error_reason`). The controller branches on [`Self::is_error`].
#[derive(Debug, Deserialize, Serialize)]
pub struct FacebookCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    // OAuth2-standard error param (some Facebook flows).
    #[serde(default)]
    pub error: Option<String>,
    // Facebook Graph redirect error params.
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub error_reason: Option<String>,
}

impl FacebookCallbackQuery {
    /// Facebook signalled cancellation or an error (`?error_reason=user_denied`, `?error_code=...`).
    pub fn is_error(&self) -> bool {
        self.error.is_some()
            || self.error_code.is_some()
            || self.error_message.is_some()
            || self.error_reason.is_some()
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

impl Validate for FacebookCallbackQuery {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn success(code: &str, state: &str) -> FacebookCallbackQuery {
        FacebookCallbackQuery {
            code: Some(code.to_string()),
            state: Some(state.to_string()),
            error: None,
            error_code: None,
            error_message: None,
            error_reason: None,
        }
    }

    fn empty() -> FacebookCallbackQuery {
        FacebookCallbackQuery {
            code: None,
            state: None,
            error: None,
            error_code: None,
            error_message: None,
            error_reason: None,
        }
    }

    #[test]
    fn accepts_success_shape() {
        let query = success("fbc", "fbs");
        assert!(query.validate().is_ok());
        assert!(!query.is_error());
    }

    #[test]
    fn accepts_facebook_graph_error_code() {
        // Facebook Graph redirect: ?error_code=200&error_message=Permissions error
        let mut query = empty();
        query.error_code = Some("200".to_string());
        query.error_message = Some("Permissions error".to_string());
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn accepts_facebook_denied_reason() {
        let mut query = empty();
        query.error_reason = Some("user_denied".to_string());
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn accepts_standard_error_param() {
        let mut query = empty();
        query.error = Some("access_denied".to_string());
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
        let mut query = empty();
        query.state = Some("s".to_string());
        assert!(query.validate().is_err());
    }

    #[test]
    fn rejects_empty_query() {
        let query = empty();
        assert!(query.validate().is_err());
    }
}
