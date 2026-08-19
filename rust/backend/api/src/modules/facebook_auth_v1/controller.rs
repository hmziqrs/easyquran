use std::sync::LazyLock;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use oauth2::{AuthorizationCode, CsrfToken, Scope, TokenResponse};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use tower_sessions::Session;
use tracing::{error, info, instrument, warn};

use crate::{
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    extractors::ValidatedQuery,
    modules::auth_v1::controller::session_rotated_headers,
    services::{auth::AuthSession, oauth},
    AppState,
};

use super::{
    service::{facebook_graph_url, get_facebook_oauth_client, load_facebook_credentials},
    validator::{
        FacebookCallbackQuery, FacebookExchangeRequest, FacebookTokenRequest, FacebookUserInfo,
    },
};

const FACEBOOK_GRAPH_MAX_BYTES: usize = 64 * 1024;
static FACEBOOK_GRAPH_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> =
    LazyLock::new(|| {
        facebook_graph_http_client_builder()
            .build()
            .map_err(|error| error.to_string())
    });

fn facebook_graph_http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
}

#[derive(Debug, Deserialize)]
struct FacebookDebugTokenResponse {
    data: FacebookDebugTokenData,
}

#[derive(Debug, Deserialize)]
struct FacebookDebugTokenData {
    app_id: String,
    is_valid: bool,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(rename = "type", default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_at: Option<i64>,
    #[serde(default)]
    data_access_expires_at: Option<i64>,
}

#[debug_handler]
#[instrument(skip(_state, session), fields(result))]
pub async fn facebook_login(
    State(_state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating Facebook OAuth login");

    let client = get_facebook_oauth_client()?;
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("public_profile".to_string()))
        .url();

    // Empty PKCE verifier: Facebook has no PKCE; "" round-trips as `None` at consume time.
    let session_id = oauth::oauth_session_id(&session)?;
    oauth::store_oauth_state(&session_id, csrf_token.secret(), "", None)?;

    info!("Generated Facebook auth URL with session-bound CSRF state");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(auth_url.as_str()))
}

#[debug_handler]
#[instrument(skip(state, auth, query), fields(user_id, result))]
pub async fn facebook_callback(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedQuery(query): ValidatedQuery<FacebookCallbackQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Facebook OAuth callback");

    // Provider cancellation/error (?error_reason=user_denied, ?error_code=…, …) — no exchange.
    if query.is_error() {
        tracing::Span::current().record("result", "cancelled");
        let url = oauth::redirect::build_failure_redirect(
            oauth::OAuthProvider::Facebook,
            oauth::redirect::FAILURE_CANCELLED,
        )?;
        return Ok(Redirect::temporary(&url));
    }

    match run_facebook_callback(&state, &mut auth, query).await {
        Ok(user) => {
            tracing::Span::current().record("user_id", user.id);
            info!(user_id = user.id, "Facebook login successful");
            tracing::Span::current().record("result", "success");
            let url = oauth::redirect::build_success_redirect(oauth::OAuthProvider::Facebook)?;
            Ok(Redirect::temporary(&url))
        }
        Err(err) => {
            warn!(code = %err.code, "Facebook callback failed; redirecting to opaque failure path");
            tracing::Span::current().record("result", "error");
            let url = oauth::redirect::build_failure_redirect(
                oauth::OAuthProvider::Facebook,
                oauth::redirect::error_to_failure_code(&err),
            )?;
            Ok(Redirect::temporary(&url))
        }
    }
}

/// Inner callback body surfaced to the caller, which redirects failures to the opaque failure path.
async fn run_facebook_callback(
    state: &AppState,
    auth: &mut AuthSession,
    query: FacebookCallbackQuery,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let session_id = oauth::oauth_session_id(auth.session())?;
    let _oauth_state = oauth::consume_oauth_state(&session_id, &query.state()?)?;

    let client = get_facebook_oauth_client()?;
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code()?))
        .request_async(oauth::token_exchange_http_client()?)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange Facebook authorization code");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_facebook_user_info(access_token).await?;
    finish_facebook_login(state, auth, user_info).await
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn facebook_exchange(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<FacebookExchangeRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Facebook OAuth code exchange from client");

    let session_id = oauth::oauth_session_id(auth.session())?;
    let _oauth_state = oauth::consume_oauth_state(&session_id, &payload.state)?;

    let client = get_facebook_oauth_client()?;
    let token_result = client
        .exchange_code(AuthorizationCode::new(payload.code))
        .request_async(oauth::token_exchange_http_client()?)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange Facebook authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_facebook_user_info(access_token).await?;
    let user = finish_facebook_login(&state, &mut auth, user_info).await?;

    info!(
        user_id = user.id,
        "Facebook login successful via client exchange"
    );
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with Facebook"
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, payload, headers), fields(user_id, result))]
pub async fn facebook_token(
    State(state): State<AppState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<FacebookTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Facebook mobile token sign-in");

    oauth::ensure_native_token_request(&headers)?;

    let credentials = load_facebook_credentials()?;
    let expected_user_id = verify_facebook_access_token(
        &payload.access_token,
        &credentials.client_id,
        &credentials.client_secret,
    )
    .await?;
    let user_info = fetch_facebook_user_info(&payload.access_token).await?;
    let user_info = reconcile_facebook_user_info(&expected_user_id, user_info)?;
    let provider_profile = json!({
        "name": &user_info.name,
        "email": &user_info.email,
        "picture": user_info.picture.as_ref().map(|picture| &picture.data.url),
    });
    let user = finish_facebook_login(&state, &mut auth, user_info).await?;

    info!(user_id = user.id, "Facebook mobile login successful");
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "providerProfile": provider_profile,
            "message": "Successfully authenticated with Facebook"
        })),
    ))
}

