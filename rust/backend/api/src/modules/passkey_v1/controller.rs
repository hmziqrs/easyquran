use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_macros::debug_handler;
use rux_auth::AuthBackend as AuthBackendTrait;
use serde_json::json;
use tracing::{info, instrument, warn};

use crate::db::sea_models::passkey_credential;
use crate::error::{ErrorCode, ErrorResponse};
use crate::extractors::ValidatedJson;
use crate::modules::auth_v1::controller::create_bound_session;
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

            // W8e: durable binding with the same fail-closed contract as password
            // login. On failure create_bound_session destroys the tower session and
            // revokes the audit row so neither lingers unbound.
            match create_bound_session(&state.sea_db, &mut auth, user.id, device, ip).await {
                Ok(()) => Ok((
                    StatusCode::OK,
                    Json(json!({ "status": "ok", "user": user })),
                )),
                Err(err) => {
                    tracing::Span::current().record("result", "session_error");
                    Err(err)
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sea_models::user;
    use crate::services::webauthn::WebauthnService;
    use chrono::TimeZone;
    use validator::Validate;

    fn make_user() -> user::Model {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .unwrap()
            .fixed_offset();
        user::Model {
            id: 1,
            name: "Pk User".to_string(),
            email: "pk@example.com".to_string(),
            password: None,
            avatar_id: None,
            is_verified: true,
            role: user::UserRole::User,
            two_fa_enabled: false,
            two_fa_secret: None,
            two_fa_backup_codes: None,
            two_fa_last_totp_counter: None,
            google_id: None,
            oauth_provider: None,
            session_auth_secret: "test-secret".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    fn svc() -> WebauthnService {
        WebauthnService::new("example.com", "https://example.com", "Test")
            .expect("dev WebAuthn service must construct for a real origin")
    }

    #[test]
    fn login_begin_envelope_shape_matches_wire_contract() {
        let svc = svc();
        let (challenge, authentication_state) = svc.start_login().unwrap();
        let body = json!({
            "challenge": challenge,
            "authentication_state": authentication_state,
        });
        assert!(body.get("challenge").is_some(), "must carry challenge");
        assert!(
            body.get("authentication_state").is_some(),
            "must carry authentication_state for the client to echo back"
        );
        assert!(
            body["challenge"].get("publicKey").is_some(),
            "RequestChallengeResponse must wrap options under publicKey"
        );
    }

    #[test]
    fn register_begin_envelope_shape_matches_wire_contract() {
        let svc = svc();
        let (challenge, registration_state) = svc.start_registration(&make_user()).unwrap();
        let body = json!({
            "challenge": challenge,
            "registration_state": registration_state,
        });
        assert!(body.get("challenge").is_some(), "must carry challenge");
        assert!(
            body.get("registration_state").is_some(),
            "must carry registration_state for the client to echo back"
        );
        assert!(
            body["challenge"].get("publicKey").is_some(),
            "CreationChallengeResponse must wrap options under publicKey"
        );
    }

    #[test]
    fn login_finish_payload_round_trips_real_authentication_state() {
        let svc = svc();
        let (_challenge, authentication_state) = svc.start_login().unwrap();
        let state_value = serde_json::to_value(&authentication_state).unwrap();

        let body = json!({
            "credential": {
                "id": "cred-1",
                "rawId": "AA",
                "type": "public-key",
                "response": {
                    "clientDataJSON": "AA",
                    "authenticatorData": "AA",
                    "signature": "AA"
                }
            },
            "authentication_state": state_value
        });
        let payload: V1LoginFinishPayload =
            serde_json::from_value(body).expect("real authentication_state must round-trip");
        assert_eq!(payload.credential.type_, "public-key");
    }

    #[test]
    fn register_finish_payload_round_trips_real_registration_state() {
        let svc = svc();
        let (_challenge, registration_state) = svc.start_registration(&make_user()).unwrap();
        let state_value = serde_json::to_value(&registration_state).unwrap();

        let body = json!({
            "credential": {
                "id": "cred-2",
                "rawId": "AA",
                "type": "public-key",
                "response": {
                    "clientDataJSON": "AA",
                    "attestationObject": "AA"
                }
            },
            "registration_state": state_value,
            "device_type": "MacBook",
            "transports": ["internal"]
        });
        let payload: V1RegisterFinishPayload =
            serde_json::from_value(body).expect("real registration_state must round-trip");
        assert_eq!(payload.credential.type_, "public-key");
        assert_eq!(payload.device_type.as_deref(), Some("MacBook"));
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn list_envelope_uses_data_key() {
        // controller returns json!({ "data": views }) — NOT { "passkeys": [...] }.
        let view = passkey_credential::Model {
            id: 1,
            user_id: 5,
            credential_id: "cred-1".to_string(),
            public_key: Vec::new(),
            counter: 0,
            device_type: None,
            transports: None,
            created_at: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap().fixed_offset(),
            last_used_at: None,
        }
        .into_view();
        let body = json!({ "data": [view] });
        assert!(body.get("data").is_some(), "list envelope must use the data key");
        assert!(
            body.get("passkeys").is_none(),
            "list envelope must not use the legacy passkeys key"
        );
        assert_eq!(body["data"][0]["credential_id"].as_str(), Some("cred-1"));
        assert!(
            body["data"][0].get("public_key").is_none(),
            "credential view must never expose public_key"
        );
    }

    #[test]
    fn remove_envelope_uses_message_key() {
        let body = json!({ "message": "Passkey removed" });
        assert_eq!(body["message"].as_str(), Some("Passkey removed"));
        assert!(
            body.get("data").is_none(),
            "remove envelope is a message, not a list"
        );
    }

    #[test]
    fn login_finish_success_envelope_wraps_user() {
        let user = make_user();
        let body = json!({ "status": "ok", "user": user });
        assert_eq!(body["status"].as_str(), Some("ok"));
        assert_eq!(body["user"]["email"].as_str(), Some("pk@example.com"));
    }
}
