//! Native-app OAuth relays (contracts/devices.md, "Sign-in matrix").
//!
//! Some providers only redirect to HTTPS (Facebook, GitHub) or only POST the result to
//! an HTTPS `form_post` target (Apple). The app cannot receive either, so the provider
//! is pointed at a relay route here, which answers `302` to the app's private-use scheme
//! (`fyi.easyquran.app:/oauth/<provider>?…`) carrying the provider's own fields.
//!
//! Rules:
//! - The target is a compile-time constant. Nothing in the request chooses where the
//!   browser goes, so a relay is never an open redirect.
//! - Only the named fields pass through; any other parameter is dropped.
//! - The relay holds no session, consumes nothing and logs nothing: the code, id token
//!   and state are the app's to check (state and PKCE live in the app). It is a pure
//!   reflection to a fixed target, so it is CSRF-exempt.
//! - Desktop uses the same scheme: NSH's `open_auth_session` opens the system browser
//!   and the claimed scheme brings the link back, so no loopback port is ever taken
//!   from a request.

use axum::{
    extract::{Form, Query},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

/// The app's private-use scheme (its app id), claimed in `native/nsh.json` `schemes`.
pub const APP_SCHEME: &str = "fyi.easyquran.app";

/// The Apple relay's full path: CSRF-exempt and exempt from the origin guard, since
/// Apple's `form_post` is a cross-site POST from `https://appleid.apple.com`.
pub const APPLE_RELAY_PATH: &str = "/auth/apple/v1/native/relay";

/// A field longer than this is not a provider value; the relay answers `invalid_request`.
const MAX_FIELD_BYTES: usize = 16 * 1024;

/// The providers that have a relay, each with its fixed app path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayProvider {
    Apple,
    Facebook,
    Github,
}

impl RelayProvider {
    fn path(self) -> &'static str {
        match self {
            RelayProvider::Apple => "apple",
            RelayProvider::Facebook => "facebook",
            RelayProvider::Github => "github",
        }
    }
}

/// `fyi.easyquran.app:/oauth/<provider>?<fields>`: the present, non-empty `fields` in
/// order, form-encoded. An oversized field replaces everything with
/// `error=invalid_request`.
pub fn app_callback_url(
    provider: RelayProvider,
    fields: &[(&'static str, Option<&str>)],
) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    let oversized = fields
        .iter()
        .any(|(_, value)| value.is_some_and(|v| v.len() > MAX_FIELD_BYTES));
    if oversized {
        query.append_pair("error", "invalid_request");
    } else {
        for (key, value) in fields {
            if let Some(value) = value.filter(|v| !v.is_empty()) {
                query.append_pair(key, value);
            }
        }
    }
    format!("{APP_SCHEME}:/oauth/{}?{}", provider.path(), query.finish())
}

/// `302 Found` to the app, never cached and with no referrer.
pub fn relay_response(
    provider: RelayProvider,
    fields: &[(&'static str, Option<&str>)],
) -> Response {
    let target = app_callback_url(provider, fields);
    // Form encoding leaves only ASCII, so this cannot fail; fall back to a bare error
    // callback rather than a 500 that could echo anything.
    let location = HeaderValue::from_str(&target).unwrap_or_else(|_| {
        HeaderValue::from_static("fyi.easyquran.app:/oauth/error?error=invalid_request")
    });
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            ),
        ],
    )
        .into_response()
}

/// Apple's `form_post` body. Unknown fields are ignored (dropped).
#[derive(Debug, Default, Deserialize)]
pub struct AppleRelayForm {
    code: Option<String>,
    id_token: Option<String>,
    state: Option<String>,
    /// First sign-in only: `{"name":{…},"email":…}` as JSON text.
    user: Option<String>,
    error: Option<String>,
}

/// `POST /auth/apple/v1/native/relay` (Apple `response_mode=form_post`).
pub async fn apple_relay(Form(form): Form<AppleRelayForm>) -> Response {
    relay_response(
        RelayProvider::Apple,
        &[
            ("code", form.code.as_deref()),
            ("id_token", form.id_token.as_deref()),
            ("state", form.state.as_deref()),
            ("user", form.user.as_deref()),
            ("error", form.error.as_deref()),
        ],
    )
}

/// A query-redirect provider's callback. Unknown parameters are ignored (dropped).
#[derive(Debug, Default, Deserialize)]
pub struct RelayQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_reason: Option<String>,
    error_description: Option<String>,
}

/// `GET /auth/facebook/v1/native/relay`.
pub async fn facebook_relay(Query(query): Query<RelayQuery>) -> Response {
    relay_response(
        RelayProvider::Facebook,
        &[
            ("code", query.code.as_deref()),
            ("state", query.state.as_deref()),
            ("error", query.error.as_deref()),
            ("error_reason", query.error_reason.as_deref()),
            ("error_description", query.error_description.as_deref()),
        ],
    )
}

/// `GET /auth/github/v1/native/relay`.
pub async fn github_relay(Query(query): Query<RelayQuery>) -> Response {
    relay_response(
        RelayProvider::Github,
        &[
            ("code", query.code.as_deref()),
            ("state", query.state.as_deref()),
            ("error", query.error.as_deref()),
            ("error_description", query.error_description.as_deref()),
        ],
    )
}

