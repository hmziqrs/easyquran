use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use axum_client_ip::ClientIp;
use axum_macros::debug_handler;
use oauth2::PkceCodeChallenge;
use serde_json::json;
use std::net::IpAddr;
use tower_sessions::Session;
use tracing::{info, instrument, warn};

use crate::{
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    extractors::ValidatedQuery,
    modules::auth_v1::controller::session_rotated_headers,
    services::{auth::AuthSession, oauth, registration_admission::CreationSite},
    AppState,
};

use super::{
    service::{
        build_apple_authorize_url, exchange_apple_code, load_apple_config,
        load_apple_token_audiences, mint_apple_client_secret, verify_apple_id_token,
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

    oauth::oauth_session_id(&session)?;
    oauth::store_oauth_state(
        &session,
        csrf.secret(),
        pkce_verifier.secret(),
        Some(&nonce),
        None,
    )
    .await?;

    info!("Generated Apple auth URL with PKCE + session-bound CSRF state + OIDC nonce");
    tracing::Span::current().record("result", "success");

    Ok(Redirect::temporary(&auth_url))
}

#[debug_handler]
#[instrument(skip(state, auth, query, client_ip), fields(user_id, result))]
pub async fn apple_callback(
    State(state): State<AppState>,
    ClientIp(client_ip): ClientIp,
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

    match run_apple_callback(&state, &mut auth, query, client_ip).await {
        Ok(user) => {
            tracing::Span::current().record("user_id", user.id);
            info!(user_id = user.id, "Apple login successful");
            tracing::Span::current().record("result", "success");
            let outcome = oauth::login::callback_login_outcome(&auth, user.id).await?;
            let url = match outcome {
                oauth::login::CallbackOutcome::TotpRequired => {
                    oauth::redirect::build_totp_redirect(oauth::OAuthProvider::Apple)?
                }
                oauth::login::CallbackOutcome::Login(_) => {
                    oauth::redirect::build_success_redirect(oauth::OAuthProvider::Apple)?
                }
                _ => return Err(ErrorResponse::new(ErrorCode::Unauthorized)),
            };
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
    client_ip: IpAddr,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    finish_apple_code(state, auth, &query.code()?, &query.state()?, client_ip).await
}

#[debug_handler]
#[instrument(skip(state, auth, payload, client_ip), fields(user_id, result))]
pub async fn apple_exchange(
    State(state): State<AppState>,
    ClientIp(client_ip): ClientIp,
    mut auth: AuthSession,
    ValidatedJson(payload): ValidatedJson<AppleExchangeRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Apple OAuth code exchange from client");

    let user =
        finish_apple_code(&state, &mut auth, &payload.code, &payload.state, client_ip).await?;

    info!(
        user_id = user.id,
        "Apple login successful via client exchange"
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
            "message": "Successfully authenticated with Apple"
        })),
    ))
}

#[debug_handler]
#[instrument(skip(headers), fields(result))]
pub async fn apple_token_nonce(headers: HeaderMap) -> Result<impl IntoResponse, ErrorResponse> {
    oauth::ensure_native_token_request(&headers)?;
    let challenge = oauth::issue_native_token_challenge(oauth::NativeTokenProvider::Apple)?;
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        Json(json!({
            "nonce": challenge.nonce,
            "providerNonce": challenge.provider_nonce,
            "expiresIn": oauth::NATIVE_TOKEN_NONCE_TTL_SECS
        })),
    ))
}

