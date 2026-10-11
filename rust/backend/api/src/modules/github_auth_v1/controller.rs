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
    services::oauth::login::{link_identity_to_unchanged_user, CallbackFailure, CallbackOutcome},
    services::{auth::AuthSession, oauth},
    AppState,
};

use super::{
    service::{get_github_oauth_client, github_api_base, load_github_credentials},
    validator::{
        GitHubCallbackQuery, GitHubEmail, GitHubExchangeRequest, GitHubTokenRequest, GitHubUserInfo,
    },
};

const GITHUB_API_VERSION: &str = "2026-03-10";
const GITHUB_API_MAX_BYTES: usize = 64 * 1024;
const GITHUB_USER_AGENT: &str = "EasyQuran";
static GITHUB_API_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    github_api_http_client_builder()
        .build()
        .map_err(|error| error.to_string())
});

fn github_api_http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
}

#[derive(Debug, Deserialize)]
struct GitHubTokenInspection {
    app: GitHubTokenApp,
    user: GitHubTokenOwner,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenApp {
    client_id: String,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenOwner {
    id: i64,
}

struct GitHubLoginResult {
    user: crate::db::sea_models::user::Model,
    provider_name: String,
    provider_email: Option<String>,
    provider_picture: Option<String>,
}

#[debug_handler]
#[instrument(skip(_state, session), fields(result))]
pub async fn github_login(
    State(_state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating GitHub OAuth login");

    let client = get_github_oauth_client()?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user:email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let session_id = oauth::oauth_session_id(&session)?;
    oauth::store_oauth_state(
        &session_id,
        csrf_token.secret(),
        pkce_verifier.secret(),
        None,
        None,
    )?;

    info!("Generated GitHub auth URL with PKCE + session-bound CSRF state");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(auth_url.as_str()))
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id, result))]
pub async fn github_link(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating GitHub OAuth link");

    let user = auth.user.as_ref().ok_or_else(|| {
        warn!("GitHub link attempted without an authenticated session");
        ErrorResponse::new(ErrorCode::Unauthorized).with_message("No active session")
    })?;
    crate::services::fresh_auth::consume(&auth, &state.sea_db).await?;
    let link_user_id = user.id;

    let client = get_github_oauth_client()?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user:email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    let session_id = oauth::oauth_session_id(auth.session())?;
    oauth::store_oauth_state(
        &session_id,
        csrf_token.secret(),
        pkce_verifier.secret(),
        None,
        Some(link_user_id),
    )?;

    tracing::Span::current().record("user_id", link_user_id);
    info!("Generated GitHub link URL with PKCE + session-bound CSRF state");
    tracing::Span::current().record("result", "success");

    crate::services::fresh_auth::authorize_link(&auth, csrf_token.secret()).await?;
    Ok(Json(json!({ "url": auth_url.as_str() })))
}