#[debug_handler(state = AppState)]
pub async fn facebook_user_info(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => Ok((StatusCode::OK, Json(json!(user)))),
        None => Err(ErrorResponse::new(ErrorCode::Unauthorized)),
    }
}

fn facebook_graph_http_client() -> Result<&'static reqwest::Client, ErrorResponse> {
    FACEBOOK_GRAPH_HTTP_CLIENT.as_ref().map_err(|error| {
        error!(error = %error, "Failed to build Facebook Graph HTTP client");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to contact Facebook")
    })
}

async fn verify_facebook_access_token(
    access_token: &str,
    expected_app_id: &str,
    app_secret: &str,
) -> Result<String, ErrorResponse> {
    let http_client = facebook_graph_http_client()?;
    verify_facebook_access_token_from_url(
        http_client,
        &facebook_graph_url("debug_token"),
        access_token,
        expected_app_id,
        app_secret,
    )
    .await
}

async fn verify_facebook_access_token_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
    expected_app_id: &str,
    app_secret: &str,
) -> Result<String, ErrorResponse> {
    let app_access_token = format!("{expected_app_id}|{app_secret}");
    let response = http_client
        .post(url)
        .form(&[("input_token", access_token)])
        .bearer_auth(app_access_token)
        .send()
        .await
        .map_err(|_| {
            // Keep request details out of logs because this call carries app credentials and user token.
            error!("Failed to inspect Facebook access token");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to verify Facebook access token")
        })?;
    let response: FacebookDebugTokenResponse = decode_facebook_graph_response(response).await?;
    validate_facebook_debug_token(response.data, expected_app_id)
}

fn validate_facebook_debug_token(
    data: FacebookDebugTokenData,
    expected_app_id: &str,
) -> Result<String, ErrorResponse> {
    let now = chrono::Utc::now().timestamp();
    let expired = data
        .expires_at
        .is_some_and(|value| value > 0 && value <= now);
    let data_access_expired = data
        .data_access_expires_at
        .is_some_and(|value| value > 0 && value <= now);
    let wrong_type = data
        .token_type
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case("USER"));
    let user_id = data
        .user_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(invalid_facebook_token)?;

    if !data.is_valid
        || data.app_id != expected_app_id
        || wrong_type
        || expired
        || data_access_expired
    {
        warn!("Facebook access token failed app, type, validity, or expiry checks");
        return Err(invalid_facebook_token());
    }

    Ok(user_id)
}

fn invalid_facebook_token() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidToken)
        .with_message("Facebook access token is invalid or expired")
}

async fn fetch_facebook_user_info(access_token: &str) -> Result<FacebookUserInfo, ErrorResponse> {
    let http_client = facebook_graph_http_client()?;
    fetch_facebook_user_info_from_url(http_client, &facebook_graph_url("me"), access_token).await
}

async fn fetch_facebook_user_info_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<FacebookUserInfo, ErrorResponse> {
    let response = http_client
        .get(url)
        .query(&[("fields", "id,name,email,picture.type(large)")])
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from Facebook");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from Facebook")
        })?;
    decode_facebook_graph_response(response).await
}

async fn decode_facebook_graph_response<T: DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, ErrorResponse> {
    let status = response.status();
    if matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        warn!(status = %status, "Facebook rejected access token");
        return Err(invalid_facebook_token());
    }
    if !status.is_success() {
        error!(status = %status, "Facebook Graph endpoint returned non-2xx");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to contact Facebook"));
    }
    if response
        .content_length()
        .is_some_and(|length| length > FACEBOOK_GRAPH_MAX_BYTES as u64)
    {
        error!("Facebook Graph content length exceeded size limit");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse Facebook response"));
    }

    let mut body = Vec::with_capacity(FACEBOOK_GRAPH_MAX_BYTES.min(4096));
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        error!(error = ?e, "Failed to read Facebook Graph response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to read Facebook response")
    })? {
        if body.len().saturating_add(chunk.len()) > FACEBOOK_GRAPH_MAX_BYTES {
            error!("Facebook Graph response exceeded size limit");
            return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse Facebook response"));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body).map_err(|e| {
        error!(error = ?e, "Failed to parse Facebook Graph response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse Facebook response")
    })
}

