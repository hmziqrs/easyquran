use std::sync::LazyLock;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use oauth2::{AuthorizationCode, CsrfToken, PkceCodeChallenge, Scope, TokenResponse};
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
    service::{discord_api_base, get_discord_oauth_client, load_discord_credentials},
    validator::{
        DiscordCallbackQuery, DiscordExchangeRequest, DiscordTokenRequest, DiscordUserInfo,
    },
};

const DISCORD_API_MAX_BYTES: usize = 64 * 1024;
const DISCORD_USER_AGENT: &str = "EasyQuran";
static DISCORD_API_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    discord_api_http_client_builder()
        .build()
        .map_err(|error| error.to_string())
});

fn discord_api_http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
}

struct DiscordLoginResult {
    user: crate::db::sea_models::user::Model,
    provider_name: String,
    provider_email: Option<String>,
    provider_picture: Option<String>,
}

#[debug_handler]
#[instrument(skip(_state, session), fields(result))]
pub async fn discord_login(
    State(_state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating Discord OAuth login");

    let client = get_discord_oauth_client()?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let session_id = oauth::oauth_session_id(&session)?;
    oauth::store_oauth_state(
        &session_id,
        csrf_token.secret(),
        pkce_verifier.secret(),
        None,
    )?;

    info!("Generated Discord auth URL with PKCE + session-bound CSRF state");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(auth_url.as_str()))
}

#[debug_handler]
#[instrument(skip(state, auth, query), fields(user_id, result))]
pub async fn discord_callback(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedQuery(query): ValidatedQuery<DiscordCallbackQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Discord OAuth callback");

    // Provider cancellation/error (?error=access_denied, …) — never attempt an exchange.
    if query.is_error() {
        tracing::Span::current().record("result", "cancelled");
        let url = oauth::redirect::build_failure_redirect(
            oauth::OAuthProvider::Discord,
            oauth::redirect::FAILURE_CANCELLED,
        )?;
        return Ok(Redirect::temporary(&url));
    }

    match run_discord_callback(&state, &mut auth, query).await {
        Ok(user) => {
            tracing::Span::current().record("user_id", user.id);
            info!(user_id = user.id, "Discord login successful");
            tracing::Span::current().record("result", "success");
            let url = oauth::redirect::build_success_redirect(oauth::OAuthProvider::Discord)?;
            Ok(Redirect::temporary(&url))
        }
        Err(err) => {
            warn!(code = %err.code, "Discord callback failed; redirecting to opaque failure path");
            tracing::Span::current().record("result", "error");
            let url = oauth::redirect::build_failure_redirect(
                oauth::OAuthProvider::Discord,
                oauth::redirect::error_to_failure_code(&err),
            )?;
            Ok(Redirect::temporary(&url))
        }
    }
}

/// Inner callback body surfaced to the caller, which redirects failures to the opaque failure path.
async fn run_discord_callback(
    state: &AppState,
    auth: &mut AuthSession,
    query: DiscordCallbackQuery,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &query.state()?)?;

    let client = get_discord_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(query.code()?));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(oauth::token_exchange_http_client()?)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange Discord authorization code");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_discord_user_info(access_token).await?;
    let result = finish_discord_login(state, auth, user_info).await?;
    Ok(result.user)
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn discord_exchange(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<DiscordExchangeRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Discord OAuth code exchange from client");

    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &payload.state)?;

    let client = get_discord_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(payload.code));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(oauth::token_exchange_http_client()?)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange Discord authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_discord_user_info(access_token).await?;
    let result = finish_discord_login(&state, &mut auth, user_info).await?;
    let user = result.user;

    info!(
        user_id = user.id,
        "Discord login successful via client exchange"
    );
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with Discord"
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, payload, headers), fields(user_id, result))]
pub async fn discord_token(
    State(state): State<AppState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<DiscordTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Discord mobile token sign-in");

    oauth::ensure_native_token_request(&headers)?;

    let credentials = load_discord_credentials()?;
    let expected_user_id =
        verify_discord_access_token(&payload.access_token, &credentials.client_id).await?;
    let user_info = fetch_discord_user_info(&payload.access_token).await?;
    let user_info = reconcile_discord_user_info(expected_user_id, user_info)?;
    let result = finish_discord_login(&state, &mut auth, user_info).await?;
    let provider_profile = json!({
        "name": &result.provider_name,
        "email": &result.provider_email,
        "picture": &result.provider_picture,
    });
    let user = result.user;

    info!(user_id = user.id, "Discord mobile login successful");
    tracing::Span::current().record("user_id", user.id);
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "providerProfile": provider_profile,
            "message": "Successfully authenticated with Discord"
        })),
    ))
}

