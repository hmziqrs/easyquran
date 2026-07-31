use std::marker::PhantomData;
use std::task::{Context, Poll};

use axum::extract::{FromRef, Request};
use axum::response::Response;
use futures_util::future::BoxFuture;
use tower::{Layer, Service};

use crate::error::{AuthError, AuthErrorCode};
use crate::requirements::AuthRequirements;
use crate::session::AuthSession;
use crate::traits::{AuthBackend, AuthUser};

#[derive(Clone)]
pub struct AuthGuardLayer<B: AuthBackend> {
    requirements: AuthRequirements,
    _marker: PhantomData<B>,
}

impl<B: AuthBackend> AuthGuardLayer<B> {
    pub fn new(requirements: AuthRequirements) -> Self {
        Self {
            requirements,
            _marker: PhantomData,
        }
    }
}

impl<S, B: AuthBackend> Layer<S> for AuthGuardLayer<B> {
    type Service = AuthGuard<S, B>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthGuard {
            inner,
            requirements: self.requirements.clone(),
            _marker: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct AuthGuard<S, B: AuthBackend> {
    inner: S,
    requirements: AuthRequirements,
    _marker: PhantomData<B>,
}

impl<S, B> Service<Request> for AuthGuard<S, B>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
    B: AuthBackend + FromRef<()> + Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let inner = self.inner.clone();
        let _requirements = self.requirements.clone();

        Box::pin(async move {
            let mut inner = inner;
            inner.call(req).await
        })
    }
}

pub fn auth_guard<B: AuthBackend>(requirements: AuthRequirements) -> AuthGuardLayer<B> {
    AuthGuardLayer::new(requirements)
}

pub async fn check_requirements<B: AuthBackend>(
    auth: &mut AuthSession<B>,
    requirements: &AuthRequirements,
) -> Result<(), AuthError> {
    if requirements.authenticated == Some(false) {
        if auth.user.is_some() {
            return Err(AuthError::new(AuthErrorCode::AlreadyAuthenticated));
        }
        return Ok(());
    }

    if requirements.authenticated == Some(true) && auth.user.is_none() {
        return Err(AuthError::new(AuthErrorCode::Unauthenticated));
    }

    let (user, state) = match (&auth.user, &auth.state) {
        (Some(u), Some(s)) => (u.clone(), s.clone()),
        _ => {
            if requirements.authenticated != Some(true) {
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
