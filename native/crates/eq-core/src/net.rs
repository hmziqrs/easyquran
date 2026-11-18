//! Transport seam (lead-owned): the request/response types every API call
//! uses and the [`Transport`] trait the app crate implements over NSH
//! `fetch` (`src/services/transport.rs`). Tests implement it with canned
//! responses. Futures are not `Send`: the NSH engine future is not.

use std::{future::Future, pin::Pin};

use serde::{Serialize, de::DeserializeOwned};

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    /// The wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
        }
    }
}

/// One outgoing request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// A request with no headers and no body.
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// `GET url`.
    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url)
    }

    /// `POST url` with `body` serialized as JSON and the JSON content type.
    pub fn post_json(url: impl Into<String>, body: &impl Serialize) -> Result<Self, NetError> {
        let bytes = serde_json::to_vec(body).map_err(|e| NetError::Decode(e.to_string()))?;
        Ok(Self {
            body: bytes,
            ..Self::new(Method::Post, url)
        }
        .header("content-type", "application/json"))
    }

    /// Adds a header (kept in order; duplicates allowed).
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// One response, any status. A non-2xx status is a response, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// The first header named `name`, compared case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// A 2xx status.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The body as UTF-8 text.
    pub fn text(&self) -> Result<&str, NetError> {
        std::str::from_utf8(&self.body).map_err(|e| NetError::Decode(e.to_string()))
    }

    /// The body decoded as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, NetError> {
        serde_json::from_slice(&self.body).map_err(|e| NetError::Decode(e.to_string()))
    }
}

/// Why a request produced no usable response.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// No connection (offline, DNS, timeout).
    #[error("offline: {0}")]
    Offline(String),
    /// The platform refused the request (capability not granted, bad URL).
    #[error("refused: {0}")]
    Refused(String),
    /// The bytes could not be encoded or decoded.
    #[error("decode: {0}")]
    Decode(String),
    /// A non-2xx reply, already decoded by the API client. A [`Transport`]
    /// never returns this (a non-2xx status is a [`Response`]); the session
    /// transport's JSON seams (`sync::transport::{SyncPost, SyncGet}`) do,
    /// so callers can act on the status table.
    #[error("http {}", .0.status)]
    Http(HttpFailure),
}

impl NetError {
    /// A non-2xx failure with `status`, `Retry-After` seconds and the
    /// server's `message` (debug builds; sync account-mismatch messages).
    pub fn http(status: u16, retry_after: Option<u64>, message: Option<String>) -> Self {
        NetError::Http(HttpFailure {
            status,
            retry_after,
            message,
        })
    }

    /// The HTTP failure, if this is one.
    pub fn as_http(&self) -> Option<&HttpFailure> {
        match self {
            NetError::Http(h) => Some(h),
            _ => None,
        }
    }
}

/// A non-2xx reply as the JSON seams report it. Never holds a session value
/// or CSRF token: the backend never echoes either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpFailure {
    pub status: u16,
    /// `Retry-After` (header or body), in seconds.
    pub retry_after: Option<u64>,
    /// The body's `message`, when sent.
    pub message: Option<String>,
}

/// A boxed, non-`Send` future of a transport call.
pub type NetFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, NetError>> + 'a>>;

/// Sends requests. One impl over NSH `fetch` in the app; fakes in tests.
pub trait Transport {
    fn send(&self, request: Request) -> NetFuture<'_, Response>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_match_case_insensitively() {
        let res = Response {
            status: 204,
            headers: vec![("X-Eq-Session".into(), "s".into())],
            body: Vec::new(),
        };
        assert_eq!(res.header("x-eq-session"), Some("s"));
        assert_eq!(res.header("missing"), None);
        assert!(res.ok());
    }

    #[test]
    fn post_json_sets_body_and_content_type() {
        let req = Request::post_json("https://easyquran.fyi/x", &serde_json::json!({"a": 1}))
            .unwrap()
            .header("x-csrf-token", "t");
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.body, br#"{"a":1}"#);
        assert_eq!(
            req.headers[0],
            ("content-type".into(), "application/json".into())
        );
        assert_eq!(req.headers.len(), 2);
    }
}
