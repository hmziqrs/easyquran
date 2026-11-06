//! Login-only marker cookie `ruxlog.auth=1` (`Path=/`).
//!
//! The web edge decides "signed-in reader" from this cookie, never from `ruxlog.sid`:
//! production scopes `ruxlog.sid` to `/api`, so the web origin cannot see it anyway, and
//! analytics or bot cookies must not count as a session. The marker carries no secret and
//! grants nothing; it only selects a cache policy. A stale marker after expiry costs one
//! cache miss.
//!
//! Flow: `create_bound_session` stages a flag in the tower session. `emit_marker` runs on
//! the auth nests, reads the flag after the handler, writes `Set-Cookie` and removes the
//! flag. Logout writes the clearing cookie directly from its handler.

use axum::{
    extract::Request,
    http::{header::SET_COOKIE, HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use tower_sessions::Session;

use crate::state::AppState;

pub(crate) const AUTH_MARKER_COOKIE: &str = "ruxlog.auth";

/// Session-scoped flag staged by a completed login. Never sent to a client.
pub const STAGED_KEY: &str = "auth_marker_staged";

/// Matches the 14-day inactivity expiry of the session cookie.
const MAX_AGE_SECS: i64 = 60 * 60 * 24 * 14;

fn with_attributes(head: &str, secure: bool) -> HeaderValue {
    let mut value = format!("{head}; Path=/; SameSite=Lax; HttpOnly");
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("marker cookie is ascii")
}

pub(crate) fn set_cookie_value(secure: bool) -> HeaderValue {
    with_attributes(
        &format!("{AUTH_MARKER_COOKIE}=1; Max-Age={MAX_AGE_SECS}"),
        secure,
    )
}

pub(crate) fn clear_cookie_value(secure: bool) -> HeaderValue {
    with_attributes(&format!("{AUTH_MARKER_COOKIE}=; Max-Age=0"), secure)
}

/// Clearing `Set-Cookie` for logout. Attributes match the set form so the browser replaces it.
pub(crate) fn clear_headers(secure: bool) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.append(SET_COOKIE, clear_cookie_value(secure));
    headers
}

/// Stage the marker for the response of the handler that completed the login.
pub(crate) async fn stage_login(session: &Session) -> Result<(), tower_sessions::session::Error> {
    session.insert(STAGED_KEY, true).await
}

/// Auth-nest layer: writes the marker when a login was staged during this request.
pub async fn emit_marker(req: Request, next: Next) -> Response {
    let secure = req
        .extensions()
        .get::<AppState>()
        .is_none_or(|state| state.settings.http.cookie_secure);
    let session = req.extensions().get::<Session>().cloned();
    let mut response = next.run(req).await;
    let Some(session) = session else {
        return response;
    };
    // Read first: `remove` always dirties the session, and a dirty session is written to the
    // store. Only a request that actually staged a login may write.
    if matches!(session.get::<bool>(STAGED_KEY).await, Ok(Some(true))) {
        let _ = session.remove::<bool>(STAGED_KEY).await;
        response
            .headers_mut()
            .append(SET_COOKIE, set_cookie_value(secure));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_cookie_is_path_root_lax_and_secure_in_production() {
        let v = set_cookie_value(true);
        let s = v.to_str().unwrap();
        assert!(s.starts_with("ruxlog.auth=1;"), "{s}");
        assert!(s.contains("Path=/"), "{s}");
        assert!(s.contains("Max-Age=1209600"), "{s}");
        assert!(s.contains("SameSite=Lax"), "{s}");
        assert!(s.contains("; Secure"), "{s}");
    }

    #[test]
    fn set_cookie_omits_secure_for_plain_http_local_dev() {
        let s = set_cookie_value(false).to_str().unwrap().to_string();
        assert!(!s.contains("Secure"), "{s}");
    }

    #[test]
    fn clear_cookie_expires_immediately_with_matching_path() {
        let headers = clear_headers(true);
        let s = headers.get(SET_COOKIE).unwrap().to_str().unwrap();
        assert!(s.starts_with("ruxlog.auth=;"), "{s}");
        assert!(s.contains("Max-Age=0"), "{s}");
        assert!(s.contains("Path=/"), "{s}");
        assert!(s.contains("; Secure"), "{s}");
    }

    #[test]
    fn marker_never_carries_session_material() {
        // The marker is a presence bit; the session cookie name must not leak into it.
        assert!(!set_cookie_value(true)
            .to_str()
            .unwrap()
            .contains("ruxlog.sid"));
    }
}
