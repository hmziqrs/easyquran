use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

/// Discord OAuth web callback query.
///
/// Accepts the success shape (`code`+`state`, both non-empty) OR the OAuth2-standard
/// cancellation/error shape (`error`/`error_description`/`error_uri`). The controller
/// branches on [`Self::is_error`] and redirects to the opaque frontend failure path.
#[derive(Debug, Deserialize, Serialize)]
pub struct DiscordCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    // OAuth2-standard error params used by Discord on cancel/deny.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
    #[serde(default)]
    pub error_uri: Option<String>,
}

impl DiscordCallbackQuery {
    /// Discord signalled cancellation or an error (`?error=access_denied`, etc.).
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

impl Validate for DiscordCallbackQuery {
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
pub struct DiscordExchangeRequest {
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 1))]
    pub state: String,
}

// Native Discord OAuth flow: the app obtains a user access token and posts it here.
#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct DiscordTokenRequest {
    #[validate(length(min = 1, max = 16384))]
    pub access_token: String,
}

/// Discord `/users/@me` profile. The snowflake id is kept as a string — it is stored
/// verbatim as the provider subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUserInfo {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub avatar: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success(code: &str, state: &str) -> DiscordCallbackQuery {
        DiscordCallbackQuery {
            code: Some(code.to_string()),
            state: Some(state.to_string()),
            error: None,
            error_description: None,
            error_uri: None,
        }
    }

    fn empty() -> DiscordCallbackQuery {
        DiscordCallbackQuery {
            code: None,
            state: None,
            error: None,
            error_description: None,
            error_uri: None,
        }
    }

    #[test]
    fn accepts_success_shape() {
        let query = success("dsc", "dss");
        assert!(query.validate().is_ok());
        assert!(!query.is_error());
    }

    #[test]
    fn accepts_discord_cancellation() {
        let mut query = empty();
        query.error = Some("access_denied".to_string());
        assert!(query.validate().is_ok());
        assert!(query.is_error());
    }

    #[test]
    fn accepts_error_with_description_and_uri() {
        let mut query = empty();
        query.error = Some("invalid_scope".to_string());
        query.error_description = Some("bad scope".to_string());
        query.error_uri = Some("https://discord.com/developers/docs".to_string());
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
        assert!(empty().validate().is_err());
    }

    #[test]
    fn exchange_request_requires_code_and_state() {
        assert!(DiscordExchangeRequest {
            code: "c".to_string(),
            state: "s".to_string()
        }
        .validate()
        .is_ok());
        assert!(DiscordExchangeRequest {
            code: String::new(),
            state: "s".to_string()
        }
        .validate()
        .is_err());
        assert!(DiscordExchangeRequest {
            code: "c".to_string(),
            state: String::new()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn user_info_tolerates_missing_optional_fields() {
        let info: DiscordUserInfo = serde_json::from_value(serde_json::json!({
            "id": "80351110224678912",
            "username": "sandbox"
        }))
        .unwrap();

        assert_eq!(info.id, "80351110224678912");
        assert!(info.email.is_none());
        assert!(info.verified.is_none());
        assert!(info.avatar.is_none());
    }

    #[test]
    fn token_request_accepts_bounded_access_token() {
        let request = DiscordTokenRequest {
            access_token: "discord-user-token".to_string(),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn token_request_rejects_empty_or_oversized_access_token() {
        let empty = DiscordTokenRequest {
            access_token: String::new(),
        };
        let oversized = DiscordTokenRequest {
            access_token: "x".repeat(16385),
        };

        assert!(empty.validate().is_err());
        assert!(oversized.validate().is_err());
    }

    #[test]
    fn token_request_rejects_unknown_fields() {
        let result = serde_json::from_value::<DiscordTokenRequest>(serde_json::json!({
            "access_token": "discord-user-token",
            "unexpected": true
        }));

        assert!(result.is_err());
    }
}
