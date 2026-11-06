use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum_client_ip::ClientIp;
use serde_json::json;
use tower_sessions::Session;

use crate::{
    config::settings,
    error::{ErrorCode, ErrorResponse},
    middlewares::static_csrf::compute_csrf_token,
    services::registration_admission,
    AppState,
};

/// Route handler: charges the per-IP anonymous-session cap (A9) before a new
/// session is created, then issues the token. A request that already carries a
/// session is never charged.
pub async fn generate_capped(
    State(state): State<AppState>,
    ClientIp(client_ip): ClientIp,
    session: Session,
) -> Result<impl IntoResponse, ErrorResponse> {
    if session.id().is_none() {
        registration_admission::admit_anonymous_session(
            &state.gate_store,
            &client_ip.to_string(),
            settings::admission().anonymous_sessions_per_ip_daily,
            chrono::Utc::now(),
        )
        .await?;
    }
    generate(session).await
}

pub async fn generate(session: Session) -> Result<impl IntoResponse, ErrorResponse> {
    // The CSRF token binds to the session id, which only materializes after save(); the marker insert makes the cookie layer treat the session as non-empty. Fail closed if the store is unreachable.
    if session.id().is_none() {
        let _ = session.insert("csrf_issued", true).await;

        session
            .save()
            .await
            .map_err(|_| ErrorResponse::new(ErrorCode::InternalServerError))?;
    }

    let Some(id) = session.id() else {
        return Err(ErrorResponse::new(ErrorCode::InternalServerError));
    };

    let token = compute_csrf_token(&id.to_string());

    Ok((
        StatusCode::OK,
        Json(json!({"message": "csrf token generated successfully", "token": token})),
    ))
}
