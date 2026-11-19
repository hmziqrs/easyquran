//! The backend error envelope, decoded (mirrors `decodeErrorEnvelope` and
//! `classifyAuthError` in `web/src/lib/auth/{auth-client,auth-copy}.ts`).
//!
//! Two wire shapes reach a client:
//! - `ErrorResponse` (`rust/backend/api/src/error/response.rs`): fields at
//!   the top level, the code in `type` (`AUTH_001`, `VAL_001`, ...),
//!   `status`, optional `context`, `retry_after`, `request_id`. `message`
//!   is sent by debug builds only.
//! - `rux_auth::AuthError` from the auth guards: `{"error":{"code",
//!   "message","context"}}` with codes like `AUTH_UNAUTHENTICATED`.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::net::NetError;

/// `ErrorResponse` code of a banned account (`AccountLocked`).
pub const BANNED: &str = "AUTH_006";
/// Guard code of a banned account.
pub const GUARD_BANNED: &str = "AUTH_BANNED";
/// `InvalidToken`: a CSRF refusal, or a wrong 2FA code.
pub const INVALID_TOKEN: &str = "AUTH_009";
/// `SessionExpired`.
pub const SESSION_EXPIRED: &str = "AUTH_003";
/// Guard: no session user.
pub const UNAUTHENTICATED: &str = "AUTH_UNAUTHENTICATED";
/// Guard: the route is for signed-out callers and this session is signed in.
pub const ALREADY_AUTHENTICATED: &str = "AUTH_ALREADY_AUTHENTICATED";
/// `EmailVerificationRequired`.
pub const VERIFICATION_REQUIRED: &str = "AUTH_008";
/// Guard: the account is not verified.
pub const GUARD_VERIFICATION_REQUIRED: &str = "AUTH_VERIFICATION_REQUIRED";

/// `context.reason` values of a CSRF refusal (`CsrfError`).
const CSRF_REASONS: &[&str] = &[
    "missing",
    "invalid_header",
    "invalid_base64",
    "invalid_utf8",
    "mismatch",
];
const CREDENTIAL_CODES: &[&str] = &["AUTH_001", "AUTH_002", "AUTH_003", "AUTH_004", "AUTH_009"];
const RATE_LIMIT_CODES: &[&str] = &["AUTH_007", "SRV_004", "AUTH_RATE_LIMITED"];

/// A non-2xx reply, decoded. Holds no secret: the backend never echoes one.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpError {
    pub status: u16,
    /// `type` (ErrorResponse) or `error.code` (guard).
    pub code: Option<String>,
    /// Debug builds of the API only.
    pub message: Option<String>,
    pub context: Option<Value>,
    /// `retry_after` in the body, else the `Retry-After` header.
    pub retry_after: Option<u64>,
    pub request_id: Option<String>,
}

impl HttpError {
    /// Decodes `body` of a reply with `status`. Never fails: an unreadable
    /// body leaves every field empty.
    pub fn decode(status: u16, body: &Value, retry_after_header: Option<u64>) -> Self {
        let mut out = Self {
            status,
            code: None,
            message: None,
            context: None,
            retry_after: retry_after_header,
            request_id: None,
        };
        let Some(root) = body.as_object() else {
            return out;
        };
        let nested = root.get("error").and_then(Value::as_object);
        let o = match (root.get("type").and_then(Value::as_str), nested) {
            (None, Some(inner)) => inner,
            _ => root,
        };
        out.code = o
            .get("type")
            .and_then(Value::as_str)
            .or_else(|| o.get("code").and_then(Value::as_str))
            .map(str::to_owned);
        out.message = o.get("message").and_then(Value::as_str).map(str::to_owned);
        out.context = o.get("context").filter(|v| !v.is_null()).cloned();
        // `retryAfter`: the sync routes' throttle body.
        let body_retry = o.get("retry_after").or_else(|| o.get("retryAfter"));
        if let Some(secs) = body_retry.and_then(Value::as_u64) {
            out.retry_after = Some(secs);
        }
        out.request_id = o
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        out
    }

    fn code_is(&self, code: &str) -> bool {
        self.code.as_deref() == Some(code)
    }

    fn context_str(&self, key: &str) -> Option<&str> {
        self.context.as_ref()?.get(key)?.as_str()
    }

