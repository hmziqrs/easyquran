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
    db::sea_models::{user, user_session},
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    extractors::ValidatedQuery,
    services::{auth::AuthSession, oauth},
    AppState,
};

use super::{
    service::{get_google_oauth_client, verify_google_id_token, GoogleIdTokenClaims},
    validator::{GoogleCallbackQuery, GoogleExchangeRequest, GoogleUserInfo},
};

#[debug_handler]
#[instrument(skip(_state, session), fields(result))]
pub async fn google_login(
    State(_state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating Google OAuth login");

    let client = get_google_oauth_client()?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // OIDC nonce binds this flow to the signed id_token checked at callback;
    // defeats token-injection/replay. (oauth2 4.4 has no nonce builder.)
    let nonce = oauth::generate_oauth_nonce();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_extra_param("nonce", nonce.clone())
        .set_pkce_challenge(pkce_challenge)
        .url();

    let session_id = oauth::oauth_session_id(&session)?;
    oauth::store_oauth_state(
        &session_id,
        csrf_token.secret(),
        pkce_verifier.secret(),
        Some(&nonce),
    )?;

    info!("Generated auth URL with PKCE + session-bound CSRF state + OIDC nonce");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(auth_url.as_str()))
}

#[debug_handler]
#[instrument(skip(state, auth, query), fields(user_id, result))]
pub async fn google_callback(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedQuery(query): ValidatedQuery<GoogleCallbackQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Google OAuth callback");

    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &query.state)?;

    let client = get_google_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(query.code));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let user = finish_google_login(
        &state,
        &mut auth,
        token_result,
        oauth_state.nonce.as_deref(),
    )
    .await?;

    info!(user_id = user.id, "Google login successful");
    tracing::Span::current().record("result", "success");

    // Allow-list the redirect — a raw FRONTEND_URL concat is an open redirect.
    let redirect_url = oauth::build_allowed_success_redirect("/auth/google/success")?;

    Ok(Redirect::temporary(&redirect_url))
}

#[debug_handler(state = AppState)]
pub async fn google_user_info(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => Ok((StatusCode::OK, Json(json!(user)))),
        None => Err(ErrorResponse::new(ErrorCode::Unauthorized)),
    }
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn google_exchange(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<GoogleExchangeRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Google OAuth code exchange from client");

    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &payload.state)?;

    let client = get_google_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(payload.code));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange authorization code");
            tracing::Span::current().record("result", "token_exchange_failed");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    let user = finish_google_login(
        &state,
        &mut auth,
        token_result,
        oauth_state.nonce.as_deref(),
    )
    .await?;

    info!(
        user_id = user.id,
        "Google login successful via client exchange"
    );
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with Google"
        })),
    ))
}

async fn finish_google_login(
    state: &AppState,
    auth: &mut AuthSession,
    token_result: oauth2::StandardTokenResponse<
        super::service::IdTokenFields,
        oauth2::basic::BasicTokenType,
    >,
    expected_nonce: Option<&str>,
) -> Result<user::Model, ErrorResponse> {
    let access_token = token_result.access_token().secret();

    // Verify the signed id_token and cross-check sub/email below — defeats token-substitution into a victim profile.
    let id_token = token_result
        .extra_fields()
        .id_token
        .as_deref()
        .ok_or_else(|| {
            warn!("Google token response omitted id_token; rejecting login");
            tracing::Span::current().record("result", "missing_id_token");
            ErrorResponse::new(ErrorCode::InvalidToken)
                .with_message("OAuth identity verification failed")
        })?;

    let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GOOGLE_CLIENT_ID not configured")
    })?;
    let id_claims: Option<GoogleIdTokenClaims> =
        match verify_google_id_token(id_token, &client_id, expected_nonce).await {
            Ok(claims) => Some(claims),
            Err(err) => {
                warn!(error = ?err, "id_token verification failed; rejecting login");
                return Err(err);
            }
        };

    let user_info = fetch_google_user_info(&state.http_client, access_token).await?;
    info!(google_id = %user_info.id, email = %user_info.email, "Retrieved user info from Google");

    if let Some(claims) = &id_claims {
        if claims.sub != user_info.id || claims.email != user_info.email {
            warn!(
                id_sub = %claims.sub,
                userinfo_id = %user_info.id,
                "id_token/userinfo identity mismatch — rejecting login"
            );
            return Err(ErrorResponse::new(ErrorCode::InvalidToken)
                .with_message("OAuth identity verification failed"));
        }
    }

    let user = find_or_create_user(state, user_info).await?;
    tracing::Span::current().record("user_id", user.id);

    auth.login(&user).await.map_err(|e| {
        error!(error = %e, user_id = user.id, "Failed to create session");
        tracing::Span::current().record("result", "session_creation_failed");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Failed to create session")
    })?;

    let session_row = user_session::Entity::create(
        &state.sea_db,
        user_session::NewUserSession::new(user.id, Some("Google OAuth".to_string()), None),
    )
    .await
    .ok();

    // Save tower-session first (auth.login cycled its id), then map row->session for sessions_terminate.
    if (auth.session().save().await).is_ok() {
        if let (Some(row), Some(tower_sid)) = (session_row.as_ref(), auth.session().id()) {
            crate::modules::auth_v1::controller::record_session_mapping(
                row.id,
                &tower_sid.to_string(),
            );
        }
    }

    Ok(user)
}

async fn fetch_google_user_info(
    http_client: &reqwest::Client,
    access_token: &str,
) -> Result<GoogleUserInfo, ErrorResponse> {
    http_client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from Google");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from Google")
        })?
        .json()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to parse user info from Google");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to parse user info from Google")
        })
}

async fn find_or_create_user(
    state: &AppState,
    user_info: GoogleUserInfo,
) -> Result<user::Model, ErrorResponse> {
    if let Some(existing_user) =
        user::Entity::find_by_google_id(&state.sea_db, user_info.id.clone()).await?
    {
        info!(
            user_id = existing_user.id,
            "Existing user found by Google ID"
        );
        return Ok(existing_user);
    }

    if let Some(existing_user) =
        user::Entity::find_by_email(&state.sea_db, user_info.email.clone()).await?
    {
        // Gate on verified_email — else an unverified Google account hijacks the victim's existing account.
        if !user_info.verified_email {
            warn!(
                user_id = existing_user.id,
                email = %user_info.email,
                "Refusing to link Google account: IdP email is not verified"
            );
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("Unable to link this Google account"));
        }

        info!(
            user_id = existing_user.id,
            "Linking Google account to existing user"
        );

        use sea_orm::ActiveModelTrait;
        let mut active: user::ActiveModel = existing_user.clone().into();
        active.google_id = sea_orm::Set(Some(user_info.id.clone()));
        active.oauth_provider = sea_orm::Set(Some("google".to_string()));
        active.updated_at = sea_orm::Set(chrono::Utc::now().fixed_offset());

        let existing_user = active.update(&state.sea_db).await.map_err(|e| {
            error!(error = ?e, "Failed to link Google account");
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Failed to link Google account")
        })?;

        return Ok(existing_user);
    }

    // Same gate on create — without it an unverified email squats a new account at the victim's address.
    if !user_info.verified_email {
        warn!(
            email = %user_info.email,
            "Refusing to create account from Google: IdP email is not verified"
        );
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Google has not verified this email address"));
    }

    info!("Creating new user from Google account");
    user::Entity::create_from_google(
        &state.sea_db,
        user_info.id.clone(),
        user_info.email.clone(),
        user_info.name.clone(),
    )
    .await
}
