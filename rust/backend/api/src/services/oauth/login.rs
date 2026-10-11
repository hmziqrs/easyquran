use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use tracing::{error, info, instrument, warn};

use rux_auth::AuthBackend as AuthBackendTrait;

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
    Discord,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Apple => "apple",
            OAuthProvider::Facebook => "facebook",
            OAuthProvider::Github => "github",
            OAuthProvider::Discord => "discord",
        }
    }

    /// Human-readable label for audit/session rows (e.g. "Google OAuth").
    pub fn label(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "Google",
            OAuthProvider::Apple => "Apple",
            OAuthProvider::Facebook => "Facebook",
            OAuthProvider::Github => "GitHub",
            OAuthProvider::Discord => "Discord",
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
#[instrument(skip(db), fields(provider = %provider))]
pub async fn find_or_create_user_for_oauth(
    db: &sea_orm::DatabaseConnection,
    provider: OAuthProvider,
    provider_user_id: &str,
    email: Option<String>,
    name: String,
    email_verified: bool,
) -> Result<user::Model, ErrorResponse> {
    if let Some(identity) =
        user_oauth_identity::Entity::find_by_provider(db, provider.as_str(), provider_user_id)
            .await?
    {
        let user = user::Entity::find_by_id_with_404(db, identity.user_id).await?;
        info!(
            user_id = user.id,
            "Existing user found by OAuth identity link"
        );
        return Ok(user);
    }

    if provider == OAuthProvider::Google {
        if let Some(account) =
            user::Entity::find_by_google_id(db, provider_user_id.to_string()).await?
        {
            let outcome = link_identity_to_user(db, account.id, provider, provider_user_id).await?;
            if outcome == LinkOutcome::AlreadyLinkedOtherUser {
                return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
                    .with_message("Google identity is linked to another account"));
            }
            return Ok(account);
        }
    }

    let email = email
        .filter(|value| !value.trim().is_empty())
        .map(|value| crate::utils::email::normalize_email(&value))
        .ok_or_else(|| {
            warn!("OAuth provider returned no email for an unlinked identity");
            ErrorResponse::new(ErrorCode::OperationNotAllowed)
                .with_message("The provider did not provide an email address")
        })?;
    ensure_verified_oauth_email(email_verified)?;

    if user::Entity::find_by_email(db, email.clone())
        .await?
        .is_some()
    {
        return Err(
            ErrorResponse::new(ErrorCode::OperationNotAllowed).with_message(
                "Sign in with your existing method, then link this provider from Account",
            ),
        );
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
    let transaction = db.begin().await?;
    let user = active.insert(&transaction).await.map_err(|e| {
        error!(error = ?e, "Failed to create user from OAuth");
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Failed to create user account")
    })?;

    link_identity(&transaction, user.id, provider.as_str(), provider_user_id).await?;
    transaction.commit().await?;
    tracing::Span::current().record("user_id", user.id);
    Ok(user)
}

fn ensure_verified_oauth_email(email_verified: bool) -> Result<(), ErrorResponse> {
    if !email_verified {
        warn!("Refusing OAuth login: IdP email is not verified");
        return Err(ErrorResponse::new(ErrorCode::OperationNotAllowed)
            .with_message("The provider has not verified this email address"));
    }
    Ok(())
}

async fn link_identity<T: ConnectionTrait>(
    db: &T,
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

/// Result of attaching a provider identity to an already-authenticated account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    Linked,
    AlreadyLinkedSameUser,
    AlreadyLinkedOtherUser,
}

fn classify_existing_link(identity_user_id: i32, user_id: i32) -> LinkOutcome {
    if identity_user_id == user_id {
        LinkOutcome::AlreadyLinkedSameUser
    } else {
        LinkOutcome::AlreadyLinkedOtherUser
    }
}

/// Attaches a provider identity without changing sessions or provider metadata.
/// Rechecks duplicate identities to distinguish existing ownership.
pub async fn link_identity_to_user<T: ConnectionTrait>(
    db: &T,
    user_id: i32,
    provider: OAuthProvider,
    provider_user_id: &str,
) -> Result<LinkOutcome, ErrorResponse> {
    if provider == OAuthProvider::Google {
        if let Some(account) =
            user::Entity::find_by_google_id(db, provider_user_id.to_string()).await?
        {
            if account.id != user_id {
                return Ok(LinkOutcome::AlreadyLinkedOtherUser);
            }
        }
    }
    if let Some(identity) =
        user_oauth_identity::Entity::find_by_provider(db, provider.as_str(), provider_user_id)
            .await?
    {
        return Ok(classify_existing_link(identity.user_id, user_id));
    }

    match link_identity(db, user_id, provider.as_str(), provider_user_id).await {
        Ok(()) => Ok(LinkOutcome::Linked),
        Err(err) => {
            // Race: a concurrent callback may have inserted the same identity between the read
            // above and this insert. Re-check and map; anything else stays a hard error.
            match user_oauth_identity::Entity::find_by_provider(
                db,
                provider.as_str(),
                provider_user_id,
            )
            .await?
            {
                Some(identity) => Ok(classify_existing_link(identity.user_id, user_id)),
                None => Err(err),
            }
        }
    }
}

/// Keeps the confirmed credential and factor state locked through identity linking.
pub async fn link_identity_to_unchanged_user(
    db: &sea_orm::DatabaseConnection,
    confirmed: &user::Model,
    provider: OAuthProvider,
    provider_user_id: &str,
) -> Result<LinkOutcome, ErrorResponse> {
    let transaction = db.begin().await?;
    let password_matches = match &confirmed.password {
        Some(password) => user::Column::Password.eq(password.clone()),
        None => user::Column::Password.is_null(),
    };
    let factor_matches = match &confirmed.two_fa_secret {
        Some(secret) => user::Column::TwoFaSecret.eq(secret.clone()),
        None => user::Column::TwoFaSecret.is_null(),
    };
    let locked = user::Entity::update_many()
        .col_expr(
            user::Column::UpdatedAt,
            sea_orm::sea_query::Expr::col(user::Column::UpdatedAt).into(),
        )
        .filter(user::Column::Id.eq(confirmed.id))
        .filter(user::Column::SessionAuthSecret.eq(confirmed.session_auth_secret.clone()))
        .filter(password_matches)
        .filter(user::Column::TwoFaEnabled.eq(confirmed.two_fa_enabled))
        .filter(factor_matches)
        .exec(&transaction)
        .await?;
    if locked.rows_affected != 1 {
        return Err(crate::services::fresh_auth::required());
    }
    let outcome =
        link_identity_to_user(&transaction, confirmed.id, provider, provider_user_id).await?;
    transaction.commit().await?;
    Ok(outcome)
}

/// Outcome of a provider web callback: a login session for the given user id, a successful link
/// onto the already-authenticated account, or a link refused because the identity belongs to
/// another user.
#[derive(Debug)]
pub enum CallbackOutcome {
    Login(i32),
    TotpRequired,
    Linked,
    AlreadyLinkedOtherUser,
}

impl From<LinkOutcome> for CallbackOutcome {
    fn from(outcome: LinkOutcome) -> Self {
        match outcome {
            LinkOutcome::Linked | LinkOutcome::AlreadyLinkedSameUser => CallbackOutcome::Linked,
            LinkOutcome::AlreadyLinkedOtherUser => CallbackOutcome::AlreadyLinkedOtherUser,
        }
    }
}

/// Callback error plus whether the flow carried link intent, so the controller redirects to the
/// login failure path or the `/account` link-error path.
pub struct CallbackFailure {
    pub link_mode: bool,
    pub error: ErrorResponse,
}

impl CallbackFailure {
    /// Failure before link intent is known (session/state handling) — always a login failure.
    pub fn login(error: ErrorResponse) -> Self {
        Self {
            link_mode: false,
            error,
        }
    }
}

pub async fn finish_oauth_login(
    state: &AppState,
    auth: &mut AuthSession,
    user: &user::Model,
    provider: OAuthProvider,
) -> Result<(), ErrorResponse> {
    finish_oauth_login_with_db(&state.sea_db, auth, user, provider).await
}

pub async fn finish_oauth_login_with_db(
    db: &sea_orm::DatabaseConnection,
    auth: &mut AuthSession,
    user: &user::Model,
    provider: OAuthProvider,
) -> Result<(), ErrorResponse> {
    match auth.backend().check_ban(&user.id).await {
        Ok(ban_status) if ban_status.is_banned() => {
            warn!(user_id = user.id, "Banned user attempted OAuth login");
            return Err(ErrorResponse::new(ErrorCode::AccountLocked)
                .with_message("This account has been banned"));
        }
        Ok(_) => {}
        Err(_) => {
            return Err(ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Unable to verify account status"));
        }
    }

    if user.two_fa_enabled {
        if auth.user.is_some() {
            auth.logout()
                .await
                .map_err(|_| ErrorResponse::new(ErrorCode::InternalServerError))?;
        }
        crate::services::pending_login::issue(auth, user).await?;
        return Ok(());
    }

    auth.login(user).await.map_err(|e| {
        error!(error = %e, user_id = user.id, "Failed to create OAuth session");
        ErrorResponse::new(ErrorCode::InternalServerError).with_message("Failed to create session")
    })?;

    let device = Some(format!("{} OAuth", provider.label()));
    let ip: Option<String> = None;
    create_bound_session(db, auth, user.id, device, ip).await
}

pub async fn callback_login_outcome(
    auth: &AuthSession,
    user_id: i32,
) -> Result<CallbackOutcome, ErrorResponse> {
    if crate::services::pending_login::response(auth)
        .await?
        .is_some()
    {
        return Ok(CallbackOutcome::TotpRequired);
    }
    if auth.user.as_ref().is_some_and(|user| user.id == user_id) {
        return Ok(CallbackOutcome::Login(user_id));
    }
    Err(ErrorResponse::new(ErrorCode::Unauthorized))
}

pub fn generate_oauth_nonce() -> String {
    use base64::Engine;
    use rand::Rng;
    let mut bytes = [0u8; 16];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_email_cannot_link_or_create_oauth_account() {
        let result = ensure_verified_oauth_email(false);

        assert!(result.is_err(), "unverified email must not link or create");
    }

    async fn mem_db() -> sea_orm::DatabaseConnection {
        use migration::{Migrator, MigratorTrait};
        use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

        let test_key: [u8; 32] =
            std::array::from_fn(|i| (i as u8).wrapping_mul(7).wrapping_add(0x11));
        crate::utils::field_crypto::set_key(&test_key).ok();
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db: DatabaseConnection = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        // No migration owns user_oauth_identities yet (pre-existing repo gap);
        // create it to match the sea-orm model so the lookup path is testable.
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS user_oauth_identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                provider TEXT NOT NULL,
                provider_user_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE (provider, provider_user_id)
            )",
        )
        .await
        .unwrap();
        db
    }

    async fn insert_user(db: &sea_orm::DatabaseConnection, email: &str) -> user::Model {
        let now = chrono::Utc::now().fixed_offset();
        user::ActiveModel {
            name: Set("Test User".to_string()),
            email: Set(email.to_string()),
            password: Set(Some("password-hash".to_string())),
            role: Set(user::UserRole::User),
            is_verified: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn unverified_provider_email_never_links_existing_password_account() {
        let db = mem_db().await;

        let now = chrono::Utc::now().fixed_offset();
        let victim = user::ActiveModel {
            name: Set("Victim".to_string()),
            email: Set("victim@corp.com".to_string()),
            password: Set(Some("password-hash".to_string())),
            role: Set(user::UserRole::User),
            is_verified: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        // Facebook-style attempt: provider asserts an email it never verified.
        // find_or_create_user_for_oauth must refuse before any link/create, so
        // finish_oauth_login (and its session) is never reached for the victim.
        let error = find_or_create_user_for_oauth(
            &db,
            OAuthProvider::Facebook,
            "attacker-fb-id",
            Some("victim@corp.com".to_string()),
            "Attacker".to_string(),
            false,
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, ErrorCode::OperationNotAllowed);

        let reloaded = user::Entity::find_by_email(&db, "victim@corp.com".to_string())
            .await
            .unwrap()
            .expect("victim account still exists");
        assert_eq!(reloaded.id, victim.id);
        assert!(
            reloaded.oauth_provider.is_none(),
            "victim account must not be marked as OAuth-owned"
        );
        assert!(
            user_oauth_identity::Entity::find_by_provider(&db, "facebook", "attacker-fb-id")
                .await
                .unwrap()
                .is_none(),
            "no identity link row may be created for the attacker"
        );
    }

    #[tokio::test]
    async fn link_identity_links_new_identity_to_user() {
        let db = mem_db().await;
        let user = insert_user(&db, "linker@example.com").await;

        let outcome = link_identity_to_user(&db, user.id, OAuthProvider::Github, "gh-1")
            .await
            .unwrap();

        assert_eq!(outcome, LinkOutcome::Linked);
        let identity = user_oauth_identity::Entity::find_by_provider(&db, "github", "gh-1")
            .await
            .unwrap()
            .expect("identity link row exists");
        assert_eq!(identity.user_id, user.id);
    }

    #[tokio::test]
    async fn link_identity_same_user_is_idempotent() {
        let db = mem_db().await;
        let user = insert_user(&db, "same@example.com").await;
        link_identity_to_user(&db, user.id, OAuthProvider::Discord, "dc-1")
            .await
            .unwrap();

        let outcome = link_identity_to_user(&db, user.id, OAuthProvider::Discord, "dc-1")
            .await
            .unwrap();

        assert_eq!(outcome, LinkOutcome::AlreadyLinkedSameUser);
    }

    #[tokio::test]
    async fn link_identity_other_user_is_rejected() {
        let db = mem_db().await;
        let owner = insert_user(&db, "owner@example.com").await;
        let attacker = insert_user(&db, "attacker@example.com").await;
        link_identity_to_user(&db, owner.id, OAuthProvider::Google, "g-1")
            .await
            .unwrap();

        let outcome = link_identity_to_user(&db, attacker.id, OAuthProvider::Google, "g-1")
            .await
            .unwrap();

        assert_eq!(outcome, LinkOutcome::AlreadyLinkedOtherUser);
        let identity = user_oauth_identity::Entity::find_by_provider(&db, "google", "g-1")
            .await
            .unwrap()
            .expect("identity link row exists");
        assert_eq!(identity.user_id, owner.id, "existing link must stay put");
    }
}