fn reconcile_facebook_user_info(
    expected_user_id: &str,
    user_info: FacebookUserInfo,
) -> Result<FacebookUserInfo, ErrorResponse> {
    if expected_user_id.trim().is_empty()
        || user_info.id.trim().is_empty()
        || user_info.id != expected_user_id
    {
        warn!("Facebook token/profile identity mismatch");
        return Err(invalid_facebook_token());
    }
    Ok(user_info)
}

async fn finish_facebook_login(
    state: &AppState,
    auth: &mut AuthSession,
    user_info: FacebookUserInfo,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let email = user_info
        .email
        .clone()
        .filter(|value| !value.trim().is_empty());

    let name = user_info
        .name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Facebook User".to_string());
    let user = oauth::find_or_create_user_for_oauth(
        &state.sea_db,
        oauth::OAuthProvider::Facebook,
        &user_info.id,
        email,
        name,
        // email_verified: the Graph API /me response carries no verified flag, so a
        // Facebook email must stay unverified — find_or_create_user_for_oauth then
        // refuses to link it onto (or create an account from) an existing email.
        false,
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Facebook).await?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::auth_v1::controller::{session_rotated_headers, SESSION_ROTATED};
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Each #[tokio::test] owns a short-lived runtime. The shared static client would park
    // idle connections whose dispatch tasks die with that runtime, so a later request that
    // reuses one fails instantly (hyper DispatchGone). Build a fresh client per test.
    fn test_http_client() -> reqwest::Client {
        facebook_graph_http_client_builder().build().unwrap()
    }

    fn valid_debug_token() -> FacebookDebugTokenData {
        FacebookDebugTokenData {
            app_id: "configured-facebook-app".to_string(),
            is_valid: true,
            user_id: Some("facebook-user-123".to_string()),
            token_type: Some("USER".to_string()),
            expires_at: None,
            data_access_expires_at: None,
        }
    }

    #[test]
    fn debug_token_rejects_token_from_different_app() {
        let result = validate_facebook_debug_token(valid_debug_token(), "other-facebook-app");

        assert!(result.is_err(), "foreign-app token must fail");
    }

    #[test]
    fn profile_rejects_different_user_than_debug_token() {
        let profile = FacebookUserInfo {
            id: "different-facebook-user".to_string(),
            email: Some("aisha@example.com".to_string()),
            name: Some("Aisha Khan".to_string()),
            picture: None,
        };

        let result = reconcile_facebook_user_info("facebook-user-123", profile);

        assert!(
            result.is_err(),
            "profile/debug-token identity mismatch must fail"
        );
    }

    #[tokio::test]
    async fn debug_token_request_binds_token_to_configured_app() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/debug_token"))
            .and(body_string_contains("input_token=facebook-user-token"))
            .and(header(
                "authorization",
                "Bearer configured-facebook-app|facebook-app-secret",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "app_id": "configured-facebook-app",
                    "is_valid": true,
                    "user_id": "facebook-user-123",
                    "type": "USER"
                }
            })))
            .mount(&server)
            .await;
        let client = test_http_client();

        let user_id = verify_facebook_access_token_from_url(
            &client,
            &format!("{}/debug_token", server.uri()),
            "facebook-user-token",
            "configured-facebook-app",
            "facebook-app-secret",
        )
        .await
        .unwrap();

        assert_eq!(user_id, "facebook-user-123");
    }

    #[tokio::test]
    async fn userinfo_request_decodes_verified_profile_picture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .and(header("authorization", "Bearer facebook-user-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "facebook-user-123",
                "email": "aisha@example.com",
                "name": "Aisha Khan",
                "picture": {"data": {"url": "https://profiles.facebook.test/aisha.jpg"}}
            })))
            .mount(&server)
            .await;
        let client = test_http_client();

        let profile = fetch_facebook_user_info_from_url(
            &client,
            &format!("{}/me", server.uri()),
            "facebook-user-token",
        )
        .await
        .unwrap();

        assert_eq!(
            profile.picture.map(|picture| picture.data.url).as_deref(),
            Some("https://profiles.facebook.test/aisha.jpg")
        );
    }

    #[tokio::test]
    async fn userinfo_unauthorized_response_is_invalid_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = test_http_client();

        let error = fetch_facebook_user_info_from_url(
            &client,
            &format!("{}/me", server.uri()),
            "expired-facebook-token",
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidToken);
    }

    #[tokio::test]
    async fn graph_redirect_is_not_followed() {
        let target = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/leak-target"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&target)
            .await;
        let source = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("location", format!("{}/leak-target", target.uri())),
            )
            .mount(&source)
            .await;
        let client = test_http_client();

        let error = fetch_facebook_user_info_from_url(
            &client,
            &format!("{}/me", source.uri()),
            "must-not-cross-redirect",
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::ExternalServiceError);
    }

    #[test]
    fn token_login_rotation_header_is_present() {
        let headers = session_rotated_headers(true);

        assert_eq!(headers.get(SESSION_ROTATED).unwrap(), "1");
    }
}
