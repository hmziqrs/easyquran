//! Provider sign-in (PARITY I4), per `contracts/devices.md` "Sign-in matrix".
//!
//! `nsh-auth` builds the authorize URL (PKCE `S256`, `state`), checks the
//! callback and exchanges the code with the provider; the provider token
//! then goes to `POST /auth/{provider}/v1/token`, whose reply runs through
//! the same sign-in machine as the email path (a `pending_login` ends in
//! `SignInStep::TotpRequired`). Provider tokens are held in memory for the
//! one exchange and never stored.
//!
//! How the page comes back, per platform:
//!
//! - Desktop (macOS, Linux, Windows): Google only (the matrix holds other
//!   providers until their desktop rules are checked). The system browser
//!   and a `127.0.0.1` loopback redirect (RFC 8252); `nsh-auth` has no
//!   desktop launcher of its own, so the loopback listener stays here.
//! - iOS, Android: NSH's `open_auth_session` (ASWebAuthenticationSession,
//!   Custom Tabs) with a private-use scheme redirect.
//!
//! A provider shows only when this build has its client id (`option_env!`,
//! listed in `native/README.md`), as the web hides disabled providers.
//! Blocked today (hidden): Apple (needs a native AuthenticationServices
//! bridge on iOS or an HTTPS `form_post` relay elsewhere), Facebook (HTTPS
//! redirects only), GitHub (the code exchange needs the client secret,
//! i.e. a broker). On iOS, Sign in with Apple must sit next to any other
//! social sign-in (O31), so iOS offers none until Apple lands.

use std::sync::Arc;

use eq_core::api::{
    Secret, SignInStep,
    types::{OAuthProvider, ProviderToken},
};
use nsh_auth::{
    Auth, AuthConfig, AuthError, BoxFuture, Http, HttpReply, MemoryStore, ProviderId, Session,
    SystemClock,
};
use nsh_core::element::{HttpRequest, create_root, fetch};
use nsh_core::prelude::spawn;

/// The scheme of the app's OAuth redirects: the app id, which is also what
/// Google's Android clients accept (`<package>:/oauth2redirect`). Claimed in
/// `nsh.json` `schemes` so Android and desktop route it back.
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
const APP_SCHEME: &str = "fyi.easyquran.app";

/// The web's provider order (`OAuthButtons`).
const ORDER: [OAuthProvider; 5] = [
    OAuthProvider::Google,
    OAuthProvider::Apple,
    OAuthProvider::Facebook,
    OAuthProvider::Github,
    OAuthProvider::Discord,
];

/// The providers this build can sign in with, in the web's order.
pub fn providers() -> Vec<OAuthProvider> {
    let social: Vec<OAuthProvider> = ORDER.into_iter().filter(|p| setup(*p).is_some()).collect();
    // O31: on iOS, no other social sign-in without Sign in with Apple.
    if cfg!(target_os = "ios") && !social.contains(&OAuthProvider::Apple) {
        return Vec::new();
    }
    social
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
    let setup = setup(provider).ok_or(())?;
    let store = crate::app::auth_store();
    let client = store.client().ok_or(())?;
    // Google and Apple id tokens must carry the API's one-time nonce.
    let nonce = match provider.has_token_nonce() {
        true => Some(
            client
                .oauth_token_nonce(provider)
                .await
                .map_err(|_| ())?
                .provider_nonce,
        ),
        false => None,
    };
    let session = run(&setup, nonce.as_deref()).await?;
    let token = provider_token(provider, session)?;
    let result = client.oauth_token(provider, &token).await;
    store
        .complete_sign_in(result, eq_core::api::TransitionKind::OAuth)
        .await
        .map_err(|_| ())
}

/// How the provider's redirect reaches the app.
enum Redirect {
    /// `http://127.0.0.1:<port>`, a listener bound per attempt.
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "linux", target_os = "windows")),
        allow(dead_code)
    )]
    Loopback,
    /// `<scheme>:<path>` through `open_auth_session`.
    #[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
    Scheme { scheme: String, path: &'static str },
}

/// One provider's build-time configuration on this platform.
struct Setup {
    id: ProviderId,
    client_id: &'static str,
    redirect: Redirect,
    /// Google's installed-app client secret (desktop clients only; Google
    /// does not treat it as secret, but its token endpoint wants it).
    installed_secret: Option<&'static str>,
}

