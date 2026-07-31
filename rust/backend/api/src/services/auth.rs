use async_trait::async_trait;
use password_auth::verify_password;
use rux_auth::{
    AuthBackend as RuxAuthBackend, AuthError, AuthErrorCode, AuthUser, BanStatus, SessionRevocation,
};
use sea_orm::DatabaseConnection;
use tower_sessions::SessionStore;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Instant;
use tokio::task;
use tracing::{error, info, instrument, warn};

use crate::{
    db::sea_models::{user, user_ban},
    services::session_store::SqliteSessionStore,
    utils::telemetry,
};

const DUMMY_VERIFY_PASSWORD: &str = "timing-equalization-dummy-fixture";
// Timing-oracle mitigation: the not-found and OAuth login branches run a dummy
// Argon2id verify against this fixed hash so they cost the same as a real
// wrong-password attempt. Removing it leaks account existence via request latency.
static DUMMY_VERIFY_HASH: LazyLock<String> =
    LazyLock::new(|| password_auth::generate_hash(DUMMY_VERIFY_PASSWORD));

pub type AuthSession = rux_auth::AuthSession<AuthBackend>;

#[derive(Clone)]
pub struct AuthBackend {
    pub pool: DatabaseConnection,
    pub session_store: Arc<SqliteSessionStore>,
    pub revoked: Arc<Mutex<HashSet<String>>>,
}

impl AuthBackend {
    pub fn new(
        pool: &DatabaseConnection,
        session_store: Arc<SqliteSessionStore>,
        revoked: Arc<Mutex<HashSet<String>>>,
    ) -> Self {
        Self {
            pool: pool.clone(),
            session_store,
            revoked,
        }
    }

    #[instrument(skip(self))]
    pub async fn terminate(&self, tower_session_id: &str) {
        use std::str::FromStr;
        use tower_sessions::session::Id;

        match Id::from_str(tower_session_id) {
            Ok(id) => {
                if let Err(e) = self.session_store.delete(&id).await {
                    warn!(
                        error = %e,
                        "Failed to delete tower-session from store (revoked_at audit row still set)"
                    );
                } else {
                    info!("Deleted tower-session from session store");
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Could not parse tower-session id for termination; skipping store delete"
                );
            }
        }

        if let Ok(mut set) = self.revoked.lock() {
            set.insert(tower_session_id.to_string());
        }
    }

    pub fn check_password(password: String, hash: &str) -> Result<bool, AuthError> {
        verify_password(password, hash)
            .map(|_| true)
            .map_err(|_| AuthError::new(AuthErrorCode::InvalidCredentials))
    }

    fn run_dummy_password_verify(password: String) -> Result<bool, AuthError> {
        let hash: &str = DUMMY_VERIFY_HASH.as_str();
        verify_password(password, hash)
            .map(|_| true)
            .map_err(|_| AuthError::new(AuthErrorCode::InvalidCredentials))
    }

    async fn run_blocking_dummy_verify(password: String) -> Result<(), AuthError> {
        match task::spawn_blocking(move || Self::run_dummy_password_verify(password)).await {
            Ok(_) => Ok(()),
            Err(join_err) => {
                error!(error = %join_err, "Dummy password verification task failed");
                Err(AuthError::new(AuthErrorCode::InternalError)
                    .with_message("Password verification failed"))
            }
        }
    }

    #[instrument(skip(self, password), fields(email = %email, result))]
    pub async fn authenticate_password(
        &self,
        email: String,
        password: String,
    ) -> Result<Option<user::Model>, AuthError> {
        let metrics = telemetry::auth_metrics();
        metrics.login_attempts.add(1, &[]);

        info!("Attempting password authentication");

        let user_result = user::Entity::find_by_email(&self.pool, email.clone()).await;

        let user = match user_result {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!("User not found");
                tracing::Span::current().record("result", "user_not_found");
                metrics.login_failure.add(
                    1,
                    &[opentelemetry::KeyValue::new("reason", "user_not_found")],
                );
                Self::run_blocking_dummy_verify(password).await?;
                return Ok(None);
            }
            Err(err) => {
                error!(error = ?err, "Database error during user lookup");
                metrics
                    .login_failure
                    .add(1, &[opentelemetry::KeyValue::new("reason", "db_error")]);
                return Err(AuthError::new(AuthErrorCode::BackendError)
                    .with_message("Database error during authentication"));
            }
        };

