use axum::{extract::Request, middleware::Next, response::Response, Extension};
use rux_auth::{auth_requirements, check_requirements, AuthError, AuthSession};
use tower_sessions::Session;

use crate::{services::auth::AuthBackend, AppState};

// The revoked-session set must be passed to AuthBackend so the per-request
// is_session_revoked check runs; dropping it lets revoked sessions stay valid.
async fn make_auth_session(state: &AppState, session: Session) -> AuthSession<AuthBackend> {
    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    AuthSession::new(backend, session).await
}

// Must match user::UserRole::to_i32() ordering.
pub const ROLE_USER: i32 = 0;
pub const ROLE_AUTHOR: i32 = 1;
pub const ROLE_MODERATOR: i32 = 2;
pub const ROLE_ADMIN: i32 = 3;
pub const ROLE_SUPER_ADMIN: i32 = 4;

pub async fn authenticated(
    Extension(state): Extension<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let mut auth = make_auth_session(&state, session).await;
    check_requirements(&mut auth, &auth_requirements().authenticated().not_banned()).await?;
    Ok(next.run(request).await)
}

pub async fn unauthenticated(
    Extension(state): Extension<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let mut auth = make_auth_session(&state, session).await;
    check_requirements(&mut auth, &auth_requirements().unauthenticated()).await?;
    Ok(next.run(request).await)
}

pub async fn unverified(
    Extension(state): Extension<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let mut auth = make_auth_session(&state, session).await;
    check_requirements(&mut auth, &auth_requirements().authenticated().unverified()).await?;
    Ok(next.run(request).await)
}

pub async fn verified(
    Extension(state): Extension<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let mut auth = make_auth_session(&state, session).await;
    check_requirements(
        &mut auth,
        &auth_requirements().authenticated().verified().not_banned(),
    )
    .await?;
    Ok(next.run(request).await)
}

pub async fn verified_with_role<const LEVEL: i32>(
    Extension(state): Extension<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let mut auth = make_auth_session(&state, session).await;
    check_requirements(
        &mut auth,
        &auth_requirements()
            .authenticated()
            .verified()
            .not_banned()
            .role_min(LEVEL),
    )
    .await?;
    Ok(next.run(request).await)
}
