//! Native session transport (devices.md D6).
//!
//! Native clients keep no cookie jar. They send the session value in the
//! `x-eq-session` request header and read a rotated value from the same header on
//! the response. This adapter runs outside the session layer, so every session,
//! CSRF, rotation and revocation rule applies unchanged.
//!
//! Browsers can never use the header: a request that carries it together with an
//! `Origin` or any `Sec-Fetch-*` header is refused, and the header is not in the
//! CORS allow list, so no cross-origin page can send it either.

use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tower_sessions::{Expiry, Session};

use crate::error::CorsError;

pub const SESSION_HEADER: HeaderName = HeaderName::from_static("x-eq-session");
const SESSION_COOKIE: &str = "ruxlog.sid";
const SESSION_COOKIE_PREFIX: &str = "ruxlog.sid=";
const MAX_TOKEN_BYTES: usize = 4096;

/// Idle lifetime of a native session registered with a verified device key.
/// Web sessions keep the 14-day layer default.
pub const NATIVE_IDLE: time::Duration = time::Duration::days(180);

/// Session data key holding the unix second of the last expiry refresh. Its
/// presence marks a native session that was registered with a device key.
pub const NATIVE_MARKER: &str = "eq_native_refreshed_at";

/// The expiry is pushed forward at most once a day, so a native session does
/// not cost a store write on every request.
const REFRESH_EVERY_SECS: i64 = 86_400;

/// Request extension set by `transport` on every native request.
#[derive(Debug, Clone, Copy)]
pub struct NativeClient;

fn browser_shaped(headers: &HeaderMap) -> bool {
    headers.contains_key(header::ORIGIN)
        || headers.contains_key("sec-fetch-site")
        || headers.contains_key("sec-fetch-mode")
        || headers.contains_key("sec-fetch-dest")
}

/// Visible token bytes that cannot break out of a Cookie value.
fn valid_token(value: &str) -> bool {
    value.len() <= MAX_TOKEN_BYTES
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_' | b'.' | b'%')
        })
}

/// Outer transport adapter. Request: `x-eq-session` becomes the only Cookie
/// (any client Cookie header is replaced, never appended). Response: the
/// `ruxlog.sid` Set-Cookie moves to `x-eq-session` (empty means cleared), and
/// every Set-Cookie is removed.
pub async fn transport(mut req: Request, next: Next) -> Result<Response, CorsError> {
    let Some(raw) = req.headers().get(&SESSION_HEADER).cloned() else {
        return Ok(next.run(req).await);
    };
    if browser_shaped(req.headers()) {
        return Err(CorsError::NativeSessionOnBrowser);
    }
    let token = match raw.to_str() {
        Ok(value) if valid_token(value) => value.to_owned(),
        _ => return Err(CorsError::InvalidNativeSession),
    };

    let headers = req.headers_mut();
    headers.remove(&SESSION_HEADER);
    headers.remove(header::COOKIE);
    if !token.is_empty() {
        let cookie = HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}"))
            .map_err(|_| CorsError::InvalidNativeSession)?;
        headers.insert(header::COOKIE, cookie);
    }
    req.extensions_mut().insert(NativeClient);

    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    let rotated = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| value.strip_prefix(SESSION_COOKIE_PREFIX))
        .map(|rest| rest.split(';').next().unwrap_or("").to_owned());
    headers.remove(header::SET_COOKIE);
    if let Some(value) = rotated {
        // The value came from the session layer's own cookie: base64 text, so it
        // is a valid header value. An unparsable one is dropped, which the client
        // reads as a lost rotation and signs in again.
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(SESSION_HEADER, value);
        }
    }
    Ok(res)
}

/// Inner expiry guard, inside the session layer. A native session registered
/// with a device key keeps a 180-day idle lifetime; the layer default would
/// otherwise shorten it to 14 days on its next save.
pub async fn keep_native_alive(session: Session, req: Request, next: Next) -> Response {
    if req.extensions().get::<NativeClient>().is_some() {
        refresh_native_expiry(&session).await;
    }
    next.run(req).await
}

async fn refresh_native_expiry(session: &Session) {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let Ok(Some(last)) = session.get::<i64>(NATIVE_MARKER).await else {
        return;
    };
    if now - last < REFRESH_EVERY_SECS {
        return;
    }
    session.set_expiry(Some(Expiry::OnInactivity(NATIVE_IDLE)));
    if let Err(err) = session.insert(NATIVE_MARKER, now).await {
        tracing::warn!(error = %err, "native session marker refresh failed");
    }
}

/// Called by device registration once a verified key is stored on a native
/// request. Marks the session and sets its 180-day idle lifetime.
pub async fn mark_native_session(session: &Session) -> Result<(), tower_sessions::session::Error> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    session.set_expiry(Some(Expiry::OnInactivity(NATIVE_IDLE)));
    session.insert(NATIVE_MARKER, now).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_charset_blocks_cookie_injection() {
        assert!(valid_token(""));
        assert!(valid_token("AbC+/=_-.%09"));
        assert!(!valid_token("a;b=c"));
        assert!(!valid_token("a,b"));
        assert!(!valid_token("a b"));
        assert!(!valid_token("a\"b"));
        assert!(!valid_token("a\r\nSet-Cookie: x=y"));
        assert!(!valid_token(&"a".repeat(MAX_TOKEN_BYTES + 1)));
    }

    #[test]
    fn browser_shaped_detects_origin_and_fetch_metadata() {
        let mut h = HeaderMap::new();
        assert!(!browser_shaped(&h));
        h.insert(header::ORIGIN, HeaderValue::from_static("https://x.test"));
        assert!(browser_shaped(&h));
        let mut h = HeaderMap::new();
        h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(browser_shaped(&h));
    }
}