#[debug_handler(state = AppState)]
pub async fn discord_user_info(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => Ok((StatusCode::OK, Json(json!(user)))),
        None => Err(ErrorResponse::new(ErrorCode::Unauthorized)),
    }
}

fn discord_api_http_client() -> Result<&'static reqwest::Client, ErrorResponse> {
    DISCORD_API_HTTP_CLIENT.as_ref().map_err(|error| {
        error!(error = %error, "Failed to build Discord API HTTP client");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to contact Discord")
    })
}

fn discord_api_request(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header(reqwest::header::ACCEPT, "application/json")
        // Discord API expects a descriptive User-Agent.
        .header(reqwest::header::USER_AGENT, DISCORD_USER_AGENT)
}

#[derive(Debug, Deserialize)]
struct DiscordTokenInspection {
    application: DiscordTokenApp,
    user: DiscordTokenOwner,
}

#[derive(Debug, Deserialize)]
struct DiscordTokenApp {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DiscordTokenOwner {
    id: String,
}

async fn verify_discord_access_token(
    access_token: &str,
    expected_client_id: &str,
) -> Result<String, ErrorResponse> {
    let http_client = discord_api_http_client()?;
    verify_discord_access_token_from_url(
        http_client,
        &format!("{}/oauth2/@me", discord_api_base()),
        access_token,
        expected_client_id,
    )
    .await
}

async fn verify_discord_access_token_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
    expected_client_id: &str,
) -> Result<String, ErrorResponse> {
    let response = discord_api_request(http_client.get(url).bearer_auth(access_token))
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to inspect Discord access token");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to verify Discord access token")
        })?;
    let inspection: DiscordTokenInspection =
        decode_discord_api_response(response, &[StatusCode::UNAUTHORIZED]).await?;
    validate_discord_token_inspection(inspection, expected_client_id)
}

fn validate_discord_token_inspection(
    inspection: DiscordTokenInspection,
    expected_client_id: &str,
) -> Result<String, ErrorResponse> {
    let user_id = inspection.user.id.trim().to_string();
    if inspection.application.id != expected_client_id || user_id.is_empty() {
        warn!("Discord token inspection returned a foreign app or invalid user");
        return Err(invalid_discord_token());
    }
    Ok(user_id)
}

fn reconcile_discord_user_info(
    expected_user_id: String,
    user_info: DiscordUserInfo,
) -> Result<DiscordUserInfo, ErrorResponse> {
    if expected_user_id.is_empty()
        || user_info.id != expected_user_id
        || user_info.username.trim().is_empty()
    {
        warn!("Discord token/profile identity mismatch");
        return Err(invalid_discord_token());
    }
    Ok(user_info)
}

async fn fetch_discord_user_info(access_token: &str) -> Result<DiscordUserInfo, ErrorResponse> {
    let http_client = discord_api_http_client()?;
    fetch_discord_user_info_from_url(
        http_client,
        &format!("{}/users/@me", discord_api_base()),
        access_token,
    )
    .await
}

async fn fetch_discord_user_info_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<DiscordUserInfo, ErrorResponse> {
    let response = discord_api_request(http_client.get(url).bearer_auth(access_token))
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from Discord");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from Discord")
        })?;
    decode_discord_api_response(response, &[StatusCode::UNAUTHORIZED]).await
}

async fn decode_discord_api_response<T: DeserializeOwned>(
    mut response: reqwest::Response,
    invalid_token_statuses: &[StatusCode],
) -> Result<T, ErrorResponse> {
    let status = response.status();
    if invalid_token_statuses.contains(&status) {
        warn!(status = %status, "Discord rejected access token");
        return Err(invalid_discord_token());
    }
    if !status.is_success() {
        error!(status = %status, "Discord API endpoint returned non-2xx");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to contact Discord"));
    }
    if response
        .content_length()
        .is_some_and(|length| length > DISCORD_API_MAX_BYTES as u64)
    {
        error!("Discord API content length exceeded size limit");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse Discord response"));
    }

    let mut body = Vec::with_capacity(DISCORD_API_MAX_BYTES.min(4096));
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        error!(error = ?e, "Failed to read Discord API response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to read Discord response")
    })? {
        if body.len().saturating_add(chunk.len()) > DISCORD_API_MAX_BYTES {
            error!("Discord API response exceeded size limit");
            return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse Discord response"));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body).map_err(|e| {
        error!(error = ?e, "Failed to parse Discord API response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse Discord response")
    })
}

fn invalid_discord_token() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidToken)
        .with_message("Discord access token is invalid or expired")
}