#[debug_handler]
#[instrument(skip(state, auth, query), fields(user_id, result))]
pub async fn github_callback(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedQuery(query): ValidatedQuery<GitHubCallbackQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing GitHub OAuth callback");

    // Provider cancellation/error (?error=access_denied, …) — never attempt an exchange.
    if query.is_error() {
        tracing::Span::current().record("result", "cancelled");
        if oauth::state::link_user_id_from_cancelled_state(auth.session(), query.state.as_deref())
            .is_some()
        {
            tracing::Span::current().record("result", "link_cancelled");
            let url = oauth::redirect::build_link_failure_redirect(
                oauth::redirect::LINK_FAILURE_CANCELLED,
            )?;
            return Ok(Redirect::temporary(&url));
        }
        let url = oauth::redirect::build_failure_redirect(
            oauth::OAuthProvider::Github,
            oauth::redirect::FAILURE_CANCELLED,
        )?;
        return Ok(Redirect::temporary(&url));
    }

    match run_github_callback(&state, &mut auth, query).await {
        Ok(CallbackOutcome::Login(user_id)) => {
            tracing::Span::current().record("user_id", user_id);
            info!(user_id, "GitHub login successful");
            tracing::Span::current().record("result", "success");
            let url = oauth::redirect::build_success_redirect(oauth::OAuthProvider::Github)?;
            Ok(Redirect::temporary(&url))
        }
        Ok(CallbackOutcome::TotpRequired) => {
            tracing::Span::current().record("result", "totp_required");
            let url = oauth::redirect::build_totp_redirect(oauth::OAuthProvider::Github)?;
            Ok(Redirect::temporary(&url))
        }
        Ok(CallbackOutcome::Linked) => {
            tracing::Span::current().record("result", "linked");
            info!("GitHub identity linked to the current account");
            let url = oauth::redirect::build_link_success_redirect(oauth::OAuthProvider::Github)?;
            Ok(Redirect::temporary(&url))
        }
        Ok(CallbackOutcome::AlreadyLinkedOtherUser) => {
            tracing::Span::current().record("result", "already_linked");
            warn!("GitHub identity already belongs to another account");
            let url = oauth::redirect::build_link_failure_redirect(
                oauth::redirect::LINK_FAILURE_ALREADY_LINKED,
            )?;
            Ok(Redirect::temporary(&url))
        }
        Err(CallbackFailure {
            link_mode: true,
            error,
        }) => {
            warn!(code = %error.code, "GitHub link callback failed; redirecting to account link error");
            tracing::Span::current().record("result", "link_error");
            let url =
                oauth::redirect::build_link_failure_redirect(oauth::redirect::LINK_FAILURE_FAILED)?;
            Ok(Redirect::temporary(&url))
        }
        Err(CallbackFailure {
            link_mode: false,
            error,
        }) => {
            warn!(code = %error.code, "GitHub callback failed; redirecting to opaque failure path");
            tracing::Span::current().record("result", "error");
            let url = oauth::redirect::build_failure_redirect(
                oauth::OAuthProvider::Github,
                oauth::redirect::error_to_failure_code(&error),
            )?;
            Ok(Redirect::temporary(&url))
        }
    }
}

/// Inner callback body surfaced to the caller, which redirects failures to the opaque failure path.
async fn run_github_callback(
    state: &AppState,
    auth: &mut AuthSession,
    query: GitHubCallbackQuery,
) -> Result<CallbackOutcome, CallbackFailure> {
    let session_id = oauth::oauth_session_id(auth.session()).map_err(CallbackFailure::login)?;
    let state_secret = query.state().map_err(CallbackFailure::login)?;
    let oauth_state =
        oauth::consume_oauth_state(&session_id, &state_secret).map_err(CallbackFailure::login)?;
    let link_user_id = oauth_state.link_user_id;
    let link_mode = link_user_id.is_some();
    let fail = move |error: ErrorResponse| CallbackFailure { link_mode, error };

    let client = get_github_oauth_client().map_err(fail)?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(query.code().map_err(fail)?));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(oauth::token_exchange_http_client().map_err(fail)?)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange GitHub authorization code");
            fail(
                ErrorResponse::new(ErrorCode::ExternalServiceError)
                    .with_message("Failed to exchange authorization code")
                    .with_details(e.to_string()),
            )
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_github_user_info(access_token).await.map_err(fail)?;

    if let Some(target_user_id) = link_user_id {
        let confirmed =
            crate::services::fresh_auth::consume_link(auth, &state.sea_db, &state_secret)
                .await
                .map_err(fail)?;
        let outcome =
            link_github_identity(&state.sea_db, Some(&confirmed), target_user_id, &user_info)
                .await
                .map_err(fail)?;
        return Ok(outcome);
    }

    let result = finish_github_login(state, auth, user_info, access_token)
        .await
        .map_err(fail)?;
    oauth::login::callback_login_outcome(auth, result.user.id)
        .await
        .map_err(fail)
}