        let pwd_hash = match &user.password {
            Some(pwd) => pwd.clone(),
            None => {
                warn!("User has no password (OAuth user attempting password login)");
                tracing::Span::current().record("result", "no_password");
                metrics
                    .login_failure
                    .add(1, &[opentelemetry::KeyValue::new("reason", "oauth_user")]);
                Self::run_blocking_dummy_verify(password).await?;
                return Ok(None);
            }
        };

        let verify_start = Instant::now();
        let password_valid =
            match task::spawn_blocking(move || Self::check_password(password, &pwd_hash)).await {
                Ok(result) => result?,
                Err(join_err) => {
                    error!(error = %join_err, "Password verification task failed");
                    metrics
                        .login_failure
                        .add(1, &[opentelemetry::KeyValue::new("reason", "task_error")]);
                    return Err(AuthError::new(AuthErrorCode::InternalError)
                        .with_message("Password verification failed"));
                }
            };

        let verify_duration = verify_start.elapsed().as_millis() as f64;
        metrics
            .password_verification_duration
            .record(verify_duration, &[]);

        if password_valid {
            info!(user_id = user.id, "Authentication successful");
            tracing::Span::current().record("result", "success");
            metrics.login_success.add(1, &[]);
            metrics.session_created.add(1, &[]);
            Ok(Some(user))
        } else {
            warn!("Invalid password");
            tracing::Span::current().record("result", "invalid_password");
            metrics.login_failure.add(
                1,
                &[opentelemetry::KeyValue::new("reason", "invalid_password")],
            );
            Ok(None)
        }
    }

    #[instrument(skip(self), fields(result))]
    pub async fn authenticate_oauth(
        &self,
        google_id: String,
    ) -> Result<Option<user::Model>, AuthError> {
        let metrics = telemetry::auth_metrics();
        info!("OAuth authentication attempt");

        let user = user::Entity::find_by_google_id(&self.pool, google_id)
            .await
            .map_err(|err| {
                error!(error = ?err, "Database error during OAuth user lookup");
                AuthError::new(AuthErrorCode::BackendError)
                    .with_message("Database error during OAuth lookup")
            })?;

        match user {
            Some(user) => {
                info!(user_id = user.id, "OAuth authentication successful");
                metrics.login_success.add(1, &[]);
                metrics.session_created.add(1, &[]);
                Ok(Some(user))
            }
            None => {
                warn!("OAuth user not found");
                metrics.login_failure.add(
                    1,
                    &[opentelemetry::KeyValue::new(
                        "reason",
                        "oauth_user_not_found",
                    )],
                );
                Ok(None)
            }
        }
    }
}

impl std::fmt::Debug for AuthBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthBackend")
            .field("pool", &"DatabaseConnection{...}")
            .field("session_store", &"Arc<SqliteSessionStore>")
            .field("revoked", &"Arc<Mutex<HashSet<String>>>")
            .finish()
    }
}

impl AuthUser for user::Model {
    type Id = i32;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        // Bind sessions to the per-user random session_auth_secret, not email —
        // email is public/derivable. Falling back to the password hash keeps
        // password users' sessions valid if the secret column is unexpectedly absent.
        if !self.session_auth_secret.is_empty() {
            return self.session_auth_secret.as_bytes();
        }
        match &self.password {
            Some(password) => password.as_bytes(),
            None => &[],
        }
    }

    fn email_verified(&self) -> bool {
        self.is_verified
    }

    fn totp_enabled(&self) -> bool {
        self.two_fa_enabled
    }

    fn role_level(&self) -> i32 {
        self.role.to_i32()
    }
}

#[async_trait]
impl RuxAuthBackend for AuthBackend {
    type User = user::Model;

    #[instrument(skip(self), fields(user_id = %id))]
    async fn get_user(&self, id: &i32) -> Result<Option<Self::User>, AuthError> {
        user::Entity::get_by_id(&self.pool, *id)
            .await
            .map_err(|err| {
                error!(error = ?err, "Error retrieving user");
                AuthError::new(AuthErrorCode::BackendError).with_message("Failed to retrieve user")
            })
    }

