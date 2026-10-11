use axum::extract::Request;
use axum::response::Response;

use crate::error::{AuthError, AuthErrorCode};
use crate::requirements::AuthRequirements;
use crate::session::AuthSession;
use crate::traits::{AuthBackend, AuthUser};

pub async fn check_requirements<B: AuthBackend>(
    auth: &mut AuthSession<B>,
    requirements: &AuthRequirements,
) -> Result<(), AuthError> {
    if requirements.requires_unauth() && requirements.requires_auth() {
        return Err(AuthError::new(AuthErrorCode::PermissionDenied));
    }
    if requirements.authenticated == Some(false) {
        if auth.user.is_some() {
            return Err(AuthError::new(AuthErrorCode::AlreadyAuthenticated));
        }
        return Ok(());
    }

    if requirements.requires_auth() && auth.user.is_none() {
        return Err(AuthError::new(AuthErrorCode::Unauthenticated));
    }

    let (user, state) = match (&auth.user, &auth.state) {
        (Some(u), Some(s)) => (u.clone(), s.clone()),
        _ => {
            if !requirements.requires_auth() {
                return Ok(());
            }
            return Err(AuthError::new(AuthErrorCode::Unauthenticated));
        }
    };

    if requirements.unverified && user.email_verified() {
        return Err(AuthError::new(AuthErrorCode::AlreadyVerified)
            .with_message("This resource is for unverified users only"));
    }

    if requirements.verified && !user.email_verified() {
        return Err(AuthError::new(AuthErrorCode::VerificationRequired));
    }

    if requirements.not_banned {
        if state.ban_cache_stale(requirements.ban_cache_duration) {
            let ban_status = auth.backend().check_ban(&user.id()).await?;
            auth.update_ban_status(&ban_status).await?;
            if ban_status.is_banned() {
                return Err(AuthError::new(AuthErrorCode::Banned));
            }
        } else if state.is_banned {
            return Err(AuthError::new(AuthErrorCode::Banned));
        }
    }

    if let Some(min_role) = requirements.min_role {
        if user.role_level() < min_role {
            return Err(AuthError::new(AuthErrorCode::InsufficientRole)
                .with_context("required_role", min_role)
                .with_context("user_role", user.role_level()));
        }
    }

    Ok(())
}

pub async fn auth_guard_fn<B: AuthBackend>(
    mut auth: AuthSession<B>,
    requirements: AuthRequirements,
    request: Request,
    next: axum::middleware::Next,
) -> Result<Response, AuthError> {
    check_requirements(&mut auth, &requirements).await?;
    Ok(next.run(request).await)
}
