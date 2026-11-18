//! Native session transport (contracts/devices.md D6) over
//! [`crate::net::Transport`] and [`crate::kv::SecretStore`]: the one
//! [`ApiClient`] every API call goes through.
//!
//! - Every request carries `x-eq-session` (empty when there is no session
//!   yet). The client never sends `Cookie`, `Origin` or any `Sec-Fetch-*`
//!   header: the server refuses `x-eq-session` next to the last two.
//! - After every response that has `x-eq-session`, the newest value is kept
//!   in the secret store (empty clears it); a response without the header
//!   keeps the old value.
//! - Unsafe methods carry the CSRF token in `csrf-token`, fetched from
//!   `POST /csrf/v1/generate`. The token derives from the session id, so it
//!   is fetched again after `x-eq-session-rotated: 1`, after a sign-in, and
//!   once after a CSRF refusal (then the call is retried once).
//! - A 401 that means the session is gone clears the stored value and the
//!   token and tells every [`ApiClient::on_session_lost`] listener. Nothing
//!   here touches sync rows: dirty rows wait for the next sign-in.
//!
//! The session value and the CSRF token are [`Secret`]s and never reach a
//! log, a trace or an error.

use std::{cell::RefCell, rc::Rc};

use serde_json::Value;

use super::{ApiError, ApiFuture, HttpError, Secret, config::ApiConfig};
use crate::{
    kv::SecretStore,
    net::{Method, NetError, NetFuture, Request, Transport},
    sync::transport::{SyncGet, SyncPost},
};

/// Request and response header carrying the session value.
pub const SESSION_HEADER: &str = "x-eq-session";
/// Response header: the session id was cycled (sign-in).
pub const ROTATED_HEADER: &str = "x-eq-session-rotated";
/// Request header the CSRF guard reads.
pub const CSRF_HEADER: &str = "csrf-token";
/// Secret-store key of the session value.
pub const SESSION_KEY: &str = "eq.session";
/// The CSRF token route.
pub const CSRF_PATH: &str = "/csrf/v1/generate";
/// The server refuses longer values (`native_session::MAX_TOKEN_BYTES`).
pub const MAX_SESSION_BYTES: usize = 4096;

/// Routes the CSRF guard skips (`static_csrf::is_csrf_exempt`), which the
/// client calls without fetching a token first.
const CSRF_EXEMPT: &[&str] = &[
    CSRF_PATH,
    "/auth/google/v1/token",
    "/auth/google/v1/token/nonce",
    "/auth/apple/v1/token",
    "/auth/apple/v1/token/nonce",
    "/auth/facebook/v1/token",
    "/auth/github/v1/token",
    "/auth/discord/v1/token",
];

/// Whether `value` is a session value the server accepts: base64 text plus
/// `+ / = - _ . %`, at most 4096 bytes.
pub fn valid_session_value(value: &str) -> bool {
    value.len() <= MAX_SESSION_BYTES
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_' | b'.' | b'%')
        })
}

/// A 2xx reply.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiReply {
    pub status: u16,
    /// `x-eq-session-rotated: 1` was set.
    pub rotated: bool,
    /// The JSON body; `Null` when empty or not JSON.
    pub body: Value,
}

type Listener = Rc<dyn Fn()>;

/// The API client: D6 session transport, CSRF and JSON calls.
pub struct ApiClient {
    config: ApiConfig,
    transport: Rc<dyn Transport>,
    secrets: Rc<dyn SecretStore>,
    csrf: RefCell<Option<Secret>>,
    lost: RefCell<Vec<Listener>>,
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("config", &self.config)
            .field("has_session", &self.has_session())
            .field("has_csrf", &self.csrf.borrow().is_some())
            .finish_non_exhaustive()
    }
}