/// CDN URL for the profile avatar. Discord returns only the hash; the CDN path needs id+hash.
fn discord_avatar_url(id: &str, avatar: Option<&str>) -> Option<String> {
    let id = id.trim();
    let avatar = avatar.map(str::trim).filter(|value| !value.is_empty())?;
    if id.is_empty() {
        return None;
    }
    Some(format!(
        "https://cdn.discordapp.com/avatars/{id}/{avatar}.png?size=256"
    ))
}

async fn finish_discord_login(
    state: &AppState,
    auth: &mut AuthSession,
    user_info: DiscordUserInfo,
) -> Result<DiscordLoginResult, ErrorResponse> {
    let email = user_info
        .email
        .clone()
        .filter(|value| !value.trim().is_empty());
    let email_verified = email.is_some() && user_info.verified.unwrap_or(false);
    if email.is_none() {
        warn!("Discord returned no email; only an existing identity link can sign in");
    }

    let name = user_info
        .global_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| user_info.username.clone());
    let provider_user_id = user_info.id.clone();

    let user = oauth::find_or_create_user_for_oauth(
        &state.sea_db,
        oauth::OAuthProvider::Discord,
        &provider_user_id,
        email.clone(),
        name.clone(),
        email_verified,
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Discord).await?;
    Ok(DiscordLoginResult {
        user,
        provider_name: name,
        provider_email: email,
        provider_picture: discord_avatar_url(&user_info.id, user_info.avatar.as_deref()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Each #[tokio::test] owns a short-lived runtime. The shared static client would park
    // idle connections whose dispatch tasks die with that runtime, so a later request that
    // reuses one fails instantly (hyper DispatchGone). Build a fresh client per test.
    fn test_http_client() -> reqwest::Client {
        discord_api_http_client_builder().build().unwrap()
    }

    fn valid_token_inspection() -> DiscordTokenInspection {
        DiscordTokenInspection {
            application: DiscordTokenApp {
                id: "configured-discord-app".to_string(),
            },
            user: DiscordTokenOwner {
                id: "80351110224678912".to_string(),
            },
        }
    }

    #[test]
    fn token_inspection_rejects_token_from_different_app() {
        let result =
            validate_discord_token_inspection(valid_token_inspection(), "other-discord-app");

        assert!(result.is_err(), "foreign-app token must fail");
    }

    #[test]
    fn token_inspection_rejects_empty_user_id() {
        let mut inspection = valid_token_inspection();
        inspection.user.id = "  ".to_string();

        let result = validate_discord_token_inspection(inspection, "configured-discord-app");

        assert!(result.is_err(), "empty user id must fail");
    }

    #[test]
    fn token_inspection_accepts_matching_app_and_user() {
        let result =
            validate_discord_token_inspection(valid_token_inspection(), "configured-discord-app")
                .unwrap();

        assert_eq!(result, "80351110224678912");
    }

    #[test]
    fn reconcile_rejects_profile_mismatch() {
        let profile: DiscordUserInfo = serde_json::from_value(json!({
            "id": "111",
            "username": "aisha"
        }))
        .unwrap();

        assert!(reconcile_discord_user_info("222".to_string(), profile).is_err());
    }

    #[test]
    fn avatar_url_requires_hash_and_id() {
        assert_eq!(
            discord_avatar_url("123", Some("abc")),
            Some("https://cdn.discordapp.com/avatars/123/abc.png?size=256".to_string())
        );
        assert_eq!(discord_avatar_url("123", None), None);
        assert_eq!(discord_avatar_url("123", Some("  ")), None);
        assert_eq!(discord_avatar_url("  ", Some("abc")), None);
    }

    #[tokio::test]
    async fn fetches_profile_with_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/@me"))
            .and(header("authorization", "Bearer discord-user-token"))
            .and(header("user-agent", DISCORD_USER_AGENT))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "80351110224678912",
                "username": "sandbox",
                "global_name": "Sandbox User",
                "email": "sandbox@example.com",
                "verified": true,
                "avatar": "hash123"
            })))
            .mount(&server)
            .await;

        let info = fetch_discord_user_info_from_url(
            &test_http_client(),
            &format!("{}/users/@me", server.uri()),
            "discord-user-token",
        )
        .await
        .unwrap();

        assert_eq!(info.id, "80351110224678912");
        assert_eq!(info.global_name.as_deref(), Some("Sandbox User"));
        assert_eq!(info.email.as_deref(), Some("sandbox@example.com"));
        assert_eq!(info.verified, Some(true));
    }

    #[tokio::test]
    async fn rejects_unauthorized_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/@me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result = fetch_discord_user_info_from_url(
            &test_http_client(),
            &format!("{}/users/@me", server.uri()),
            "expired",
        )
        .await;

        assert!(result.is_err(), "401 must map to an invalid-token error");
    }
}
