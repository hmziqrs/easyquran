use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use oauth2::{reqwest::async_http_client, AuthorizationCode, CsrfToken, Scope, TokenResponse};
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
    service::get_facebook_oauth_client,
    validator::{
        FacebookCallbackQuery, FacebookExchangeRequest, FacebookTokenRequest, FacebookUserInfo,
    },
};

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
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange Facebook authorization code");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_facebook_user_info(&state.http_client, access_token).await?;
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
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange Facebook authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let access_token = token_result.access_token().secret();
    let user_info = fetch_facebook_user_info(&state.http_client, access_token).await?;
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
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn facebook_token(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<FacebookTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Facebook mobile token sign-in");

    // The native Facebook Login SDK hands the app a user access_token; pull profile info straight
    // from the Graph API with it — no web redirect/code exchange round-trip.
    let user_info = fetch_facebook_user_info(&state.http_client, &payload.access_token).await?;
    let user = finish_facebook_login(&state, &mut auth, user_info).await?;

    info!(user_id = user.id, "Facebook mobile login successful");
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "user": user,
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

async fn fetch_facebook_user_info(
    http_client: &reqwest::Client,
    access_token: &str,
) -> Result<FacebookUserInfo, ErrorResponse> {
    http_client
        .get("https://graph.facebook.com/v18.0/me")
        .query(&[("fields", "id,name,email")])
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from Facebook");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from Facebook")
        })?
        .json()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to parse user info from Facebook");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse user info from Facebook")
        })
}

async fn finish_facebook_login(
    state: &AppState,
    auth: &mut AuthSession,
    user_info: FacebookUserInfo,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let email = user_info.email.clone().ok_or_else(|| {
        warn!("Facebook returned no email; cannot create/link account");
        ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Facebook did not provide an email address")
    })?;

    let name = user_info
        .name
        .unwrap_or_else(|| "Facebook User".to_string());
    let user = oauth::find_or_create_user_for_oauth(
        state,
        oauth::OAuthProvider::Facebook,
        &user_info.id,
        email,
        name,
        // email_verified: Graph API emails are policy-verified.
        true,
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Facebook).await?;
    Ok(user)
}