/// Links GitHub's identity to the same confirmed session account.
/// Leaves login sessions and provider metadata intact.
async fn link_github_identity(
    db: &sea_orm::DatabaseConnection,
    session_user: Option<&crate::db::sea_models::user::Model>,
    target_user_id: i32,
    user_info: &GitHubUserInfo,
) -> Result<CallbackOutcome, ErrorResponse> {
    if session_user.map(|user| user.id) != Some(target_user_id) {
        warn!("GitHub link callback session does not match the state link target");
        return Err(ErrorResponse::new(ErrorCode::Unauthorized)
            .with_message("Link session does not match the link target"));
    }

    let outcome = link_identity_to_unchanged_user(
        db,
        session_user.ok_or_else(|| ErrorResponse::new(ErrorCode::Unauthorized))?,
        oauth::OAuthProvider::Github,
        &user_info.id.to_string(),
    )
    .await?;
    Ok(CallbackOutcome::from(outcome))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn github_exchange(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<GitHubExchangeRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing GitHub OAuth code exchange from client");

    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &payload.state)?;
    if oauth_state.link_user_id.is_some() {
        warn!("Rejecting account-link state on the GitHub exchange endpoint");
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Account linking must complete through the OAuth callback"));
    }

    let client = get_github_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(payload.code));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(oauth::token_exchange_http_client()?)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange GitHub authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_github_user_info(access_token).await?;
    let result = finish_github_login(&state, &mut auth, user_info, access_token).await?;
    let user = result.user;

    info!(
        user_id = user.id,
        "GitHub login successful via client exchange"
    );
    tracing::Span::current().record("result", "success");

    if let Some(body) = crate::services::pending_login::response(&auth).await? {
        tracing::Span::current().record("result", "totp_required");
        return Ok((StatusCode::OK, session_rotated_headers(false), Json(body)));
    }

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with GitHub"
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, payload, headers), fields(user_id, result))]
pub async fn github_token(
    State(state): State<AppState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<GitHubTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing GitHub mobile token sign-in");

    oauth::ensure_native_token_request(&headers)?;

    let credentials = load_github_credentials()?;
    let expected_user_id = verify_github_access_token(
        &payload.access_token,
        &credentials.client_id,
        &credentials.client_secret,
    )
    .await?;
    let user_info = fetch_github_user_info(&payload.access_token).await?;
    let user_info = reconcile_github_user_info(expected_user_id, user_info)?;
    let result = finish_github_login(&state, &mut auth, user_info, &payload.access_token).await?;
    let provider_profile = json!({
        "name": &result.provider_name,
        "email": &result.provider_email,
        "picture": &result.provider_picture,
    });
    let user = result.user;

    info!(user_id = user.id, "GitHub mobile login successful");
    tracing::Span::current().record("user_id", user.id);
    tracing::Span::current().record("result", "success");

    if let Some(body) = crate::services::pending_login::response(&auth).await? {
        tracing::Span::current().record("result", "totp_required");
        return Ok((StatusCode::OK, session_rotated_headers(false), Json(body)));
    }

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "providerProfile": provider_profile,
            "message": "Successfully authenticated with GitHub"
        })),
    ))
}

#[debug_handler(state = AppState)]
pub async fn github_user_info(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => Ok((StatusCode::OK, Json(json!(user)))),
        None => Err(ErrorResponse::new(ErrorCode::Unauthorized)),
    }
}

fn github_api_http_client() -> Result<&'static reqwest::Client, ErrorResponse> {
    GITHUB_API_HTTP_CLIENT.as_ref().map_err(|error| {
        error!(error = %error, "Failed to build GitHub API HTTP client");
        ErrorResponse::new(ErrorCode::ExternalServiceError).with_message("Failed to contact GitHub")
    })
}

fn github_api_request(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
        // GitHub rejects API requests without a User-Agent.
        .header(reqwest::header::USER_AGENT, GITHUB_USER_AGENT)
}

fn github_app_token_url(client_id: &str) -> Result<reqwest::Url, ErrorResponse> {
    let mut url = reqwest::Url::parse(&github_api_base()).map_err(|e| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Invalid GitHub API URL")
            .with_details(e.to_string())
    })?;
    url.path_segments_mut()
        .map_err(|_| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid GitHub API URL")
        })?
        .extend(["applications", client_id, "token"]);
    Ok(url)
}

