use sea_orm::{ActiveModelTrait, Set};
use tracing::{error, info, instrument, warn};

use crate::{
    db::sea_models::{user, user_oauth_identity},
    error::{ErrorCode, ErrorResponse},
    modules::auth_v1::controller::create_bound_session,
    services::auth::AuthSession,
    AppState,
};

use user_oauth_identity::NewOauthIdentity;

/// Supported OAuth identity providers. The DB key (`as_str`) must stay stable — it is stored in
/// `user_oauth_identity.provider` and `users.oauth_provider`, so renaming it would orphan existing links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    Apple,
    Facebook,
    Github,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Apple => "apple",
            OAuthProvider::Facebook => "facebook",
            OAuthProvider::Github => "github",
        }
    }

    /// Human-readable label for audit/session rows (e.g. "Google OAuth").
    pub fn label(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "Google",
            OAuthProvider::Apple => "Apple",
            OAuthProvider::Facebook => "Facebook",
            OAuthProvider::Github => "GitHub",
        }
    }
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `email_verified` must be true before linking onto or creating an account: an unverified-at-IdP identity with a victim's email must not take it over.
#[allow(clippy::too_many_arguments)]
#[instrument(skip(state), fields(provider = %provider))]
pub async fn find_or_create_user_for_oauth(
    state: &AppState,
    provider: OAuthProvider,
    provider_user_id: &str,
    email: String,
    name: String,
    email_verified: bool,
) -> Result<user::Model, ErrorResponse> {
    if let Some(identity) = user_oauth_identity::Entity::find_by_provider(
        &state.sea_db,
        provider.as_str(),
        provider_user_id,
    )
    .await?
    {
        let user = user::Entity::find_by_id_with_404(&state.sea_db, identity.user_id).await?;
        info!(
            user_id = user.id,
            "Existing user found by OAuth identity link"
        );
        return Ok(user);
    }

    if let Some(existing) = user::Entity::find_by_email(&state.sea_db, email.clone()).await? {
        if !email_verified {
            warn!(user_id = existing.id, "Refusing to link OAuth account: IdP email is not verified");
            return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("Unable to link this account"));
        }
        info!(
            user_id = existing.id,
            "Linking OAuth account to existing user"
        );
        link_identity(
            &state.sea_db,
            existing.id,
            provider.as_str(),
            provider_user_id,
        )
        .await?;
        return Ok(existing);
    }

    if !email_verified {
        warn!("Refusing to create account from OAuth: IdP email is not verified");
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("The provider has not verified this email address"));
    }

    info!("Creating new user from OAuth account");
    let now = chrono::Utc::now().fixed_offset();
    let active = user::ActiveModel {
        name: Set(name),
        email: Set(email),
        password: Set(None),
        oauth_provider: Set(Some(provider.to_string())),
        role: Set(user::UserRole::User),
        is_verified: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    let user = active.insert(&state.sea_db).await.map_err(|e| {
        error!(error = ?e, "Failed to create user from OAuth");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to create user account")
    })?;

    link_identity(&state.sea_db, user.id, provider.as_str(), provider_user_id).await?;
    tracing::Span::current().record("user_id", user.id);
    Ok(user)
}

async fn link_identity(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    provider: &str,
    provider_user_id: &str,
) -> Result<(), ErrorResponse> {
    let now = chrono::Utc::now().fixed_offset();
    user_oauth_identity::Entity::link(
        db,
        NewOauthIdentity {
            user_id,
            provider: provider.to_string(),
            provider_user_id: provider_user_id.to_string(),
            created_at: now,
        },
    )
    .await
    .map(|_| ())
}

pub async fn finish_oauth_login(
    state: &AppState,
    auth: &mut AuthSession,
    user: &user::Model,
    provider: OAuthProvider,
) -> Result<(), ErrorResponse> {
    auth.login(user).await.map_err(|e| {
        error!(error = %e, user_id = user.id, "Failed to create OAuth session");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Failed to create session")
    })?;

    // W8e: durable binding with the same fail-closed contract as password login
    // (create audit row -> save tower session -> record binding). On failure the
    // tower session is destroyed and the audit row revoked so neither lingers.
    let device = Some(format!("{} OAuth", provider.label()));
    let ip: Option<String> = None;
    create_bound_session(&state.sea_db, auth, user.id, device, ip).await
}

pub fn generate_oauth_nonce() -> String {
    use base64::Engine;
    use rand::Rng;
    let mut bytes = [0u8; 16];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
