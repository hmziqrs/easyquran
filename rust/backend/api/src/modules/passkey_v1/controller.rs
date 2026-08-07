use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use rux_auth::AuthBackend as AuthBackendTrait;
use serde_json::json;
use tracing::{info, instrument, warn};

use crate::db::sea_models::{passkey_credential, user_session};
use crate::error::{ErrorCode, ErrorResponse};
use crate::extractors::ValidatedJson;
use crate::modules::auth_v1::controller::record_session_mapping;
use crate::modules::passkey_v1::validator::{
    V1LoginFinishPayload, V1RegisterFinishPayload, V1RemovePasskeyPayload,
};
use crate::services::auth::AuthSession;
use crate::AppState;

fn webauthn_service(
    state: &AppState,
) -> Result<&crate::services::webauthn::WebauthnService, ErrorResponse> {
    state.webauthn.as_deref().ok_or_else(|| {
        ErrorResponse::new(ErrorCode::ServiceUnavailable)
            .with_message("Passkey authentication is not configured")
    })
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id))]
pub async fn register_begin(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    tracing::Span::current().record("user_id", user.id);

    let svc = webauthn_service(&state)?;
    let (challenge, registration_state) = svc.start_registration(user)?;

    info!(user_id = user.id, "passkey registration begun");
    Ok((
        StatusCode::OK,
        Json(json!({
            "challenge": challenge,
            "registration_state": registration_state,
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id))]
pub async fn register_finish(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1RegisterFinishPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    tracing::Span::current().record("user_id", user.id);
    let payload = payload.0;

    let svc = webauthn_service(&state)?;
    let model = svc
        .finish_registration(
            &state.sea_db,
            user.id,
            &payload.credential,
            &payload.registration_state,
            payload.device_type.clone(),
            payload.transports.clone(),
        )
        .await?;

    info!(
        user_id = user.id,
        credential_id = %model.credential_id,
        "passkey registered"
    );
    Ok((StatusCode::CREATED, Json(json!(model.into_view()))))
}

#[debug_handler]
#[instrument(skip(state, auth), fields(user_id))]
pub async fn list(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    tracing::Span::current().record("user_id", user.id);

    let rows = passkey_credential::Entity::list_by_user(&state.sea_db, user.id).await?;
    let views: Vec<_> = rows
        .into_iter()
        .map(passkey_credential::Model::into_view)
        .collect();
    Ok((StatusCode::OK, Json(json!({ "data": views }))))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id))]
pub async fn remove(
    State(state): State<AppState>,
    auth: AuthSession,
    payload: ValidatedJson<V1RemovePasskeyPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let user = auth.user_required()?;
    tracing::Span::current().record("user_id", user.id);
    let payload = payload.0;

    let removed = passkey_credential::Entity::delete_by_credential_id_for_user(
        &state.sea_db,
        &payload.credential_id,
        user.id,
    )
    .await?;

    if removed == 0 {
        return Err(ErrorResponse::new(ErrorCode::RecordNotFound)
            .with_message("Passkey credential not found"));
    }
    info!(user_id = user.id, credential_id = %payload.credential_id, "passkey removed");
    Ok((
        StatusCode::OK,
        Json(json!({ "message": "Passkey removed" })),
    ))
}

#[debug_handler]
#[instrument(skip(state))]
pub async fn login_begin(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let svc = webauthn_service(&state)?;
    let (challenge, authentication_state) = svc.start_login()?;

    info!("passkey login begun");
    Ok((
        StatusCode::OK,
        Json(json!({
            "challenge": challenge,
            "authentication_state": authentication_state,
        })),
    ))
}

#[debug_handler]
#[instrument(skip(state, auth, payload), fields(user_id, result))]
pub async fn login_finish(
    State(state): State<AppState>,
    mut auth: AuthSession,
    payload: ValidatedJson<V1LoginFinishPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let payload = payload.0;

    let svc = webauthn_service(&state)?;
    let (credential, user) = svc
        .finish_login(
            &state.sea_db,
            &payload.credential,
            &payload.authentication_state,
        )
        .await?;
    tracing::Span::current().record("user_id", user.id);

    // Fail-closed: a banned user must not obtain a session via passkey.
    match auth.backend().check_ban(&user.id).await {
        Ok(ban_status) if ban_status.is_banned() => {
            warn!(user_id = user.id, "passkey login: banned user");
            return Err(ErrorResponse::new(ErrorCode::AccountLocked)
                .with_message("This account has been banned"));
        }
        Ok(_) => {}
        Err(_) => {
            return Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Unable to verify account status"));
        }
    }

    let device = credential.device_type.clone();
    let ip: Option<String> = None;
    match auth
        .login_with_metadata(&user, device.clone(), ip.clone())
        .await
    {
        Ok(_) => {
            info!(
                user_id = user.id,
                credential_id = %credential.credential_id,
                "passkey login successful, full session issued"
            );
            tracing::Span::current().record("result", "success");

            let session_row = user_session::Entity::create(
                &state.sea_db,
                user_session::NewUserSession::new(user.id, device, ip),
            )
            .await
            .ok();

            // Record session-row -> tower-session-id so sessions_terminate can invalidate the live session.
            if (auth.session().save().await).is_ok() {
                if let (Some(row), Some(tower_sid)) = (session_row.as_ref(), auth.session().id()) {
                    record_session_mapping(row.id, &tower_sid.to_string());
                }
            }

            Ok((
                StatusCode::OK,
                Json(json!({ "status": "ok", "user": user })),
            ))
        }
        Err(err) => {
            warn!(error = %err, user_id = user.id, "passkey login: session creation failed");
            tracing::Span::current().record("result", "session_error");
            Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("An error occurred while logging in")
                .with_details(err.to_string()))
        }
    }
}
