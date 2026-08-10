use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use oauth2::{
    reqwest::async_http_client, AuthorizationCode, CsrfToken, PkceCodeChallenge, Scope,
    TokenResponse,
};
use serde_json::json;
use tower_sessions::Session;
use tracing::{error, info, instrument, warn};

use crate::{
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    extractors::ValidatedQuery,
    services::{auth::AuthSession, oauth},
    AppState,
};

use super::{
    service::get_github_oauth_client,
    validator::{GitHubCallbackQuery, GitHubEmail, GitHubExchangeRequest, GitHubUserInfo},
};

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
    )?;

    info!("Generated GitHub auth URL with PKCE + session-bound CSRF state");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(auth_url.as_str()))
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
        let url = oauth::redirect::build_failure_redirect(
            oauth::OAuthProvider::Github,
            oauth::redirect::FAILURE_CANCELLED,
        )?;
        return Ok(Redirect::temporary(&url));
    }

    match run_github_callback(&state, &mut auth, query).await {
        Ok(user) => {
            tracing::Span::current().record("user_id", user.id);
            info!(user_id = user.id, "GitHub login successful");
            tracing::Span::current().record("result", "success");
            let url = oauth::redirect::build_success_redirect(oauth::OAuthProvider::Github)?;
            Ok(Redirect::temporary(&url))
        }
        Err(err) => {
            warn!(code = %err.code, "GitHub callback failed; redirecting to opaque failure path");
            tracing::Span::current().record("result", "error");
            let url = oauth::redirect::build_failure_redirect(
                oauth::OAuthProvider::Github,
                oauth::redirect::error_to_failure_code(&err),
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
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &query.state()?)?;

    let client = get_github_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(query.code()?));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange GitHub authorization code");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_github_user_info(&state.http_client, access_token).await?;
    finish_github_login(state, auth, user_info, access_token).await
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

    let client = get_github_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(payload.code));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange GitHub authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_github_user_info(&state.http_client, access_token).await?;
    let user = finish_github_login(&state, &mut auth, user_info, access_token).await?;

    info!(
        user_id = user.id,
        "GitHub login successful via client exchange"
    );
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "user": user,
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

async fn fetch_github_user_info(
    http_client: &reqwest::Client,
    access_token: &str,
) -> Result<GitHubUserInfo, ErrorResponse> {
    http_client
        .get("https://api.github.com/user")
        .bearer_auth(access_token)
        // GitHub API requires a User-Agent or it 403s.
        .header(reqwest::header::USER_AGENT, "ruxlog")
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from GitHub");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from GitHub")
        })?
        .json()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to parse user info from GitHub");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse user info from GitHub")
        })
}

async fn fetch_github_primary_verified_email(
    http_client: &reqwest::Client,
    access_token: &str,
) -> Result<Option<GitHubEmail>, ErrorResponse> {
    let emails: Vec<GitHubEmail> = http_client
        .get("https://api.github.com/user/emails")
        .bearer_auth(access_token)
        .header(reqwest::header::USER_AGENT, "ruxlog")
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch emails from GitHub");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch emails from GitHub")
        })?
        .json()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to parse emails from GitHub");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse emails from GitHub")
        })?;

    Ok(emails
        .iter()
        .find(|e| e.primary && e.verified)
        .or_else(|| emails.iter().find(|e| e.verified))
        .cloned())
}

/// GitHub's `/user` `email` is public but unverified; only `/user/emails` carries the real `verified` flag. Treat the public email as unverified.
async fn finish_github_login(
    state: &AppState,
    auth: &mut AuthSession,
    user_info: GitHubUserInfo,
    access_token: &str,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let verified_email =
        fetch_github_primary_verified_email(&state.http_client, access_token).await?;

    let (email, email_verified) = match (verified_email.as_ref(), user_info.email.as_ref()) {
        (Some(ve), _) => (ve.email.clone(), ve.verified),
        (None, Some(public)) => {
            warn!("GitHub returned no verified email; falling back to unverified public email");
            (public.clone(), false)
        }
        (None, None) => {
            warn!("GitHub returned no email at all; cannot create/link account");
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("GitHub did not provide a verified email address"));
        }
    };

    let name = user_info
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| user_info.login.clone());
    let provider_user_id = user_info.id.to_string();

    let user = oauth::find_or_create_user_for_oauth(
        state,
        oauth::OAuthProvider::Github,
        &provider_user_id,
        email,
        name,
        email_verified,
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Github).await?;
    Ok(user)
}