async fn verify_github_access_token(
    access_token: &str,
    expected_client_id: &str,
    client_secret: &str,
) -> Result<i64, ErrorResponse> {
    let http_client = github_api_http_client()?;
    let url = github_app_token_url(expected_client_id)?;
    verify_github_access_token_from_url(
        http_client,
        url.as_str(),
        access_token,
        expected_client_id,
        client_secret,
    )
    .await
}

async fn verify_github_access_token_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
    expected_client_id: &str,
    client_secret: &str,
) -> Result<i64, ErrorResponse> {
    let response = github_api_request(
        http_client
            .post(url)
            .basic_auth(expected_client_id, Some(client_secret))
            .json(&json!({"access_token": access_token})),
    )
    .send()
    .await
    .map_err(|_| {
        // This request carries the app secret and user token. Never include request details.
        error!("Failed to inspect GitHub access token");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to verify GitHub access token")
    })?;
    let inspection: GitHubTokenInspection = decode_github_api_response(
        response,
        &[StatusCode::NOT_FOUND, StatusCode::UNPROCESSABLE_ENTITY],
    )
    .await?;
    validate_github_token_inspection(inspection, expected_client_id)
}

fn validate_github_token_inspection(
    inspection: GitHubTokenInspection,
    expected_client_id: &str,
) -> Result<i64, ErrorResponse> {
    if inspection.app.client_id != expected_client_id || inspection.user.id <= 0 {
        warn!("GitHub token inspection returned a foreign app or invalid user");
        return Err(invalid_github_token());
    }
    Ok(inspection.user.id)
}

fn invalid_github_token() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidToken)
        .with_message("GitHub access token is invalid or expired")
}

async fn fetch_github_user_info(access_token: &str) -> Result<GitHubUserInfo, ErrorResponse> {
    let http_client = github_api_http_client()?;
    fetch_github_user_info_from_url(
        http_client,
        &format!("{}/user", github_api_base()),
        access_token,
    )
    .await
}

async fn fetch_github_user_info_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<GitHubUserInfo, ErrorResponse> {
    let response = github_api_request(http_client.get(url).bearer_auth(access_token))
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from GitHub");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from GitHub")
        })?;
    decode_github_api_response(response, &[StatusCode::UNAUTHORIZED]).await
}

async fn fetch_github_primary_verified_email(
    access_token: &str,
) -> Result<Option<GitHubEmail>, ErrorResponse> {
    let http_client = github_api_http_client()?;
    fetch_github_primary_verified_email_from_url(
        http_client,
        &format!("{}/user/emails", github_api_base()),
        access_token,
    )
    .await
}

async fn fetch_github_primary_verified_email_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<Option<GitHubEmail>, ErrorResponse> {
    let response = github_api_request(
        http_client
            .get(url)
            .query(&[("per_page", 100)])
            .bearer_auth(access_token),
    )
    .send()
    .await
    .map_err(|e| {
        error!(error = ?e, "Failed to fetch emails from GitHub");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to fetch emails from GitHub")
    })?;
    if response.status() == StatusCode::FORBIDDEN {
        // Tokens without user:email can still authenticate an already-linked identity. New links
        // remain blocked below because they require a verified provider email.
        warn!("GitHub token cannot read private emails; using public profile email if available");
        return Ok(None);
    }
    let emails: Vec<GitHubEmail> =
        decode_github_api_response(response, &[StatusCode::UNAUTHORIZED]).await?;

    Ok(emails
        .iter()
        .find(|e| e.primary && e.verified)
        .or_else(|| emails.iter().find(|e| e.verified))
        .cloned())
}

async fn decode_github_api_response<T: DeserializeOwned>(
    mut response: reqwest::Response,
    invalid_token_statuses: &[StatusCode],
) -> Result<T, ErrorResponse> {
    let status = response.status();
    if invalid_token_statuses.contains(&status) {
        warn!(status = %status, "GitHub rejected access token");
        return Err(invalid_github_token());
    }
    if !status.is_success() {
        error!(status = %status, "GitHub API endpoint returned non-2xx");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to contact GitHub"));
    }
    if response
        .content_length()
        .is_some_and(|length| length > GITHUB_API_MAX_BYTES as u64)
    {
        error!("GitHub API content length exceeded size limit");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse GitHub response"));
    }

    let mut body = Vec::with_capacity(GITHUB_API_MAX_BYTES.min(4096));
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        error!(error = ?e, "Failed to read GitHub API response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to read GitHub response")
    })? {
        if body.len().saturating_add(chunk.len()) > GITHUB_API_MAX_BYTES {
            error!("GitHub API response exceeded size limit");
            return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse GitHub response"));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body).map_err(|e| {
        error!(error = ?e, "Failed to parse GitHub API response");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse GitHub response")
    })
}