impl ApiClient {
    /// A client for `config` over `transport`, keeping the session value
    /// in `secrets`.
    pub fn new(
        config: ApiConfig,
        transport: Rc<dyn Transport>,
        secrets: Rc<dyn SecretStore>,
    ) -> Self {
        Self {
            config,
            transport,
            secrets,
            csrf: RefCell::new(None),
            lost: RefCell::new(Vec::new()),
        }
    }

    /// Where calls go.
    pub fn config(&self) -> &ApiConfig {
        &self.config
    }

    /// The secret store the session value lives in (shared with device keys).
    pub fn secrets(&self) -> &Rc<dyn SecretStore> {
        &self.secrets
    }

    /// Whether a session value is stored.
    pub fn has_session(&self) -> bool {
        self.session().is_some()
    }

    fn session(&self) -> Option<Secret> {
        self.secrets
            .get(SESSION_KEY)
            .filter(|v| !v.is_empty())
            .map(Secret::from)
    }

    fn keep_session(&self, value: &str) {
        if value.is_empty() {
            self.secrets.remove(SESSION_KEY);
            return;
        }
        if !valid_session_value(value) {
            // Kept out: the server would refuse it. The next call 401s and
            // the user signs in again, as with a lost rotation.
            return;
        }
        if self.secrets.get(SESSION_KEY).as_deref() != Some(value) {
            // A failed write leaves the old value; the next call 401s.
            let _ = self.secrets.set(SESSION_KEY, value);
        }
    }

    /// Forgets the session value and the CSRF token (sign-out, account
    /// deletion, a lost session).
    pub fn clear_session(&self) {
        self.secrets.remove(SESSION_KEY);
        self.clear_csrf();
    }

    /// Forgets the CSRF token; the next unsafe call fetches a new one.
    pub fn clear_csrf(&self) {
        *self.csrf.borrow_mut() = None;
    }

    /// Whether a CSRF token is held.
    pub fn has_csrf(&self) -> bool {
        self.csrf.borrow().is_some()
    }

    /// Calls `listener` whenever a call finds the session gone (401). The
    /// stored value is already cleared when it runs.
    pub fn on_session_lost(&self, listener: impl Fn() + 'static) {
        self.lost.borrow_mut().push(Rc::new(listener));
    }

    fn session_lost(&self) {
        self.clear_session();
        let listeners: Vec<Listener> = self.lost.borrow().clone();
        for listener in listeners {
            listener();
        }
    }