fn env_id(value: Option<&'static str>) -> Option<&'static str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

fn setup(provider: OAuthProvider) -> Option<Setup> {
    match provider {
        OAuthProvider::Google => google(),
        OAuthProvider::Discord => discord(),
        OAuthProvider::Apple | OAuthProvider::Facebook | OAuthProvider::Github => None,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn google() -> Option<Setup> {
    Some(Setup {
        id: ProviderId::Google,
        client_id: env_id(option_env!("EQ_GOOGLE_DESKTOP_CLIENT_ID"))?,
        redirect: Redirect::Loopback,
        installed_secret: env_id(option_env!("EQ_GOOGLE_DESKTOP_CLIENT_SECRET")),
    })
}

/// Google iOS clients redirect to their reversed client id
/// (`com.googleusercontent.apps.<prefix>:/oauth2redirect`).
#[cfg(target_os = "ios")]
fn google() -> Option<Setup> {
    let client_id = env_id(option_env!("EQ_GOOGLE_IOS_CLIENT_ID"))?;
    let prefix = client_id.strip_suffix(".apps.googleusercontent.com")?;
    Some(Setup {
        id: ProviderId::Google,
        client_id,
        redirect: Redirect::Scheme {
            scheme: format!("com.googleusercontent.apps.{prefix}"),
            path: "/oauth2redirect",
        },
        installed_secret: None,
    })
}

/// Google Android clients (with "Custom URI scheme" enabled) redirect to
/// the package name.
#[cfg(target_os = "android")]
fn google() -> Option<Setup> {
    Some(Setup {
        id: ProviderId::Google,
        client_id: env_id(option_env!("EQ_GOOGLE_ANDROID_CLIENT_ID"))?,
        redirect: Redirect::Scheme {
            scheme: APP_SCHEME.to_owned(),
            path: "/oauth2redirect",
        },
        installed_secret: None,
    })
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows",
    target_os = "ios",
    target_os = "android"
)))]
fn google() -> Option<Setup> {
    None
}

/// Discord on phones: a public client with PKCE.
#[cfg(any(target_os = "ios", target_os = "android"))]
fn discord() -> Option<Setup> {
    Some(Setup {
        id: ProviderId::Discord,
        client_id: env_id(option_env!("EQ_DISCORD_CLIENT_ID"))?,
        redirect: Redirect::Scheme {
            scheme: APP_SCHEME.to_owned(),
            path: "/oauth/discord",
        },
        installed_secret: None,
    })
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn discord() -> Option<Setup> {
    None
}

/// The provider half: authorize in the browser, check the callback,
/// exchange the code.
async fn run(setup: &Setup, api_nonce: Option<&str>) -> Result<Session, ()> {
    let (redirect_uri, listener) = match &setup.redirect {
        Redirect::Loopback => {
            let (uri, listener) = loopback::bind()?;
            (uri, Some(listener))
        }
        Redirect::Scheme { scheme, path } => (format!("{scheme}:{path}"), None),
    };
    let mut config = AuthConfig::for_provider(setup.id)
        .client_id(setup.client_id)
        .redirect_uri(redirect_uri.as_str());
    // The API reads the account itself and verifies the id token: no
    // userinfo, JWKS or revocation calls from here.
    config.userinfo_url = None;
    config.jwks_url = None;
    config.revoke_url = None;
    // The API's nonce goes into the request instead of the library's own.
    if api_nonce.is_some() {
        config.use_nonce = false;
    }
    let http = NshHttp {
        installed_secret: setup.installed_secret,
    };
    let (_owner, auth) = create_root(|| {
        Auth::new(
            config,
            Arc::new(MemoryStore::new()),
            Arc::new(SystemClock),
            Arc::new(http),
        )
    });
    let request = auth.authorize_request(setup.id).map_err(|_| ())?;
    let url = authorize_url(setup.id, &request.url, api_nonce)?;
    let callback = match (&setup.redirect, listener) {
        (Redirect::Scheme { scheme, .. }, _) => scheme_session(url, scheme).await?,
        (Redirect::Loopback, Some(listener)) => {
            loopback::wait(url, listener, &redirect_uri).await?
        }
        (Redirect::Loopback, None) => return Err(()),
    };
    auth.complete(setup.id, &callback).await.map_err(|_| ())
}

/// The library's authorize URL with the API's nonce, and for Google the
/// account chooser instead of a forced consent screen (no refresh token
/// is needed: the provider token is spent at once).
fn authorize_url(id: ProviderId, url: &str, api_nonce: Option<&str>) -> Result<String, ()> {
    let mut parsed = url::Url::parse(url).map_err(|_| ())?;
    let drop: &[&str] = match id {
        ProviderId::Google => &["access_type", "prompt"],
        _ => &[],
    };
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !drop.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    {
        let mut pairs = parsed.query_pairs_mut();
        pairs.clear();
        for (k, v) in &kept {
            pairs.append_pair(k, v);
        }
        if id == ProviderId::Google {
            pairs.append_pair("prompt", "select_account");
        }
        if let Some(nonce) = api_nonce {
            pairs.append_pair("nonce", nonce);
        }
    }
    Ok(parsed.to_string())
}

#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
async fn scheme_session(url: String, scheme: &str) -> Result<String, ()> {
    use nsh_core::auth_session::{AuthSessionOutcome, AuthSessionRequest};
    match nsh_core::element::open_auth_session(AuthSessionRequest::new(url, scheme)).await {
        Ok(AuthSessionOutcome::Completed(callback)) => Ok(callback),
        _ => Err(()),
    }
}

/// What `/auth/{provider}/v1/token` takes from the provider session.
fn provider_token(provider: OAuthProvider, session: Session) -> Result<ProviderToken, ()> {
    match provider {
        OAuthProvider::Google => Ok(ProviderToken::Google {
            id_token: Secret::from(session.id_token.ok_or(())?.as_str()),
            access_token: Some(Secret::from(session.access_token.as_str())),
        }),
        OAuthProvider::Apple => Ok(ProviderToken::Apple {
            identity_token: Secret::from(session.id_token.ok_or(())?.as_str()),
        }),
        OAuthProvider::Facebook | OAuthProvider::Github | OAuthProvider::Discord => Ok(
            ProviderToken::Access(Secret::from(session.access_token.as_str())),
        ),
    }
}

/// `nsh-auth`'s HTTP seam over the engine's `fetch`, so provider calls run
/// under the `network` capability like every other request.
struct NshHttp {
    installed_secret: Option<&'static str>,
}

fn send(
    mut request: HttpRequest,
    bearer: Option<&str>,
) -> BoxFuture<'static, Result<HttpReply, AuthError>> {
    request = request.header("accept", "application/json");
    if let Some(token) = bearer {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    // The engine's `Fetch` is not `Send` and the library's futures must be:
    // the fetch runs as an engine task (this is called on the engine
    // thread) and hands its reply over a `Send` channel.
    let (tx, rx) = futures_channel::oneshot::channel();
    spawn(async move {
        let reply = fetch(request)
            .await
            .map(|response| HttpReply {
                status: response.status(),
                body: String::from_utf8_lossy(&response.body).into_owned(),
            })
            .map_err(AuthError::Network);
        let _ = tx.send(reply);
    });
    Box::pin(async move {
        rx.await
            .unwrap_or_else(|_| Err(AuthError::Network("request dropped".to_owned())))
    })
}

impl Http for NshHttp {
    fn post_form<'a>(
        &'a self,
        url: &'a str,
        form: &'a [(&'a str, String)],
        bearer: Option<&'a str>,
    ) -> BoxFuture<'a, Result<HttpReply, AuthError>> {
        let mut body = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in form {
            body.append_pair(key, value);
        }
        if let Some(secret) = self.installed_secret {
            body.append_pair("client_secret", secret);
        }
        let request = HttpRequest::new("POST", url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body.finish().into_bytes());
        send(request, bearer)
    }

    fn get_json<'a>(
        &'a self,
        url: &'a str,
        bearer: Option<&'a str>,
    ) -> BoxFuture<'a, Result<HttpReply, AuthError>> {
        send(HttpRequest::new("GET", url), bearer)
    }

    fn post_json<'a>(
        &'a self,
        url: &'a str,
        body: &'a serde_json::Value,
        bearer: Option<&'a str>,
    ) -> BoxFuture<'a, Result<HttpReply, AuthError>> {
        let request = HttpRequest::new("POST", url)
            .header("content-type", "application/json")
            .body(body.to_string().into_bytes());
        send(request, bearer)
    }

    fn delete<'a>(
        &'a self,
        url: &'a str,
        bearer: &'a str,
    ) -> BoxFuture<'a, Result<HttpReply, AuthError>> {
        send(HttpRequest::new("DELETE", url), Some(bearer))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