#[debug_handler]
#[instrument(
    skip(state, auth, payload, headers, client_ip),
    fields(user_id, result)
)]
pub async fn apple_token(
    State(state): State<AppState>,
    ClientIp(client_ip): ClientIp,
    mut auth: AuthSession,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<AppleTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Apple mobile token sign-in");

    oauth::ensure_native_token_request(&headers)?;

    let audiences = load_apple_token_audiences()?;
    let allowed_audiences: Vec<&str> = audiences.iter().map(String::as_str).collect();

    // Verify signature and identity claims before atomically consuming signed one-time nonce.
    let claims = verify_apple_id_token(&payload.identity_token, &allowed_audiences, None)
        .await
        .map_err(|e| {
            warn!(error = ?e, "Apple mobile identity_token verification failed");
            tracing::Span::current().record("result", "invalid_token");
            e
        })?;
    oauth::consume_native_token_nonce(oauth::NativeTokenProvider::Apple, claims.nonce.as_deref())?;

    if claims.sub.trim().is_empty() {
        warn!("Apple id_token carried an empty subject");
        return Err(ErrorResponse::new(ErrorCode::InvalidToken)
            .with_message("Invalid Apple identity token"));
    }

    let email = claims
        .email
        .clone()
        .filter(|value| !value.trim().is_empty());
    let name = "Apple User".to_string();
    let provider_profile = json!({
        "name": &name,
        "email": &email,
        "picture": serde_json::Value::Null,
    });

    let ip = client_ip.to_string();
    let user = oauth::find_or_create_user_for_oauth(
        &state.sea_db,
        oauth::OAuthProvider::Apple,
        &claims.sub,
        email,
        name,
        claims.is_email_verified(),
        CreationSite {
            store: &state.gate_store,
            ip: &ip,
        },
    )
    .await?;

    oauth::finish_oauth_login(&state, &mut auth, &user, oauth::OAuthProvider::Apple).await?;

    info!(user_id = user.id, "Apple mobile login successful");
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
            "message": "Successfully authenticated with Apple"
        })),
    ))
}

/// `POST /auth/apple/v1/token/link`: links Apple to the signed-in native session
/// after fresh-auth confirmation. CSRF-protected (not on the token sign-in exempt list).
#[debug_handler]
#[instrument(skip(state, auth, payload, headers), fields(result))]
pub async fn apple_token_link(
    State(state): State<AppState>,
    auth: AuthSession,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<AppleTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Apple native account link");

    oauth::ensure_native_link_ready(&state, &auth, &headers).await?;

    let audiences = load_apple_token_audiences()?;
    let allowed_audiences: Vec<&str> = audiences.iter().map(String::as_str).collect();
    let claims = verify_apple_id_token(&payload.identity_token, &allowed_audiences, None)
        .await
        .map_err(|e| {
            warn!(error = ?e, "Apple native link identity_token verification failed");
            tracing::Span::current().record("result", "invalid_token");
            e
        })?;
    oauth::consume_native_token_nonce(oauth::NativeTokenProvider::Apple, claims.nonce.as_deref())?;

    if claims.sub.trim().is_empty() {
        warn!("Apple id_token carried an empty subject");
        return Err(ErrorResponse::new(ErrorCode::InvalidToken)
            .with_message("Invalid Apple identity token"));
    }

    let snapshot =
        oauth::finish_native_link(&state, &auth, oauth::OAuthProvider::Apple, &claims.sub).await?;
    tracing::Span::current().record("result", "linked");
    Ok((StatusCode::OK, Json(snapshot)))
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
    client_ip: IpAddr,
) -> Result<crate::db::sea_models::user::Model, ErrorResponse> {
    oauth::oauth_session_id(auth.session())?;
    let oauth_state =
        oauth::consume_oauth_state_guarded(state, client_ip, auth.session(), state_secret).await?;
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

    let claims = verify_apple_id_token(
        &token_resp.id_token,
        &[cfg.client_id.as_str()],
        nonce.as_deref(),
    )
    .await?;

    let email = claims
        .email
        .clone()
        .filter(|value| !value.trim().is_empty());

    let email_verified = claims.is_email_verified();
    let name = "Apple User".to_string();

    let ip = client_ip.to_string();
    let user = oauth::find_or_create_user_for_oauth(
        &state.sea_db,
        oauth::OAuthProvider::Apple,
        &claims.sub,
        email,
        name,
        email_verified,
        CreationSite {
            store: &state.gate_store,
            ip: &ip,
        },
    )
    .await?;

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Apple).await?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use crate::modules::auth_v1::controller::{session_rotated_headers, SESSION_ROTATED};

    #[test]
    fn token_login_rotation_header_is_present() {
        let headers = session_rotated_headers(true);

        assert_eq!(headers.get(SESSION_ROTATED).unwrap(), "1");
    }
}