    /// A CSRF refusal (401 `AUTH_009` with a CSRF `context.reason`), as
    /// opposed to a wrong 2FA code, which shares the code.
    pub fn is_csrf_refusal(&self) -> bool {
        self.status == 401
            && self.code_is(INVALID_TOKEN)
            && self
                .context_str("reason")
                .is_some_and(|r| CSRF_REASONS.contains(&r))
    }

    /// The session is gone: expired, signed out elsewhere, revoked or never
    /// signed in. 401 from the auth guard or `SessionExpired`.
    pub fn is_session_lost(&self) -> bool {
        self.status == 401 && (self.code_is(UNAUTHENTICATED) || self.code_is(SESSION_EXPIRED))
    }

    /// The account is banned: the notice, else `None`.
    pub fn ban_notice(&self) -> Option<BanNotice> {
        if !(self.code_is(BANNED) || self.code_is(GUARD_BANNED)) {
            return None;
        }
        Some(BanNotice::read(self.context.as_ref()))
    }

    /// The signed-out-only route found this session signed in (409).
    pub fn is_already_authenticated(&self) -> bool {
        self.status == 409 && self.code_is(ALREADY_AUTHENTICATED)
    }

    /// The account must verify its email first.
    pub fn is_verification_required(&self) -> bool {
        self.code_is(VERIFICATION_REQUIRED) || self.code_is(GUARD_VERIFICATION_REQUIRED)
    }

    /// A sensitive action needs a fresh re-authentication
    /// (`context.reason == "fresh_auth_required"`).
    pub fn is_fresh_auth_required(&self) -> bool {
        self.context_str("reason") == Some("fresh_auth_required")
    }

    /// Throttled: 429, a rate-limit code, or a retry hint.
    pub fn is_rate_limited(&self) -> bool {
        self.status == 429
            || self
                .code
                .as_deref()
                .is_some_and(|c| RATE_LIMIT_CODES.contains(&c))
            || self.retry_after.is_some()
    }

    /// Per-field messages for `fields`, from `context` (or `context.errors`):
    /// a string, a list of strings or `{message}`, or one `{message}`.
    pub fn field_errors(&self, fields: &[&str]) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let Some(root) = self.context.as_ref().and_then(Value::as_object) else {
            return out;
        };
        let bucket = root
            .get("errors")
            .and_then(Value::as_object)
            .unwrap_or(root);
        for field in fields {
            if let Some(msg) = bucket.get(*field).and_then(pick_field_message) {
                out.insert((*field).to_owned(), msg);
            }
        }
        out
    }

    /// The web's error classes, in the web's order.
    pub fn classify(&self, fields: &[&str]) -> ErrorKind {
        if self.is_rate_limited() {
            return ErrorKind::RateLimited {
                retry_after: self.retry_after,
            };
        }
        if self.status >= 500 {
            return ErrorKind::Server;
        }
        if let Some(notice) = self.ban_notice() {
            return ErrorKind::Banned(notice);
        }
        let field_errors = self.field_errors(fields);
        if self.status == 400 && !field_errors.is_empty() {
            return ErrorKind::Field(field_errors);
        }
        if self.is_verification_required() {
            return ErrorKind::VerifiedOnly;
        }
        let credential_code = self
            .code
            .as_deref()
            .is_some_and(|c| CREDENTIAL_CODES.contains(&c));
        if matches!(self.status, 401 | 403 | 404) || credential_code {
            return ErrorKind::Credential;
        }
        ErrorKind::Other
    }
}

fn non_blank(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.to_owned())
}

fn pick_field_message(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => non_blank(s),
        Value::Array(items) => items.iter().find_map(|entry| match entry {
            Value::String(s) => non_blank(s),
            Value::Object(o) => o.get("message").and_then(Value::as_str).and_then(non_blank),
            _ => None,
        }),
        Value::Object(o) => o.get("message").and_then(Value::as_str).and_then(non_blank),
        _ => None,
    }
}

/// What a banned screen may show (`readBanNotice`): a reason code and an
/// appeal path, each dropped unless well formed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BanNotice {
    pub reason_code: Option<String>,
    pub appeal_path: Option<String>,
}

impl BanNotice {
    fn read(context: Option<&Value>) -> Self {
        let get = |key: &str| context.and_then(|c| c.get(key)).and_then(Value::as_str);
        Self {
            reason_code: get("reasonCode")
                .filter(|c| valid_reason_code(c))
                .map(str::to_owned),
            appeal_path: get("appealPath")
                .filter(|p| p.len() <= 200 && p.starts_with('/') && !p.starts_with("//"))
                .map(str::to_owned),
        }
    }
}

