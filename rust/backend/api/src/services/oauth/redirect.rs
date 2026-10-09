use tracing::warn;

use crate::error::{ErrorCode, ErrorResponse};

use super::login::OAuthProvider;

// Fail closed: reject rather than redirect to an unvalidated origin (open-redirect defense).
#[allow(clippy::result_large_err)]
pub fn build_allowed_success_redirect(path: &str) -> Result<String, ErrorResponse> {
    let origin = crate::config::settings::frontend_origin().map_err(|error| {
        warn!(error = %error, "Post-login redirect origin is not configured");
        ErrorResponse::new(ErrorCode::ConfigurationError).with_message(error)
    })?;
    let allowed = crate::config::settings::oauth_allowed_redirect_origins();
    if !allowed.contains(&origin) {
        warn!(origin = %origin, "FRONTEND_URL origin rejected by OAUTH_ALLOWED_REDIRECT_ORIGINS");
        return Err(ErrorResponse::new(ErrorCode::ConfigurationError)
            .with_message("Post-login redirect origin is not allowed"));
    }

    Ok(format!("{origin}{path}"))
}

/// Opaque failure code carried in the failure-redirect query as `?ec=<code>`.
///
/// These are short, provider-independent, and carry NO IdP payload, message, code, state, email,
/// or subject — the frontend maps each token to localized copy. Adding a variant is safe; reuse
/// the existing set before adding narrowly-detailed ones (W8d/W8f privacy invariants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureCode(&'static str);

impl FailureCode {
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

/// Provider signalled cancellation or an error redirect (`?error=...`).
pub const FAILURE_CANCELLED: FailureCode = FailureCode("cancel");
/// Upstream token exchange or identity-token verification failure.
pub const FAILURE_AUTH: FailureCode = FailureCode("auth");
/// Account link/create refused (e.g. IdP email unverified).
pub const FAILURE_LINK: FailureCode = FailureCode("link");
/// Unspecified server-side failure.
pub const FAILURE_SERVER: FailureCode = FailureCode("server");

/// Frontend success path for a provider: `/auth/{provider}/success`.
pub fn success_path(provider: OAuthProvider) -> String {
    format!("/auth/{}/success", provider.as_str())
}

/// Frontend failure path for a provider: `/auth/{provider}/failure?ec=<opaque>`.
/// Only the opaque code enters the URL — never code, state, or provider payload.
pub fn failure_path(provider: OAuthProvider, code: FailureCode) -> String {
    format!("/auth/{}/failure?ec={}", provider.as_str(), code.as_str())
}

/// Map an in-controller error to an opaque failure code (no message/payload leakage into the URL).
pub fn error_to_failure_code(err: &ErrorResponse) -> FailureCode {
    match err.code {
        ErrorCode::OperationNotAllowed => FAILURE_LINK,
        ErrorCode::InvalidToken | ErrorCode::ExternalServiceError => FAILURE_AUTH,
        _ => FAILURE_SERVER,
    }
}

/// Absolute allow-listed success redirect: `{allowed_origin}/auth/{provider}/success`.
#[allow(clippy::result_large_err)]
pub fn build_success_redirect(provider: OAuthProvider) -> Result<String, ErrorResponse> {
    build_allowed_success_redirect(&success_path(provider))
}

/// Absolute allow-listed failure redirect: `{allowed_origin}/auth/{provider}/failure?ec=<opaque>`.
#[allow(clippy::result_large_err)]
pub fn build_failure_redirect(
    provider: OAuthProvider,
    code: FailureCode,
) -> Result<String, ErrorResponse> {
    build_allowed_success_redirect(&failure_path(provider, code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes::ErrorCode;

    #[test]
    fn success_path_is_provider_relative() {
        assert_eq!(success_path(OAuthProvider::Google), "/auth/google/success");
        assert_eq!(success_path(OAuthProvider::Apple), "/auth/apple/success");
        assert_eq!(
            success_path(OAuthProvider::Facebook),
            "/auth/facebook/success"
        );
        assert_eq!(success_path(OAuthProvider::Github), "/auth/github/success");
        assert_eq!(
            success_path(OAuthProvider::Discord),
            "/auth/discord/success"
        );
    }

    #[test]
    fn failure_path_carries_only_opaque_code() {
        let url = failure_path(OAuthProvider::Google, FAILURE_CANCELLED);
        assert_eq!(url, "/auth/google/failure?ec=cancel");
        // Load-bearing privacy invariant: nothing provider-supplied leaks into the path.
        assert!(!url.contains("code="));
        assert!(!url.contains("state="));
        assert!(!url.contains("error_description"));
        assert!(!url.contains("access_token"));
    }

    #[test]
    fn failure_path_each_provider() {
        assert_eq!(
            failure_path(OAuthProvider::Apple, FAILURE_AUTH),
            "/auth/apple/failure?ec=auth"
        );
        assert_eq!(
            failure_path(OAuthProvider::Facebook, FAILURE_LINK),
            "/auth/facebook/failure?ec=link"
        );
        assert_eq!(
            failure_path(OAuthProvider::Github, FAILURE_SERVER),
            "/auth/github/failure?ec=server"
        );
        assert_eq!(
            failure_path(OAuthProvider::Discord, FAILURE_CANCELLED),
            "/auth/discord/failure?ec=cancel"
        );
    }

    #[test]
    fn error_to_failure_code_maps_categories() {
        assert_eq!(
            error_to_failure_code(&ErrorResponse::new(ErrorCode::OperationNotAllowed)),
            FAILURE_LINK
        );
        assert_eq!(
            error_to_failure_code(&ErrorResponse::new(ErrorCode::InvalidToken)),
            FAILURE_AUTH
        );
        assert_eq!(
            error_to_failure_code(&ErrorResponse::new(ErrorCode::ExternalServiceError)),
            FAILURE_AUTH
        );
        assert_eq!(
            error_to_failure_code(&ErrorResponse::new(ErrorCode::InternalServerError)),
            FAILURE_SERVER
        );
        assert_eq!(
            error_to_failure_code(&ErrorResponse::new(ErrorCode::Unauthorized)),
            FAILURE_SERVER
        );
    }

    #[test]
    fn failure_codes_are_opaque_short_tokens() {
        // Short, non-leaky tokens only — no PII, no provider payload.
        for c in [
            FAILURE_CANCELLED,
            FAILURE_AUTH,
            FAILURE_LINK,
            FAILURE_SERVER,
        ] {
            let s = c.as_str();
            assert!(s.len() <= 8);
            assert!(!s.contains('@'));
        }
    }
}