/// The native redirect URIs a broker accepts: `raw` (comma-separated, exact strings)
/// when set, else the relay under the frontend origin (`{origin}/api{relay_path}`).
pub fn redirect_allow_list(
    raw: Option<&str>,
    origin: Option<&str>,
    relay_path: &str,
) -> Vec<String> {
    let configured: Vec<String> = raw
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect();
    if !configured.is_empty() {
        return configured;
    }
    origin
        .map(|o| format!("{}/api{relay_path}", o.trim_end_matches('/')))
        .into_iter()
        .collect()
}

/// Exact match only: no prefix, case or normalisation games.
pub fn redirect_uri_allowed(candidate: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|entry| entry == candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_is_the_fixed_app_scheme_and_encodes_fields() {
        let url = app_callback_url(
            RelayProvider::Apple,
            &[
                ("code", Some("c 1&x=y")),
                ("id_token", Some("a.b.c")),
                ("state", Some("s")),
                ("user", None),
            ],
        );
        assert_eq!(
            url,
            "fyi.easyquran.app:/oauth/apple?code=c+1%26x%3Dy&id_token=a.b.c&state=s"
        );
    }

    #[test]
    fn empty_fields_are_dropped_and_oversized_fields_become_an_error() {
        let url = app_callback_url(
            RelayProvider::Github,
            &[("code", Some("")), ("state", Some("s"))],
        );
        assert_eq!(url, "fyi.easyquran.app:/oauth/github?state=s");
        let big = "x".repeat(MAX_FIELD_BYTES + 1);
        let url = app_callback_url(
            RelayProvider::Github,
            &[("code", Some(&big)), ("state", Some("s"))],
        );
        assert_eq!(url, "fyi.easyquran.app:/oauth/github?error=invalid_request");
    }

    #[test]
    fn allow_list_prefers_config_then_derives_the_relay() {
        let derived = redirect_allow_list(
            None,
            Some("https://easyquran.fyi/"),
            "/auth/github/v1/native/relay",
        );
        assert_eq!(
            derived,
            vec!["https://easyquran.fyi/api/auth/github/v1/native/relay"]
        );
        let configured = redirect_allow_list(
            Some(" https://a.test/r , ,https://b.test/r"),
            Some("https://easyquran.fyi"),
            "/x",
        );
        assert_eq!(configured, vec!["https://a.test/r", "https://b.test/r"]);
        assert!(redirect_allow_list(None, None, "/x").is_empty());
    }

    async fn relay_location(
        request: axum::http::Request<axum::body::Body>,
    ) -> (StatusCode, String) {
        use tower::ServiceExt;
        let app = axum::Router::new()
            .route("/apple", axum::routing::post(apple_relay))
            .route("/facebook", axum::routing::get(facebook_relay))
            .route("/github", axum::routing::get(github_relay));
        let reply = app.oneshot(request).await.unwrap();
        let status = reply.status();
        assert_eq!(reply.headers()[header::CACHE_CONTROL], "no-store");
        let location = reply.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .to_owned();
        (status, location)
    }

    fn get(uri: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::get(uri)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn apple_relay_posts_through_to_the_app_and_drops_unknown_fields() {
        let request = axum::http::Request::post("/apple")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(axum::body::Body::from(
                "code=c1&id_token=h.p.s&state=st&user=%7B%22name%22%3A1%7D\
                 &redirect=https%3A%2F%2Fevil.test&target=x",
            ))
            .unwrap();
        let (status, location) = relay_location(request).await;
        assert_eq!(status, StatusCode::FOUND);
        assert_eq!(
            location,
            "fyi.easyquran.app:/oauth/apple?code=c1&id_token=h.p.s&state=st&user=%7B%22name%22%3A1%7D"
        );
    }

    #[tokio::test]
    async fn query_relays_go_only_to_the_fixed_target() {
        let (status, location) = relay_location(get(
            "/facebook?code=c&state=s&redirect_uri=https%3A%2F%2Fevil.test&next=%2F%2Fevil.test",
        ))
        .await;
        assert_eq!(status, StatusCode::FOUND);
        assert_eq!(location, "fyi.easyquran.app:/oauth/facebook?code=c&state=s");

        let (_, location) = relay_location(get(
            "/github?error=access_denied&error_description=no&error_uri=https%3A%2F%2Fx&state=s",
        ))
        .await;
        assert_eq!(
            location,
            "fyi.easyquran.app:/oauth/github?state=s&error=access_denied&error_description=no"
        );

        let (_, location) = relay_location(get("/github")).await;
        assert_eq!(location, "fyi.easyquran.app:/oauth/github?");
    }

    #[test]
    fn allow_list_match_is_exact() {
        let allowed = vec!["https://easyquran.fyi/api/auth/github/v1/native/relay".to_owned()];
        assert!(redirect_uri_allowed(
            "https://easyquran.fyi/api/auth/github/v1/native/relay",
            &allowed
        ));
        for bad in [
            "https://easyquran.fyi/api/auth/github/v1/native/relay/",
            "https://easyquran.fyi/api/auth/github/v1/native/relay?x=1",
            "https://EASYQURAN.fyi/api/auth/github/v1/native/relay",
            "https://evil.test/api/auth/github/v1/native/relay",
            "fyi.easyquran.app:/oauth/github",
            "",
        ] {
            assert!(!redirect_uri_allowed(bad, &allowed), "{bad}");
        }
    }
}