fn reconcile_github_user_info(
    expected_user_id: i64,
    user_info: GitHubUserInfo,
) -> Result<GitHubUserInfo, ErrorResponse> {
    if expected_user_id <= 0
        || user_info.id != expected_user_id
        || user_info.login.trim().is_empty()
    {
        warn!("GitHub token/profile identity mismatch");
        return Err(invalid_github_token());
    }
    Ok(user_info)
}

/// GitHub's `/user` `email` is public but unverified; only `/user/emails` carries the real `verified` flag. Treat the public email as unverified.
async fn finish_github_login(
    state: &AppState,
    auth: &mut AuthSession,
    user_info: GitHubUserInfo,
    access_token: &str,
) -> Result<GitHubLoginResult, ErrorResponse> {
    let verified_email = fetch_github_primary_verified_email(access_token).await?;

    let (email, email_verified) = match (verified_email.as_ref(), user_info.email.as_ref()) {
        (Some(ve), _) => (Some(ve.email.clone()), ve.verified),
        (None, Some(public)) => {
            warn!("GitHub returned no verified email; falling back to unverified public email");
            (Some(public.clone()), false)
        }
        (None, None) => {
            warn!("GitHub returned no email; only an existing identity link can sign in");
            (None, false)
        }
    };

    let name = user_info
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| user_info.login.clone());
    let provider_user_id = user_info.id.to_string();

    let user = oauth::find_or_create_user_for_oauth(
        &state.sea_db,
        oauth::OAuthProvider::Github,
        &provider_user_id,
        email.clone(),
        name.clone(),
        email_verified,
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Github).await?;
    Ok(GitHubLoginResult {
        user,
        provider_name: name,
        provider_email: email,
        provider_picture: user_info
            .avatar_url
            .filter(|value| !value.trim().is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::auth_v1::controller::{session_rotated_headers, SESSION_ROTATED};
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // Each #[tokio::test] owns a short-lived runtime. The shared static client would park
    // idle connections whose dispatch tasks die with that runtime, so a later request that
    // reuses one fails instantly (hyper DispatchGone). Build a fresh client per test.
    fn test_http_client() -> reqwest::Client {
        github_api_http_client_builder().build().unwrap()
    }

    fn valid_token_inspection() -> GitHubTokenInspection {
        GitHubTokenInspection {
            app: GitHubTokenApp {
                client_id: "configured-github-app".to_string(),
            },
            user: GitHubTokenOwner { id: 12345 },
        }
    }

    fn github_profile(id: i64) -> GitHubUserInfo {
        GitHubUserInfo {
            id,
            login: "aisha".to_string(),
            name: Some("Aisha Khan".to_string()),
            email: Some("public@example.com".to_string()),
            avatar_url: Some("https://avatars.github.test/aisha.png".to_string()),
        }
    }

    async fn mem_db() -> sea_orm::DatabaseConnection {
        use migration::{Migrator, MigratorTrait};
        use sea_orm::{ConnectOptions, Database, DatabaseConnection};

        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db: DatabaseConnection = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn insert_user(
        db: &sea_orm::DatabaseConnection,
        email: &str,
    ) -> crate::db::sea_models::user::Model {
        use sea_orm::{ActiveModelTrait, Set};

        let now = chrono::Utc::now().fixed_offset();
        crate::db::sea_models::user::ActiveModel {
            name: Set("Test User".to_string()),
            email: Set(email.to_string()),
            password: Set(Some("password-hash".to_string())),
            role: Set(crate::db::sea_models::user::UserRole::User),
            is_verified: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    #[test]
    fn token_inspection_rejects_token_from_different_app() {
        let result = validate_github_token_inspection(valid_token_inspection(), "other-github-app");

        assert!(result.is_err(), "foreign-app token must fail");
    }

    #[test]
    fn profile_rejects_different_user_than_token_inspection() {
        let result = reconcile_github_user_info(12345, github_profile(98765));

        assert!(result.is_err(), "profile/token identity mismatch must fail");
    }

    #[test]
    fn token_check_url_percent_encodes_client_id_as_one_path_segment() {
        let url = github_app_token_url("client/id").unwrap();

        assert_eq!(
            url.as_str(),
            "https://api.github.com/applications/client%2Fid/token"
        );
    }

    #[tokio::test]
    async fn token_inspection_request_binds_token_to_configured_app() {
        let server = MockServer::start().await;
        let basic = BASE64_STANDARD.encode("configured-github-app:github-app-secret");
        Mock::given(method("POST"))
            .and(path("/applications/configured-github-app/token"))
            .and(header("authorization", format!("Basic {basic}").as_str()))
            .and(header("accept", "application/vnd.github+json"))
            .and(header("x-github-api-version", GITHUB_API_VERSION))
            .and(header("user-agent", GITHUB_USER_AGENT))
            .and(body_string_contains(
                "\"access_token\":\"github-user-token\"",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "token": "must-not-be-returned-or-logged",
                "app": {"client_id": "configured-github-app"},
                "user": {"id": 12345}
            })))
            .mount(&server)
            .await;
        let client = test_http_client();

        let user_id = verify_github_access_token_from_url(
            &client,
            &format!("{}/applications/configured-github-app/token", server.uri()),
            "github-user-token",
            "configured-github-app",
            "github-app-secret",
        )
        .await
        .unwrap();

        assert_eq!(user_id, 12345);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(
            requests[0].url.query().is_none(),
            "token must not enter URL"
        );
    }

    #[tokio::test]
    async fn invalid_token_inspection_statuses_are_invalid_token() {
        for status in [StatusCode::NOT_FOUND, StatusCode::UNPROCESSABLE_ENTITY] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(status.as_u16()))
                .mount(&server)
                .await;

            let error = verify_github_access_token_from_url(
                &test_http_client(),
                &format!("{}/applications/app/token", server.uri()),
                "invalid-token",
                "app",
                "secret",
            )
            .await
            .unwrap_err();

            assert_eq!(error.code, ErrorCode::InvalidToken);
        }
    }

    #[tokio::test]
    async fn user_info_request_decodes_profile_picture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", "Bearer github-user-token"))
            .and(header("x-github-api-version", GITHUB_API_VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 12345,
                "login": "aisha",
                "name": "Aisha Khan",
                "email": "public@example.com",
                "avatar_url": "https://avatars.github.test/aisha.png"
            })))
            .mount(&server)
            .await;

        let profile = fetch_github_user_info_from_url(
            &test_http_client(),
            &format!("{}/user", server.uri()),
            "github-user-token",
        )
        .await
        .unwrap();

        assert_eq!(profile.id, 12345);
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://avatars.github.test/aisha.png")
        );
    }

    #[tokio::test]
    async fn user_info_unauthorized_is_invalid_token_but_forbidden_is_external() {
        for (status, expected) in [
            (StatusCode::UNAUTHORIZED, ErrorCode::InvalidToken),
            (StatusCode::FORBIDDEN, ErrorCode::ExternalServiceError),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(status.as_u16()))
                .mount(&server)
                .await;

            let error = fetch_github_user_info_from_url(
                &test_http_client(),
                &format!("{}/user", server.uri()),
                "github-user-token",
            )
            .await
            .unwrap_err();

            assert_eq!(error.code, expected);
        }
    }

    #[tokio::test]
    async fn verified_primary_email_is_preferred() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/emails"))
            .and(query_param("per_page", "100"))
            .and(header("authorization", "Bearer github-user-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"email": "verified@example.com", "primary": false, "verified": true},
                {"email": "primary@example.com", "primary": true, "verified": true}
            ])))
            .mount(&server)
            .await;

        let email = fetch_github_primary_verified_email_from_url(
            &test_http_client(),
            &format!("{}/user/emails", server.uri()),
            "github-user-token",
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(email.email, "primary@example.com");
    }

    #[tokio::test]
    async fn email_scope_forbidden_falls_back_without_trusting_public_email() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let email = fetch_github_primary_verified_email_from_url(
            &test_http_client(),
            &format!("{}/user/emails", server.uri()),
            "github-user-token-without-email-scope",
        )
        .await
        .unwrap();

        assert!(email.is_none());
    }

    #[tokio::test]
    async fn github_api_redirect_is_not_followed() {
        let target = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/leak-target"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&target)
            .await;
        let source = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("location", format!("{}/leak-target", target.uri())),
            )
            .mount(&source)
            .await;

        let error = fetch_github_user_info_from_url(
            &test_http_client(),
            &format!("{}/user", source.uri()),
            "must-not-cross-redirect",
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::ExternalServiceError);
    }

    #[tokio::test]
    async fn oversized_github_response_is_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(vec![b'x'; GITHUB_API_MAX_BYTES + 1]),
            )
            .mount(&server)
            .await;

        let error = fetch_github_user_info_from_url(
            &test_http_client(),
            &format!("{}/user", server.uri()),
            "github-user-token",
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

    #[tokio::test]
    async fn link_mode_links_identity_without_creating_or_logging_in() {
        use sea_orm::EntityTrait;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", "Bearer github-user-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 12345,
                "login": "aisha",
                "name": "Aisha Khan",
                "email": "public@example.com",
                "avatar_url": "https://avatars.github.test/aisha.png"
            })))
            .mount(&server)
            .await;
        // Link mode reads the identity id from /user only — the login path's email fetch never runs.
        Mock::given(method("GET"))
            .and(path("/user/emails"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(0)
            .mount(&server)
            .await;

        let db = mem_db().await;
        let user = insert_user(&db, "linker@example.com").await;
        let profile = fetch_github_user_info_from_url(
            &test_http_client(),
            &format!("{}/user", server.uri()),
            "github-user-token",
        )
        .await
        .unwrap();

        let outcome = link_github_identity(&db, Some(&user), user.id, &profile)
            .await
            .unwrap();

        assert!(matches!(outcome, CallbackOutcome::Linked));
        let identity = crate::db::sea_models::user_oauth_identity::Entity::find_by_provider(
            &db, "github", "12345",
        )
        .await
        .unwrap()
        .expect("provider identity linked");
        assert_eq!(identity.user_id, user.id);
        let users = crate::db::sea_models::user::Entity::find()
            .all(&db)
            .await
            .unwrap();
        assert_eq!(users.len(), 1, "link mode must not create or log in a user");
    }

    #[tokio::test]
    async fn link_mode_rejects_session_target_mismatch() {
        let db = mem_db().await;
        let session_user = insert_user(&db, "session@example.com").await;
        let target = insert_user(&db, "target@example.com").await;

        let error =
            link_github_identity(&db, Some(&session_user), target.id, &github_profile(12345))
                .await
                .unwrap_err();

        assert_eq!(error.code, ErrorCode::Unauthorized);
        assert!(
            crate::db::sea_models::user_oauth_identity::Entity::find_by_provider(
                &db, "github", "12345",
            )
            .await
            .unwrap()
            .is_none(),
            "mismatched link session must not attach any identity"
        );
    }

    #[tokio::test]
    async fn link_mode_reports_identity_owned_by_another_user() {
        let db = mem_db().await;
        let owner = insert_user(&db, "owner@example.com").await;
        let target = insert_user(&db, "target@example.com").await;
        let profile = github_profile(12345);
        link_github_identity(&db, Some(&owner), owner.id, &profile)
            .await
            .unwrap();

        let outcome = link_github_identity(&db, Some(&target), target.id, &profile)
            .await
            .unwrap();

        assert!(matches!(outcome, CallbackOutcome::AlreadyLinkedOtherUser));
    }
}