/// `^[a-z][a-z0-9_-]{0,39}$`.
fn valid_reason_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    let Some((first, rest)) = bytes.split_first() else {
        return false;
    };
    first.is_ascii_lowercase()
        && rest.len() <= 39
        && rest
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}

/// The web's error classes (`ErrorKind` in `auth-copy.ts`). Screens map a
/// class to its copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Per-field messages (400 with field context).
    Field(BTreeMap<String, String>),
    /// Wrong credentials, or a session that cannot do this.
    Credential,
    /// The account is banned.
    Banned(BanNotice),
    /// Verify the email first.
    VerifiedOnly,
    /// Throttled; seconds to wait when known.
    RateLimited { retry_after: Option<u64> },
    /// No connection.
    Transport,
    /// 5xx.
    Server,
    /// Anything else.
    Other,
}

/// Why an API call failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ApiError {
    /// No response (offline, refused by the platform).
    #[error("network: {0}")]
    Net(#[from] NetError),
    /// A non-2xx reply. Boxed: the decoded reply is large and every API
    /// call returns this error (`clippy::result_large_err`).
    #[error("http {}: {}", .0.status, .0.code.as_deref().unwrap_or("-"))]
    Http(Box<HttpError>),
    /// A 2xx reply whose body is not the expected shape.
    #[error("unexpected response: {0}")]
    Decode(String),
}

impl ApiError {
    /// The HTTP status; 0 when there was no reply.
    pub fn status(&self) -> u16 {
        match self {
            ApiError::Http(e) => e.status,
            ApiError::Net(_) | ApiError::Decode(_) => 0,
        }
    }

    /// The decoded reply, for a non-2xx status.
    pub fn http(&self) -> Option<&HttpError> {
        match self {
            ApiError::Http(e) => Some(e),
            ApiError::Net(_) | ApiError::Decode(_) => None,
        }
    }

    /// The session is gone (see [`HttpError::is_session_lost`]).
    pub fn is_session_lost(&self) -> bool {
        self.http().is_some_and(HttpError::is_session_lost)
    }

    /// The web's error class.
    pub fn classify(&self, fields: &[&str]) -> ErrorKind {
        match self {
            ApiError::Net(_) => ErrorKind::Transport,
            ApiError::Http(e) => e.classify(fields),
            ApiError::Decode(_) => ErrorKind::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn decodes_both_wire_shapes() {
        let flat = HttpError::decode(
            401,
            &json!({"type":"AUTH_009","status":401,"context":{"reason":"mismatch"}}),
            None,
        );
        assert!(flat.is_csrf_refusal());
        let guard = HttpError::decode(
            401,
            &json!({"error":{"code":"AUTH_UNAUTHENTICATED","message":"Authentication required"}}),
            None,
        );
        assert!(guard.is_session_lost());
        assert_eq!(guard.message.as_deref(), Some("Authentication required"));
    }

    #[test]
    fn a_wrong_totp_code_is_not_a_csrf_refusal() {
        let totp = HttpError::decode(401, &json!({"type":"AUTH_009","status":401}), None);
        assert!(!totp.is_csrf_refusal());
        assert_eq!(totp.classify(&["code"]), ErrorKind::Credential);
    }

    #[test]
    fn ban_notice_drops_malformed_values() {
        let e = HttpError::decode(
            403,
            &json!({"type":"AUTH_006","context":{"reasonCode":"spam","appealPath":"//evil"}}),
            None,
        );
        assert_eq!(
            e.classify(&[]),
            ErrorKind::Banned(BanNotice {
                reason_code: Some("spam".into()),
                appeal_path: None
            })
        );
    }

    #[test]
    fn field_errors_read_validator_shapes() {
        let e = HttpError::decode(
            400,
            &json!({"type":"VAL_001","context":{"email":[{"message":"bad email"}],"name":"  "}}),
            None,
        );
        let kind = e.classify(&["email", "name"]);
        let ErrorKind::Field(map) = kind else {
            panic!("field kind")
        };
        assert_eq!(map.get("email").map(String::as_str), Some("bad email"));
        assert!(!map.contains_key("name"));
    }

    #[test]
    fn rate_limit_reads_header_and_body() {
        let e = HttpError::decode(429, &Value::Null, Some(30));
        assert_eq!(
            e.classify(&[]),
            ErrorKind::RateLimited {
                retry_after: Some(30)
            }
        );
    }
}