    /// `POST /csrf/v1/generate` with the session; the token is kept.
    pub async fn refresh_csrf(&self) -> Result<(), ApiError> {
        self.clear_csrf();
        let reply = self.send(Method::Post, CSRF_PATH, None, false).await?;
        let token = reply
            .body
            .get("token")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ApiError::Decode("csrf generate returned no token".into()))?;
        *self.csrf.borrow_mut() = Some(Secret::new(token));
        Ok(())
    }

    async fn ensure_csrf(&self) -> Result<(), ApiError> {
        if self.has_csrf() {
            return Ok(());
        }
        self.refresh_csrf().await
    }

    /// `GET path`.
    pub async fn get(&self, path: &str) -> Result<ApiReply, ApiError> {
        self.call(Method::Get, path, None).await
    }

    /// `POST path` with a JSON `body`.
    pub async fn post(&self, path: &str, body: Value) -> Result<ApiReply, ApiError> {
        self.call(Method::Post, path, Some(body)).await
    }

    /// `POST path` with no body.
    pub async fn post_empty(&self, path: &str) -> Result<ApiReply, ApiError> {
        self.call(Method::Post, path, None).await
    }

    /// `DELETE path`.
    pub async fn delete(&self, path: &str) -> Result<ApiReply, ApiError> {
        self.call(Method::Delete, path, None).await
    }

    /// One call: CSRF first for an unsafe method on a guarded route, then
    /// the request, with one retry after a CSRF refusal.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<ApiReply, ApiError> {
        let unsafe_method = method != Method::Get;
        if unsafe_method && !CSRF_EXEMPT.contains(&path) {
            self.ensure_csrf().await?;
        }
        match self.send(method, path, body.as_ref(), true).await {
            Err(ApiError::Http(e)) if e.is_csrf_refusal() && unsafe_method => {
                self.refresh_csrf().await?;
                self.send(method, path, body.as_ref(), false).await
            }
            other => other,
        }
    }

    /// Sends one request and applies the D6 response rules. `may_retry`:
    /// a CSRF refusal is handed back for [`ApiClient::call`] to retry
    /// instead of being treated as final.
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        may_retry: bool,
    ) -> Result<ApiReply, ApiError> {
        let session = self.session();
        let sent_session = session.is_some();
        let mut request = Request::new(method, self.config.url(path))
            .header("accept", "application/json")
            .header(
                SESSION_HEADER,
                session.as_ref().map(Secret::expose).unwrap_or_default(),
            );
        if method != Method::Get
            && let Some(token) = self.csrf.borrow().as_ref()
        {
            request = request.header(CSRF_HEADER, token.expose());
        }
        if let Some(body) = body {
            request.body = serde_json::to_vec(body).map_err(|e| ApiError::Decode(e.to_string()))?;
            request = request.header("content-type", "application/json");
        }

        let response = self.transport.send(request).await?;
        if let Some(value) = response.header(SESSION_HEADER) {
            self.keep_session(value);
        }
        let rotated = response.header(ROTATED_HEADER) == Some("1");
        let parsed: Value = if response.body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&response.body).unwrap_or(Value::Null)
        };

        if !response.ok() {
            let retry_after = response
                .header("retry-after")
                .and_then(|v| v.trim().parse::<u64>().ok());
            let error = HttpError::decode(response.status, &parsed, retry_after);
            if error.is_csrf_refusal() {
                self.clear_csrf();
                if may_retry {
                    return Err(ApiError::Http(error));
                }
            }
            if error.is_session_lost() && sent_session {
                self.session_lost();
            }
            return Err(ApiError::Http(error));
        }

        if rotated && path != CSRF_PATH {
            // A failed refetch leaves no token; the next unsafe call fetches.
            // Boxed: `refresh_csrf` sends through here too.
            let refresh: ApiFuture<'_, ()> = Box::pin(self.refresh_csrf());
            if refresh.await.is_err() {
                self.clear_csrf();
            }
        }
        Ok(ApiReply {
            status: response.status,
            rotated,
            body: parsed,
        })
    }
}

/// The sync seams' error: transport failures pass through, a non-2xx reply
/// becomes [`NetError::Http`] (status, `Retry-After`, server `message`). The
/// D6 rules (CSRF retry, rotation, 401 → session lost) already ran.
fn to_net_error(error: ApiError) -> NetError {
    match error {
        ApiError::Net(net) => net,
        ApiError::Http(http) => NetError::http(http.status, http.retry_after, http.message),
        ApiError::Decode(message) => NetError::Decode(message),
    }
}

impl SyncPost for ApiClient {
    fn post_json(&self, path: &str, body: Value) -> NetFuture<'_, Value> {
        let path = path.to_owned();
        Box::pin(async move {
            self.post(&path, body)
                .await
                .map(|reply| reply.body)
                .map_err(to_net_error)
        })
    }
}

impl SyncGet for ApiClient {
    fn get_json(&self, path: &str) -> NetFuture<'_, Value> {
        let path = path.to_owned();
        Box::pin(async move {
            self.get(&path)
                .await
                .map(|reply| reply.body)
                .map_err(to_net_error)
        })
    }
}

impl SyncPost for Rc<ApiClient> {
    fn post_json(&self, path: &str, body: Value) -> NetFuture<'_, Value> {
        self.as_ref().post_json(path, body)
    }
}

impl SyncGet for Rc<ApiClient> {
    fn get_json(&self, path: &str) -> NetFuture<'_, Value> {
        self.as_ref().get_json(path)
    }
}
