use std::sync::LazyLock;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
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
    db::sea_models::user,
    error::{ErrorCode, ErrorResponse},
    extractors::ValidatedJson,
    extractors::ValidatedQuery,
    modules::auth_v1::controller::session_rotated_headers,
    services::{auth::AuthSession, oauth},
    AppState,
};

use super::{
    service::{get_google_oauth_client, verify_google_id_token, GoogleIdTokenClaims},
    validator::{GoogleCallbackQuery, GoogleExchangeRequest, GoogleTokenRequest, GoogleUserInfo},
};

const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const GOOGLE_USERINFO_MAX_BYTES: usize = 64 * 1024;
static GOOGLE_USERINFO_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> =
    LazyLock::new(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|error| error.to_string())
    });

#[debug_handler]
#[instrument(skip(_state, session), fields(result))]
pub async fn google_login(
    State(_state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Initiating Google OAuth login");

    let client = get_google_oauth_client()?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // OIDC nonce binds this flow to the signed id_token checked at callback; defeats token-injection/replay. (oauth2 4.4 has no nonce builder.)
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

    // Provider cancellation/error redirect (?error=access_denied, …) — never attempt an exchange.
    if query.is_error() {
        tracing::Span::current().record("result", "cancelled");
        let url = oauth::redirect::build_failure_redirect(
            oauth::OAuthProvider::Google,
            oauth::redirect::FAILURE_CANCELLED,
        )?;
        return Ok(Redirect::temporary(&url));
    }

    match run_google_callback(&state, &mut auth, query).await {
        Ok(user) => {
            tracing::Span::current().record("user_id", user.id);
            info!(user_id = user.id, "Google login successful");
            tracing::Span::current().record("result", "success");
            let url = oauth::redirect::build_success_redirect(oauth::OAuthProvider::Google)?;
            Ok(Redirect::temporary(&url))
        }
        Err(err) => {
            // Any post-validator failure → opaque failure redirect (no payload/code/state in URL).
            warn!(code = %err.code, "Google callback failed; redirecting to opaque failure path");
            tracing::Span::current().record("result", "error");
            let url = oauth::redirect::build_failure_redirect(
                oauth::OAuthProvider::Google,
                oauth::redirect::error_to_failure_code(&err),
            )?;
            Ok(Redirect::temporary(&url))
        }
    }
}

/// Inner callback body: session/state consumption, code exchange, login finish. Any error here is
/// surfaced to the caller, which redirects to the opaque failure path rather than returning JSON.
async fn run_google_callback(
    state: &AppState,
    auth: &mut AuthSession,
    query: GoogleCallbackQuery,
) -> Result<user::Model, ErrorResponse> {
    let session_id = oauth::oauth_session_id(auth.session())?;
    let oauth_state = oauth::consume_oauth_state(&session_id, &query.state()?)?;

    let client = get_google_oauth_client()?;
    let mut exchange = client.exchange_code(AuthorizationCode::new(query.code()?));
    if let Some(verifier) = oauth_state.pkce_verifier {
        exchange = exchange.set_pkce_verifier(verifier);
    }
    let token_result = exchange
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to exchange authorization code");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to exchange authorization code")
                .with_details(e.to_string())
        })?;

    finish_google_login(state, auth, token_result, oauth_state.nonce.as_deref()).await
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
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "message": "Successfully authenticated with Google"
        })),
    ))
}

