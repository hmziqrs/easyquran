//! Provider sign-in (PARITY I4), per `contracts/devices.md` "Sign-in matrix".
//!
//! - Desktop (macOS, Linux, Windows): Google through the system browser with
//!   PKCE and a `127.0.0.1` loopback redirect, then
//!   `POST /auth/google/v1/token` with the id token. Built only when the
//!   app is compiled with `EQ_GOOGLE_DESKTOP_CLIENT_ID` (a Google "Desktop
//!   app" OAuth client listed in the API's `GOOGLE_DESKTOP_CLIENT_IDS`;
//!   `EQ_GOOGLE_DESKTOP_CLIENT_SECRET` too, which Google requires for that
//!   client type and does not treat as secret).
//! - GitHub, Discord on desktop: the matrix holds them until the
//!   providers' desktop rules are checked; not offered.
//! - iOS, Android: providers need their SDKs, and NSH 0.5.1 has no bridge
//!   to them; not offered.

use eq_core::api::{
    SignInStep,
    types::{OAuthProvider, ProviderToken},
};

/// The providers this build can sign in with, in the web's order.
pub fn providers() -> Vec<OAuthProvider> {
    if desktop::available() {
        return vec![OAuthProvider::Google];
    }
    Vec::new()
}

/// The provider's display name (`OAuthButtons` labels).
pub fn label(provider: OAuthProvider) -> &'static str {
    match provider {
        OAuthProvider::Google => "Google",
        OAuthProvider::Apple => "Apple",
        OAuthProvider::Facebook => "Facebook",
        OAuthProvider::Github => "GitHub",
        OAuthProvider::Discord => "Discord",
    }
}

/// Runs `provider`'s sign-in to the end. `Err(())`: it failed or was
/// cancelled (`lastErrorCode`; the buttons show `auth_oauth_error`).
pub async fn sign_in(provider: OAuthProvider) -> Result<SignInStep, ()> {
    let token = match provider {
        OAuthProvider::Google => desktop::google().await?,
        _ => return Err(()),
    };
    finish(provider, token).await
}

async fn finish(provider: OAuthProvider, token: ProviderToken) -> Result<SignInStep, ()> {
    let store = crate::app::auth_store();
    let client = store.client().ok_or(())?;
    let result = client.oauth_token(provider, &token).await;
    store
        .complete_sign_in(result, eq_core::api::TransitionKind::OAuth)
        .await
        .map_err(|_| ())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
mod desktop {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        time::{Duration, Instant},
    };

    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use eq_core::{
        api::{
            Secret,
            types::{OAuthProvider, ProviderToken},
        },
        net::{Method, Request, Transport},
    };
    use sha2::{Digest, Sha256};

    const CLIENT_ID: Option<&str> = option_env!("EQ_GOOGLE_DESKTOP_CLIENT_ID");
    const CLIENT_SECRET: Option<&str> = option_env!("EQ_GOOGLE_DESKTOP_CLIENT_SECRET");
    const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
    const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
    /// How long the browser round trip may take before the attempt ends.
    const WAIT: Duration = Duration::from_secs(300);

    pub fn available() -> bool {
        CLIENT_ID.is_some_and(|id| !id.trim().is_empty())
    }

    fn random_b64(bytes: usize) -> Result<String, ()> {
        let mut buf = vec![0_u8; bytes];
        getrandom::getrandom(&mut buf).map_err(|_| ())?;
        Ok(URL_SAFE_NO_PAD.encode(buf))
    }

    /// Percent-encodes everything but RFC 3986 unreserved characters.
    fn encode(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for b in value.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(char::from(b));
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    fn decode(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let hex = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            match (bytes[i], hex) {
                (b'%', Some(v)) => {
                    out.push(v);
                    i += 3;
                }
                (b'+', _) => {
                    out.push(b' ');
                    i += 1;
                }
                (b, _) => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn query_value(query: &str, key: &str) -> Option<String> {
        query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key).then(|| decode(v))
        })
    }

    fn form(pairs: &[(&str, &str)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Reads one request's target (`/?code=…`), answers it, and returns its
    /// query when it carries `code` or `error`.
    fn answer(mut stream: TcpStream) -> Option<String> {
        stream.set_nonblocking(false).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
        let mut buf = [0_u8; 8192];
        let n = stream.read(&mut buf).ok()?;
        let head = String::from_utf8_lossy(buf.get(..n)?).into_owned();
        let target = head.lines().next()?.split(' ').nth(1)?.to_owned();
        let query = target.split_once('?').map(|(_, q)| q.to_owned());
        let relevant = query
            .as_deref()
            .is_some_and(|q| q.contains("code=") || q.contains("error="));
        // A browser page, not the app's UI: the platform shows it in English.
        let body = "<!doctype html><meta charset=utf-8><title>EasyQuran</title>\
                    <p>You can close this tab and return to EasyQuran.</p>";
        let status = if relevant { "200 OK" } else { "404 Not Found" };
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if relevant { query } else { None }
    }

    /// Waits for the browser's redirect on `listener`, up to [`WAIT`].
    fn wait_for_redirect(listener: TcpListener) -> Option<String> {
        listener.set_nonblocking(true).ok()?;
        let start = Instant::now();
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Some(query) = answer(stream) {
                        return Some(query);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() > WAIT {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(_) => return None,
            }
        }
    }

    pub async fn google() -> Result<ProviderToken, ()> {
        let client_id = CLIENT_ID.ok_or(())?.trim().to_owned();
        let api = crate::app::auth_store().client().ok_or(())?;
        // The id token must carry the API's one-time nonce.
        let nonce = api
            .oauth_token_nonce(OAuthProvider::Google)
            .await
            .map_err(|_| ())?;
        let verifier = random_b64(32)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_b64(16)?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| ())?;
        let port = listener.local_addr().map_err(|_| ())?.port();
        let redirect = format!("http://127.0.0.1:{port}");
        let url = format!(
            "{AUTH_URL}?{}",
            form(&[
                ("client_id", &client_id),
                ("redirect_uri", &redirect),
                ("response_type", "code"),
                ("scope", "openid email profile"),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("state", &state),
                ("nonce", &nonce.provider_nonce),
                ("prompt", "select_account"),
            ])
        );
        if !nsh_core::prelude::open_url(url).await {
            return Err(());
        }
        let query = nsh_core::prelude::blocking(move || wait_for_redirect(listener))
            .await
            .map_err(|_| ())?
            .ok_or(())?;
        if query_value(&query, "state").as_deref() != Some(state.as_str()) {
            return Err(());
        }
        let code = query_value(&query, "code").ok_or(())?;
        let mut pairs = vec![
            ("code", code.as_str()),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ];
        if let Some(secret) = CLIENT_SECRET {
            pairs.push(("client_secret", secret));
        }
        let mut request = Request::new(Method::Post, TOKEN_URL)
            .header("content-type", "application/x-www-form-urlencoded");
        request.body = form(&pairs).into_bytes();
        let response = crate::services::transport::NshTransport
            .send(request)
            .await
            .map_err(|_| ())?;
        if !response.ok() {
            return Err(());
        }
        let body: serde_json::Value = response.json().map_err(|_| ())?;
        let id_token = body
            .get("id_token")
            .and_then(serde_json::Value::as_str)
            .ok_or(())?;
        let access_token = body
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .map(Secret::from);
        Ok(ProviderToken::Google {
            id_token: Secret::from(id_token),
            access_token,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod desktop {
    use eq_core::api::types::ProviderToken;

    pub fn available() -> bool {
        false
    }

    pub async fn google() -> Result<ProviderToken, ()> {
        Err(())
    }
}
