use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_macros::debug_handler;
use oauth2::PkceCodeChallenge;
use serde_json::json;
use tower_sessions::Session;
use tracing::{info, instrument, warn};

use crate::{
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    extractors::ValidatedQuery,
    modules::auth_v1::controller::session_rotated_headers,
    services::{auth::AuthSession, oauth},
    AppState,
};

use super::{
    service::{
        build_apple_authorize_url, exchange_apple_code, load_apple_config,
        mint_apple_client_secret, verify_apple_id_token,
    },
    validator::{AppleCallbackQuery, AppleExchangeRequest, AppleTokenRequest},
};

#[debug_handler]
#[instrument(skip(_state, session), fields(result))]
pub async fn apple_login(
    State(_state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating Apple Sign-in");

    let cfg = load_apple_config()?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let nonce = oauth::generate_oauth_nonce();
    let csrf = oauth2::CsrfToken::new_random();

    let auth_url = build_apple_authorize_url(&cfg, csrf.secret(), pkce_challenge.as_str(), &nonce)?;

    let session_id = oauth::oauth_session_id(&session)?;
    oauth::store_oauth_state(
        &session_id,
        csrf.secret(),
        pkce_verifier.secret(),
        Some(&nonce),
    )?;

    info!("Generated Apple auth URL with PKCE + session-bound CSRF state + OIDC nonce");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(&auth_url))
}

#[debug_handler]
#[instrument(skip(state, auth, query), fields(user_id, result))]
pub async fn apple_callback(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedQuery(query): ValidatedQuery<AppleCallbackQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Apple OAuth callback");

    // Provider cancellation/error (?error=user_cancelled, …) — never attempt an exchange.
    if query.is_error() {
        tracing::Span::current().record("result", "cancelled");
        let url = oauth::redirect::build_failure_redirect(
            oauth::OAuthProvider::Apple,
            oauth::redirect::FAILURE_CANCELLED,
        )?;
        return Ok(Redirect::temporary(&url));
    }

    match run_apple_callback(&state, &mut auth, query).await {
        Ok(user) => {
            tracing::Span::current().record("user_id", user.id);
            info!(user_id = user.id, "Apple login successful");
            tracing::Span::current().record("result", "success");
            let url = oauth::redirect::build_success_redirect(oauth::OAuthProvider::Apple)?;
            Ok(Redirect::temporary(&url))
        }
        Err(err) => {
            warn!(code = %err.code, "Apple callback failed; redirecting to opaque failure path");
            tracing::Span::current().record("result", "error");
            let url = oauth::redirect::build_failure_redirect(
                oauth::OAuthProvider::Apple,
                oauth::redirect::error_to_failure_code(&err),
            )?;
            Ok(Redirect::temporary(&url))
        }
    }
}

/// Inner callback body surfaced to the caller, which redirects failures to the opaque failure path.
async fn run_apple_callback(
    state: &AppState,
    auth: &mut AuthSession,
    query: AppleCallbackQuery,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    finish_apple_code(state, auth, &query.code()?, &query.state()?).await
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn apple_exchange(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<AppleExchangeRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Apple OAuth code exchange from client");

    let user = finish_apple_code(&state, &mut auth, &payload.code, &payload.state).await?;

    info!(
        user_id = user.id,
        "Apple login successful via client exchange"
    );
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with Apple"
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn apple_token(
    State(state): State<AppState>,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<AppleTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Apple mobile token sign-in");

    let cfg = load_apple_config()?;

    // No OIDC nonce: the native Sign in with Apple flow never bound one, unlike the web code flow.
    let claims = verify_apple_id_token(&payload.identity_token, &cfg.client_id, None)
        .await
        .map_err(|e| {
            warn!(error = ?e, "Apple mobile identity_token verification failed");
            tracing::Span::current().record("result", "invalid_token");
            e
        })?;

    let email = claims.email.clone().ok_or_else(|| {
        warn!("Apple id_token carried no email; cannot create/link account");
        ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Apple did not provide an email address")
    })?;

    let user = oauth::find_or_create_user_for_oauth(
        &state,
        oauth::OAuthProvider::Apple,
        &claims.sub,
        email,
        "Apple User".to_string(),
        claims.is_email_verified(),
    )
    .await?;

    oauth::finish_oauth_login(&state, &mut auth, &user, oauth::OAuthProvider::Apple).await?;

    info!(user_id = user.id, "Apple mobile login successful");
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with Apple"
        })),
    ))
}

#[debug_handler(state = AppState)]
pub async fn apple_user_info(auth: AuthSession) -> Result<impl IntoResponse, ErrorResponse> {
    match auth.user {
        Some(user) => Ok((StatusCode::OK, Json(json!(user)))),
        None => Err(ErrorResponse::new(ErrorCode::Unauthorized)),
    }
}

async fn finish_apple_code(
    state: &AppState,
    auth: &mut AuthSession,
    code: &str,
    state_secret: &str,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, state_secret)?;
    let code_verifier = oauth_state
        .pkce_verifier
        .as_ref()
        .map(|v| v.secret().to_string());
    let nonce = oauth_state.nonce;

    let cfg = load_apple_config()?;
    let client_secret_jwt = mint_apple_client_secret(&cfg)?;

    let token_resp = exchange_apple_code(
        &state.http_client,
        &cfg,
        code,
        &client_secret_jwt,
        code_verifier.as_deref(),
    )
    .await?;

    let claims =
        verify_apple_id_token(&token_resp.id_token, &cfg.client_id, nonce.as_deref()).await?;

    let email = claims.email.clone().ok_or_else(|| {
        warn!("Apple id_token carried no email; cannot create/link account");
        ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Apple did not provide an email address")
    })?;

    let email_verified = claims.is_email_verified();
    let name = "Apple User".to_string();

    let user = oauth::find_or_create_user_for_oauth(
        state,
        oauth::OAuthProvider::Apple,
        &claims.sub,
        email,
        name,
        email_verified,
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Apple).await?;
    Ok(user)
}