mod loopback {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        time::{Duration, Instant},
    };

    /// How long the browser round trip may take before the attempt ends.
    const WAIT: Duration = Duration::from_secs(300);

    /// A listener on an OS-assigned `127.0.0.1` port, and its redirect URI.
    pub fn bind() -> Result<(String, TcpListener), ()> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| ())?;
        let port = listener.local_addr().map_err(|_| ())?.port();
        Ok((format!("http://127.0.0.1:{port}"), listener))
    }

    /// Opens `url` in the system browser and returns the callback URL the
    /// browser was redirected to.
    pub async fn wait(url: String, listener: TcpListener, redirect: &str) -> Result<String, ()> {
        if !nsh_core::prelude::open_url(url).await {
            return Err(());
        }
        let query = nsh_core::prelude::blocking(move || wait_for_redirect(listener))
            .await
            .map_err(|_| ())?
            .ok_or(())?;
        Ok(format!("{redirect}/?{query}"))
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
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod loopback {
    pub fn bind() -> Result<(String, std::net::TcpListener), ()> {
        Err(())
    }

    pub async fn wait(
        _url: String,
        _listener: std::net::TcpListener,
        _redirect: &str,
    ) -> Result<String, ()> {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_url_carries_the_api_nonce_and_no_forced_consent() {
        let url = "https://accounts.google.com/o/oauth2/v2/auth?client_id=a&state=s\
                   &access_type=offline&prompt=consent";
        let out = authorize_url(ProviderId::Google, url, Some("n1")).unwrap();
        let parsed = url::Url::parse(&out).unwrap();
        let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("nonce".into(), "n1".into())));
        assert!(pairs.contains(&("prompt".into(), "select_account".into())));
        assert!(pairs.contains(&("state".into(), "s".into())));
        assert!(!pairs.iter().any(|(k, _)| k == "access_type"));
        assert_eq!(pairs.iter().filter(|(k, _)| k == "nonce").count(), 1);
    }
}
