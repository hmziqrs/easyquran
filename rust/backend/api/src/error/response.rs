use super::codes::ErrorCode;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub code: ErrorCode,

    #[cfg(debug_assertions)]
    pub message: String,

    #[cfg(not(debug_assertions))]
    #[serde(skip)]
    pub message: String,

    pub status: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg(debug_assertions)]
    pub details: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ErrorResponse {
    pub fn new(code: ErrorCode) -> Self {
        let status = code.status_code();
        Self {
            message: code.default_message().to_string(),
            code,
            status,
            #[cfg(debug_assertions)]
            details: None,
            context: None,
            retry_after: None,
            request_id: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        #[cfg(debug_assertions)]
        {
            self.details = Some(details.into());
        }
        self
    }

    pub fn with_context(mut self, context: impl Serialize) -> Self {
        match serde_json::to_value(context) {
            Ok(value) => self.context = Some(value),
            Err(err) => {
                eprintln!("Failed to serialize error context: {}", err);
            }
        }
        self
    }

    pub fn with_retry_after(mut self, secs: u64) -> Self {
        self.retry_after = Some(secs);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let status_u16 = self.code.status_code();
        let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        if status.is_server_error() {
            eprintln!("Server error {}: {}", self.code, self.message);
            #[cfg(debug_assertions)]
            if let Some(details) = &self.details {
                eprintln!("  Details: {}", details);
            }
        }

        let retry_after = self.retry_after;

        let mut body = self;
        body.status = status_u16;

        let mut response = (status, Json(body)).into_response();

        if let Some(secs) = retry_after {
            if let Ok(value) = axum::http::HeaderValue::from_str(&secs.to_string()) {
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
        }

        response
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

pub trait IntoErrorResponse {
    fn into_error_response(self) -> ErrorResponse;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes::ErrorCode;

    #[test]
    fn new_has_code_and_status() {
        let err = ErrorResponse::new(ErrorCode::RecordNotFound);
        assert_eq!(err.code, ErrorCode::RecordNotFound);
        assert_eq!(err.status, 404u16);
    }

    #[test]
    fn with_message_overrides_default() {
        let err = ErrorResponse::new(ErrorCode::InvalidCredentials).with_message("Custom message");
        assert_eq!(err.message, "Custom message");
    }

    #[test]
    fn with_retry_after_sets_field() {
        let err = ErrorResponse::new(ErrorCode::RateLimited).with_retry_after(60);
        assert_eq!(err.retry_after, Some(60));
    }

    #[test]
    fn with_request_id_sets_field() {
        let err = ErrorResponse::new(ErrorCode::InternalServerError).with_request_id("req-123");
        assert_eq!(err.request_id, Some("req-123".to_string()));
    }

    #[test]
    fn with_context_sets_json_value() {
        let err = ErrorResponse::new(ErrorCode::ValidationError)
            .with_context(serde_json::json!({"field": "email"}));
        assert!(err.context.is_some());
        assert_eq!(err.context.unwrap()["field"], "email");
    }

    #[test]
    fn builder_chaining() {
        let err = ErrorResponse::new(ErrorCode::RateLimited)
            .with_message("Slow down")
            .with_retry_after(30)
            .with_request_id("abc");

        assert_eq!(err.message, "Slow down");
        assert_eq!(err.retry_after, Some(30));
        assert_eq!(err.request_id, Some("abc".to_string()));
    }

    #[test]
    fn display_format() {
        let err = ErrorResponse::new(ErrorCode::RecordNotFound).with_message("Post not found");
        let s = format!("{}", err);
        assert!(s.contains("DB_002"));
        assert!(s.contains("Post not found"));
    }

    #[test]
    fn into_response_derives_status_from_code() {
        let err = ErrorResponse::new(ErrorCode::DuplicateEntry);
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);
    }
}
