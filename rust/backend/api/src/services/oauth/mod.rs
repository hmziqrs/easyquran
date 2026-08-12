pub mod login;
pub mod redirect;
pub mod state;

use axum::http::{header::ORIGIN, HeaderMap};
use tracing::warn;

use crate::error::{ErrorCode, ErrorResponse};

pub use login::{
    find_or_create_user_for_oauth, finish_oauth_login, generate_oauth_nonce, OAuthProvider,
};
pub use redirect::build_allowed_success_redirect;
pub use state::{
    consume_native_token_nonce, consume_oauth_state, issue_native_token_challenge,
    oauth_session_id, store_oauth_state, ConsumedOauthState, NativeTokenProvider,
    NATIVE_TOKEN_NONCE_TTL_SECS,
};

/// Native provider-token endpoints are CSRF-exempt because native SDK clients have no browser
/// session. Reject browser-origin requests so they cannot create or replace a cookie session.
pub fn ensure_native_token_request(headers: &HeaderMap) -> Result<(), ErrorResponse> {
    if headers.contains_key(ORIGIN) {
        warn!("Rejecting browser-origin request to native provider-token endpoint");
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("This endpoint accepts native app requests only"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::http::{header::ORIGIN, HeaderMap};

    use super::*;

    #[test]
    fn native_token_request_rejects_browser_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(ORIGIN, "https://easyquran.fyi".parse().unwrap());

        let result = ensure_native_token_request(&headers);

        assert!(result.is_err(), "browser-origin token login must fail");
    }

    #[test]
    fn native_token_request_accepts_missing_origin() {
        let result = ensure_native_token_request(&HeaderMap::new());

        assert!(result.is_ok(), "native requests omit Origin");
    }
}