#[debug_handler]
#[instrument(skip(headers), fields(result))]
pub async fn google_token_nonce(headers: HeaderMap) -> Result<impl IntoResponse, ErrorResponse> {
    oauth::ensure_native_token_request(&headers)?;
    let challenge = oauth::issue_native_token_challenge(oauth::NativeTokenProvider::Google)?;
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
#[instrument(skip(state, auth, payload, headers), fields(user_id, result))]
pub async fn google_token(
    State(state): State<AppState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<GoogleTokenRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("Processing Google mobile token sign-in");

    oauth::ensure_native_token_request(&headers)?;

    // Native Google SDKs mint id_tokens whose `aud` is the Android/iOS client ID, not the web
    // client — accept every configured audience so a device-issued token verifies here too.
    let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GOOGLE_CLIENT_ID not configured")
    })?;
    let mut allowed_auds: Vec<String> = vec![client_id];
    if let Ok(extra) = std::env::var("GOOGLE_MOBILE_CLIENT_IDS") {
        allowed_auds.extend(
            extra
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        );
    }
    let allowed_auds: Vec<&str> = allowed_auds.iter().map(String::as_str).collect();

    // Verify signature and identity claims before atomically consuming signed one-time nonce.
    let claims = verify_google_id_token(&payload.id_token, &allowed_auds, None)
        .await
        .map_err(|e| {
            warn!(error = ?e, "Google mobile id_token verification failed");
            tracing::Span::current().record("result", "invalid_token");
            e
        })?;
    oauth::consume_native_token_nonce(oauth::NativeTokenProvider::Google, claims.nonce.as_deref())?;

    let user_info = if let Some(access_token) = payload.access_token.as_deref() {
        let provider_profile = fetch_google_user_info(access_token).await?;
        reconcile_google_user_info(&claims, provider_profile)?
    } else {
        let token_profile = google_user_info_from_claims(&claims);
        reconcile_google_user_info(&claims, token_profile)?
    };

    let provider_profile = json!({
        "name": &user_info.name,
        "email": &user_info.email,
        "picture": &user_info.picture,
    });
    let user = find_or_create_user(&state, user_info).await?;
    tracing::Span::current().record("user_id", user.id);

    oauth::finish_oauth_login(&state, &mut auth, &user, oauth::OAuthProvider::Google).await?;

    info!(user_id = user.id, "Google mobile login successful");
    tracing::Span::current().record("result", "success");

    Ok((
        StatusCode::OK,
        session_rotated_headers(true),
        Json(json!({
            "success": true,
            "user": user,
            "providerProfile": provider_profile,
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
    let id_claims =
        match verify_google_id_token(id_token, &[client_id.as_str()], expected_nonce).await {
            Ok(claims) => claims,
            Err(err) => {
                warn!(error = ?err, "id_token verification failed; rejecting login");
                return Err(err);
            }
        };

    let provider_profile = fetch_google_user_info(access_token).await?;
    info!("Retrieved user info from Google");
    let user_info = reconcile_google_user_info(&id_claims, provider_profile)?;

    let user = find_or_create_user(state, user_info).await?;
    tracing::Span::current().record("user_id", user.id);

    oauth::finish_oauth_login(state, auth, &user, oauth::OAuthProvider::Google).await?;

    Ok(user)
}

fn google_user_info_from_claims(claims: &GoogleIdTokenClaims) -> GoogleUserInfo {
    let fallback_name = claims
        .email
        .split('@')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Google User");
    let name = claims
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback_name)
        .to_string();
    let picture = claims
        .picture
        .as_deref()
        .map(str::trim)
        .filter(|picture| !picture.is_empty())
        .map(str::to_string);

    GoogleUserInfo {
        id: claims.sub.clone(),
        email: claims.email.clone(),
        name,
        picture,
        verified_email: claims.email_verified.unwrap_or(false),
    }
}

fn reconcile_google_user_info(
    claims: &GoogleIdTokenClaims,
    mut user_info: GoogleUserInfo,
) -> Result<GoogleUserInfo, ErrorResponse> {
    if claims.sub.trim().is_empty()
        || claims.email.trim().is_empty()
        || claims.email_verified != Some(true)
        || user_info.id.trim().is_empty()
        || user_info.email.trim().is_empty()
        || !user_info.verified_email
    {
        warn!("Google returned incomplete or unverified identity claims");
        return Err(ErrorResponse::new(ErrorCode::InvalidToken)
            .with_message("OAuth identity verification failed"));
    }

    if claims.sub != user_info.id || claims.email != user_info.email {
        warn!("id_token/userinfo identity mismatch — rejecting login");
        return Err(ErrorResponse::new(ErrorCode::InvalidToken)
            .with_message("OAuth identity verification failed"));
    }

    let token_profile = google_user_info_from_claims(claims);
    if user_info.name.trim().is_empty() {
        user_info.name = token_profile.name;
    }
    if user_info
        .picture
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        user_info.picture = token_profile.picture;
    }

    Ok(user_info)
}

fn google_userinfo_http_client() -> Result<&'static reqwest::Client, ErrorResponse> {
    GOOGLE_USERINFO_HTTP_CLIENT.as_ref().map_err(|error| {
        error!(error = %error, "Failed to build Google UserInfo HTTP client");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to fetch user info from Google")
    })
}

async fn fetch_google_user_info(access_token: &str) -> Result<GoogleUserInfo, ErrorResponse> {
    let http_client = google_userinfo_http_client()?;
    fetch_google_user_info_from_url(http_client, GOOGLE_USERINFO_URL, access_token).await
}

async fn fetch_google_user_info_from_url(
    http_client: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<GoogleUserInfo, ErrorResponse> {
    let mut response = http_client
        .get(url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to fetch user info from Google");
            ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from Google")
        })?;

    let status = response.status();
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        warn!(status = %status, "Google rejected UserInfo access token");
        return Err(ErrorResponse::new(ErrorCode::InvalidToken)
            .with_message("Google access token is invalid or expired"));
    }
    if !status.is_success() {
        error!(status = %status, "Google UserInfo endpoint returned non-2xx");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to fetch user info from Google"));
    }

    if response
        .content_length()
        .is_some_and(|length| length > GOOGLE_USERINFO_MAX_BYTES as u64)
    {
        error!("Google UserInfo content length exceeded size limit");
        return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to fetch user info from Google"));
    }

    let mut body = Vec::with_capacity(GOOGLE_USERINFO_MAX_BYTES.min(4096));
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        error!(error = ?e, "Failed to read Google user info");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to fetch user info from Google")
    })? {
        if body.len().saturating_add(chunk.len()) > GOOGLE_USERINFO_MAX_BYTES {
            error!("Google UserInfo response exceeded size limit");
            return Err(ErrorResponse::new(ErrorCode::ExternalServiceError)
                .with_message("Failed to fetch user info from Google"));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body).map_err(|e: serde_json::Error| {
        error!(error = ?e, "Failed to parse user info from Google");
        ErrorResponse::new(ErrorCode::ExternalServiceError)
            .with_message("Failed to parse user info from Google")
    })
}

