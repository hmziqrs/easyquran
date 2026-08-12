use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

/// Google OAuth2 web callback query.
///
/// Two accepted shapes (W8d): the success shape (`code`+`state`, both non-empty) OR the
/// provider cancellation/error shape (`error`/`error_description`/`error_reason`). A
/// cancellation/error is detected via [`Self::is_error`] and the controller redirects to the opaque
/// frontend failure path rather than attempting an exchange. Validation stays strict for the
/// success shape: a query carrying neither a complete success nor a provider error is rejected.
#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    // OAuth2 standard error params sent by Google on cancel/deny.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
    #[serde(default)]
    pub error_reason: Option<String>,
}

impl GoogleCallbackQuery {
    /// Provider signalled cancellation or an error (`?error=access_denied`, etc.).
    pub fn is_error(&self) -> bool {
        self.error.is_some() || self.error_description.is_some() || self.error_reason.is_some()
    }

    /// Success-shape authorization code; errors if this is not a success-shape callback.
    pub fn code(&self) -> Result<String, crate::error::ErrorResponse> {
        self.code
            .clone()
            .filter(|c| !c.is_empty())
            .ok_or_else(|| crate::error::ErrorResponse::new(crate::error::ErrorCode::InvalidInput))
    }

    /// Success-shape state token; errors if this is not a success-shape callback.
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

impl Validate for GoogleCallbackQuery {
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
pub struct GoogleExchangeRequest {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

// Mobile Google Sign-In flow: id_token proves identity; optional access_token lets the backend
// fetch current profile claims from Google UserInfo without trusting client-supplied profile data.
#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct GoogleTokenRequest {
    #[validate(length(min = 1, max = 16384))]
    pub id_token: String,
    #[serde(default)]
    #[validate(length(min = 1, max = 16384))]
    pub access_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub verified_email: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success(code: &str, state: &str) -> GoogleCallbackQuery {
        GoogleCallbackQuery {
            code: Some(code.to_string()),
            state: Some(state.to_string()),
            error: None,
            error_description: None,
            error_reason: None,
        }
    }

    fn error_shape(
        error: Option<&str>,
        desc: Option<&str>,
        reason: Option<&str>,
    ) -> GoogleCallbackQuery {
        GoogleCallbackQuery {
            code: None,
            state: None,
            error: error.map(str::to_string),
            error_description: desc.map(str::to_string),
            error_reason: reason.map(str::to_string),
        }
    }

    #[test]
    fn accepts_success_shape() {
        let query = success("abc", "xyz");
        assert!(query.validate().is_ok());
        assert!(!query.is_error());
    }

    #[test]
    fn accepts_cancellation_error_only() {
        let query = error_shape(Some("access_denied"), None, None);
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn accepts_cancellation_with_description() {
        let query = error_shape(
            Some("access_denied"),
            Some("User cancelled"),
            Some("user_denied"),
        );
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
    fn rejects_partial_success_missing_state() {
        let query = success("abc", "");
        assert!(query.validate().is_err());
    }

    #[test]
    fn rejects_empty_query() {
        let query = GoogleCallbackQuery {
            code: None,
            state: None,
            error: None,
            error_description: None,
            error_reason: None,
        };
        assert!(query.validate().is_err());
    }

    #[test]
    fn mobile_token_request_accepts_google_access_token() {
        let request: GoogleTokenRequest = serde_json::from_value(serde_json::json!({
            "id_token": "signed-google-id-token",
            "access_token": "google-user-access-token"
        }))
        .unwrap();

        assert_eq!(
            request.access_token.as_deref(),
            Some("google-user-access-token")
        );
        assert!(request.validate().is_ok());
    }

    #[test]
    fn mobile_token_request_keeps_access_token_optional() {
        let request: GoogleTokenRequest = serde_json::from_value(serde_json::json!({
            "id_token": "signed-google-id-token"
        }))
        .unwrap();

        assert_eq!(request.access_token, None);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn mobile_token_request_rejects_empty_access_token() {
        let request: GoogleTokenRequest = serde_json::from_value(serde_json::json!({
            "id_token": "signed-google-id-token",
            "access_token": ""
        }))
        .unwrap();

        assert!(request.validate().is_err());
    }

    #[test]
    fn mobile_token_request_rejects_client_supplied_identity() {
        let request = serde_json::from_value::<GoogleTokenRequest>(serde_json::json!({
            "id_token": "signed-google-id-token",
            "email": "attacker-controlled@example.com",
            "name": "Attacker Controlled"
        }));

        assert!(request.is_err());
    }

    #[test]
    fn mobile_token_request_rejects_oversized_id_token() {
        let request = GoogleTokenRequest {
            id_token: "x".repeat(16385),
            access_token: None,
        };

        assert!(request.validate().is_err());
    }
}
