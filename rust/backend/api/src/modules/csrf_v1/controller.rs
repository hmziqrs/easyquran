use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use tower_sessions::Session;

use crate::{
    error::{ErrorCode, ErrorResponse},
    middlewares::static_csrf::compute_csrf_token,
};

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