async fn find_or_create_user(
    state: &AppState,
    user_info: GoogleUserInfo,
) -> Result<user::Model, ErrorResponse> {
    if !user_info.verified_email {
        warn!("Refusing Google login: IdP email is not verified");
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("Google has not verified this email address"));
    }

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

    info!("Creating new user from Google account");
    user::Entity::create_from_google(
        &state.sea_db,
        user_info.id.clone(),
        user_info.email.clone(),
        user_info.name.clone(),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::auth_v1::controller::{session_rotated_headers, SESSION_ROTATED};
    use axum::http::header::ORIGIN;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mobile_claims() -> GoogleIdTokenClaims {
        GoogleIdTokenClaims {
            sub: "google-sub-123".to_string(),
            email: "aisha@example.com".to_string(),
            email_verified: Some(true),
            name: Some("Aisha Khan".to_string()),
            picture: Some("https://profiles.google.test/aisha.jpg".to_string()),
            nonce: None,
        }
    }

    #[test]
    fn id_token_profile_is_used_when_access_token_is_absent() {
        let claims = mobile_claims();

        let profile = google_user_info_from_claims(&claims);

        assert_eq!(profile.name, "Aisha Khan");
        assert_eq!(
            profile.picture.as_deref(),
            Some("https://profiles.google.test/aisha.jpg")
        );
    }

    #[test]
    fn userinfo_subject_mismatch_is_rejected() {
        let claims = mobile_claims();
        let user_info = GoogleUserInfo {
            id: "different-google-sub".to_string(),
            email: claims.email.clone(),
            name: "Aisha Khan".to_string(),
            picture: None,
            verified_email: true,
        };

        let result = reconcile_google_user_info(&claims, user_info);

        assert!(
            result.is_err(),
            "mismatched Google subject must be rejected"
        );
    }

    #[test]
    fn userinfo_email_mismatch_is_rejected() {
        let claims = mobile_claims();
        let user_info = GoogleUserInfo {
            id: claims.sub.clone(),
            email: "different@example.com".to_string(),
            name: "Aisha Khan".to_string(),
            picture: None,
            verified_email: true,
        };

        let result = reconcile_google_user_info(&claims, user_info);

        assert!(result.is_err(), "mismatched Google email must be rejected");
    }

    #[test]
    fn unverified_id_token_email_is_rejected() {
        let mut claims = mobile_claims();
        claims.email_verified = Some(false);
        let user_info = google_user_info_from_claims(&claims);

        let result = reconcile_google_user_info(&claims, user_info);

        assert!(result.is_err(), "unverified Google email must be rejected");
    }

    #[test]
    fn unverified_userinfo_email_is_rejected() {
        let claims = mobile_claims();
        let mut user_info = google_user_info_from_claims(&claims);
        user_info.verified_email = false;

        let result = reconcile_google_user_info(&claims, user_info);

        assert!(
            result.is_err(),
            "unverified UserInfo email must be rejected"
        );
    }

    #[test]
    fn native_token_request_rejects_browser_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(ORIGIN, "https://easyquran.fyi".parse().unwrap());

        let result = oauth::ensure_native_token_request(&headers);

        assert!(
            result.is_err(),
            "browser-origin token exchange must be rejected"
        );
    }

    #[test]
    fn native_token_request_accepts_missing_origin() {
        let result = oauth::ensure_native_token_request(&HeaderMap::new());

        assert!(
            result.is_ok(),
            "native requests do not send an Origin header"
        );
    }

    #[tokio::test]
    async fn userinfo_request_uses_bearer_token_and_decodes_profile() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .and(header("authorization", "Bearer fake-google-access-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "google-sub-123",
                "email": "aisha@example.com",
                "name": "Aisha Khan",
                "picture": "https://profiles.google.test/aisha.jpg",
                "verified_email": true
            })))
            .mount(&server)
            .await;
        let client = google_userinfo_http_client().unwrap();

        let result = fetch_google_user_info_from_url(
            client,
            &format!("{}/userinfo", server.uri()),
            "fake-google-access-token",
        )
        .await
        .unwrap();

        assert_eq!(result.name, "Aisha Khan");
    }

    #[tokio::test]
    async fn userinfo_unauthorized_response_is_invalid_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = google_userinfo_http_client().unwrap();

        let error = fetch_google_user_info_from_url(
            client,
            &format!("{}/userinfo", server.uri()),
            "expired-google-access-token",
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::InvalidToken);
    }

    #[tokio::test]
    async fn userinfo_redirect_is_not_followed() {
        let target = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/leak-target"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&target)
            .await;

        let source = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/userinfo"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("location", format!("{}/leak-target", target.uri())),
            )
            .mount(&source)
            .await;
        let client = google_userinfo_http_client().unwrap();

        let error = fetch_google_user_info_from_url(
            client,
            &format!("{}/userinfo", source.uri()),
            "must-not-cross-redirect",
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::ExternalServiceError);
    }

    // W8B-003: regression guard that the rotation header is emitted on the
    // success path of a session-rotating endpoint (login / OAuth exchange).
    // finish_oauth_login cycles the session id, so the exchange response must
    // carry X-EQ-Session-Rotated or the web client's CSRF token goes stale.
    #[test]
    fn session_rotated_header_present_on_successful_rotation() {
        let headers = session_rotated_headers(true);
        assert_eq!(
            headers.get(&SESSION_ROTATED).map(|v| v.to_str().unwrap()),
            Some("1"),
            "successful rotation must emit the X-EQ-Session-Rotated header"
        );

        let headers = session_rotated_headers(false);
        assert!(
            !headers.contains_key(&SESSION_ROTATED),
            "non-rotating response must omit the header"
        );
    }
}
