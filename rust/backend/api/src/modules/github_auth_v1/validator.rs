use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

/// GitHub OAuth web callback query (W8d).
///
/// Accepts the success shape (`code`+`state`, both non-empty) OR the GitHub cancellation/error
/// shape (OAuth2 standard `error`/`error_description`/`error_uri`). The controller branches on
/// [`Self::is_error`] and redirects to the opaque frontend failure path.
#[derive(Debug, Deserialize, Serialize)]
pub struct GitHubCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    // OAuth2-standard error params used by GitHub on cancel/deny.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
    #[serde(default)]
    pub error_uri: Option<String>,
}

impl GitHubCallbackQuery {
    /// GitHub signalled cancellation or an error (`?error=access_denied`, etc.).
    pub fn is_error(&self) -> bool {
        self.error.is_some() || self.error_description.is_some() || self.error_uri.is_some()
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

impl Validate for GitHubCallbackQuery {
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
pub struct GitHubExchangeRequest {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

// Native GitHub OAuth flow: the app obtains a user access token and posts it here.
#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct GitHubTokenRequest {
    #[validate(length(min = 1, max = 16384))]
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUserInfo {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
    #[serde(default)]
    pub visibility: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success(code: &str, state: &str) -> GitHubCallbackQuery {
        GitHubCallbackQuery {
            code: Some(code.to_string()),
            state: Some(state.to_string()),
            error: None,
            error_description: None,
            error_uri: None,
        }
    }

    fn empty() -> GitHubCallbackQuery {
        GitHubCallbackQuery {
            code: None,
            state: None,
            error: None,
            error_description: None,
            error_uri: None,
        }
    }

    #[test]
    fn accepts_success_shape() {
        let query = success("ghc", "ghs");
        assert!(query.validate().is_ok());
        assert!(!query.is_error());
    }

    #[test]
    fn accepts_github_cancellation() {
        let mut query = empty();
        query.error = Some("access_denied".to_string());
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn accepts_github_error_with_description_and_uri() {
        let mut query = empty();
        query.error = Some("bad_verification_code".to_string());
        query.error_description = Some("expired".to_string());
        query.error_uri = Some("https://docs.github.com".to_string());
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
        query.code = Some("c".to_string());
        assert!(query.validate().is_err());
    }

    #[test]
    fn rejects_empty_query() {
        let query = empty();
        assert!(query.validate().is_err());
    }

    #[test]
    fn token_request_accepts_bounded_access_token() {
        let request = GitHubTokenRequest {
            access_token: "github-user-token".to_string(),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn token_request_rejects_empty_or_oversized_access_token() {
        let empty = GitHubTokenRequest {
            access_token: String::new(),
        };
        let oversized = GitHubTokenRequest {
            access_token: "x".repeat(16385),
        };

        assert!(empty.validate().is_err());
        assert!(oversized.validate().is_err());
    }

    #[test]
    fn token_request_rejects_unknown_fields() {
        let result = serde_json::from_value::<GitHubTokenRequest>(serde_json::json!({
            "access_token": "github-user-token",
            "unexpected": true
        }));

        assert!(result.is_err());
    }
}
