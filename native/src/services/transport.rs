//! `eq_core::net::Transport` over NSH `fetch` (PARITY I1, B4): the one
//! network path for API calls, under the `network` capability hosts.
//!
//! What the engines add (NSH 0.5.1 source): nothing of `Origin`,
//! `Sec-Fetch-*` or `Cookie` on desktop (`nsh-engine/src/net.rs`, ureq 3
//! agents without the `cookies` feature) or Android (`Net.java`,
//! `HttpURLConnection` with no `CookieHandler`). Apple platforms use an
//! `NSURLSession` with the default configuration
//! (`nsh-platform-{macos,ios}/src/net.rs` `UrlSessionNetwork::new`), whose
//! shared cookie storage would attach cookies it holds for the host; the
//! API never sends `Set-Cookie` to a native request and replaces any
//! `Cookie` with the `x-eq-session` value, so the session is unaffected.
//! As a guard this transport drops any such header before sending.
//!
//! The session header is added by `eq_core::api`; nothing here logs a
//! request or response.

use eq_core::net::{NetError, NetFuture, Request, Response, Transport};
use nsh_core::element::{HttpRequest, fetch};

/// Headers a native request must never carry (devices.md D6).
fn forbidden(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "cookie" || name == "origin" || name.starts_with("sec-fetch-")
}

/// [`Transport`] over the engine's `fetch`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NshTransport;

impl Transport for NshTransport {
    fn send(&self, request: Request) -> NetFuture<'_, Response> {
        let mut http = HttpRequest::new(request.method.as_str(), request.url);
        for (name, value) in request.headers {
            if !forbidden(&name) {
                http = http.header(name, value);
            }
        }
        if !request.body.is_empty() {
            http = http.body(request.body);
        }
        Box::pin(async move {
            let response = fetch(http).await.map_err(|reason| {
                // `Fetch` merges a capability refusal into its message.
                if reason.contains("capabilit") || reason.contains("not granted") {
                    return NetError::Refused(reason);
                }
                NetError::Offline(reason)
            })?;
            Ok(Response {
                status: response.status(),
                headers: response.head.headers,
                body: response.body,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::forbidden;

    #[test]
    fn browser_headers_are_dropped() {
        assert!(forbidden("Cookie") && forbidden("origin") && forbidden("Sec-Fetch-Mode"));
        assert!(!forbidden("x-eq-session") && !forbidden("csrf-token"));
    }
}