    #[instrument(skip(self), fields(user_id = %user_id))]
    async fn check_ban(&self, user_id: &i32) -> Result<BanStatus, AuthError> {
        let ban = user_ban::Entity::get_active_ban(&self.pool, *user_id)
            .await
            .map_err(|err| {
                error!(error = ?err, "Error checking ban status");
                AuthError::new(AuthErrorCode::BackendError)
                    .with_message("Failed to check ban status")
            })?;

        match ban {
            Some(ban) => Ok(BanStatus::Banned {
                reason: ban.reason,
                expires_at: ban.expires_at,
                banned_by: ban.banned_by.map(|id| id as i64),
            }),
            None => Ok(BanStatus::NotBanned),
        }
    }

    #[instrument(skip(self, password), fields(user_id = %user_id))]
    async fn verify_password(&self, user_id: &i32, password: &str) -> Result<bool, AuthError> {
        let user = self.get_user(user_id).await?;

        let user = match user {
            Some(u) => u,
            None => return Ok(false),
        };

        let pwd_hash = match &user.password {
            Some(pwd) => pwd.clone(),
            None => return Ok(false),
        };

        let password = password.to_string();
        match task::spawn_blocking(move || Self::check_password(password, &pwd_hash)).await {
            Ok(result) => result.map_err(|_| AuthError::new(AuthErrorCode::InvalidCredentials)),
            Err(_) => Err(AuthError::new(AuthErrorCode::InternalError)
                .with_message("Password verification task failed")),
        }
    }

    async fn on_login(&self, user: &Self::User) -> Result<(), AuthError> {
        info!(user_id = user.id, "User logged in via rux-auth");
        Ok(())
    }

    async fn on_logout(&self, user_id: &i32) -> Result<(), AuthError> {
        info!(user_id = user_id, "User logged out via rux-auth");
        Ok(())
    }
}

pub const SESSION_MAX_AGE_SECS: u64 = 14 * 24 * 60 * 60;

type Mapping = (String, Instant);

static SESSION_MAP: OnceLock<Mutex<HashMap<i32, Mapping>>> = OnceLock::new();

fn session_map() -> &'static Mutex<HashMap<i32, Mapping>> {
    SESSION_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reap_stale(map: &mut HashMap<i32, Mapping>) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < SESSION_MAX_AGE_SECS);
}

#[allow(dead_code)]
pub(crate) fn session_mapping_key(pg_session_id: i32) -> String {
    format!("rux:sid_map:{pg_session_id}")
}

pub(crate) fn record_session_mapping(pg_session_id: i32, tower_session_id: &str) {
    let mut map = match session_map().lock() {
        Ok(guard) => guard,
        Err(e) => {
            warn!(error = %e, "session map poisoned; cannot record tower-session mapping");
            return;
        }
    };
    reap_stale(&mut map);
    map.insert(
        pg_session_id,
        (tower_session_id.to_string(), Instant::now()),
    );
}

pub(crate) fn lookup_session_mapping(pg_session_id: i32) -> Option<String> {
    let mut map = session_map().lock().ok()?;
    reap_stale(&mut map);
    map.get(&pg_session_id).map(|(sid, _)| sid.clone())
}

#[async_trait]
impl SessionRevocation for AuthBackend {
    #[instrument(skip(self), level = "debug")]
    async fn is_session_revoked(&self, tower_session_id: &str) -> Result<bool, AuthError> {
        match self.revoked.lock() {
            Ok(set) => Ok(set.contains(tower_session_id)),
            Err(e) => {
                warn!(
                    error = %e,
                    tower_session_id = %tower_session_id,
                    "Revocation set lock poisoned (fail-open): session remains valid"
                );
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_verify_does_full_work_not_parse_short_circuit() {
        let dummy = DUMMY_VERIFY_HASH.as_str();

        let wrong = password_auth::verify_password("definitely-not-the-fixture", dummy);
        assert!(
            matches!(wrong, Err(password_auth::VerifyError::PasswordInvalid)),
            "dummy verify must fail as a wrong password (PasswordInvalid), \
             not short-circuit with a Parse error — got: {wrong:?}"
        );

        assert_eq!(
            password_auth::verify_password(DUMMY_VERIFY_PASSWORD, dummy),
            Ok(()),
            "fixture password must round-trip against DUMMY_VERIFY_HASH"
        );
    }

    #[test]
    fn dummy_hash_is_full_argon2id_phc() {
        let dummy = DUMMY_VERIFY_HASH.as_str();
        assert!(
            dummy.starts_with("$argon2id$"),
            "DUMMY_VERIFY_HASH must be an Argon2id PHC string, got: {dummy:?}"
        );
    }
}
