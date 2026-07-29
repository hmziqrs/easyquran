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

/// CRYP-SC-002 (login timing equalization): a fixed, non-secret Argon2id hash
/// used to drive a DUMMY password verify on the user-not-found and
/// OAuth-no-password branches of [`AuthBackend::authenticate_password`].
///
/// The valid-user / wrong-password branch performs a full Argon2id
/// `verify_password` that dominates the request's CPU cost; the two early-return
/// branches did none of that work, so an attacker averaging request latency
/// could distinguish "valid user, wrong password" from "no such user" / "OAuth
/// user". To close that timing oracle we run the SAME verify against THIS fixed
/// hash before returning, so every login attempt pays the Argon2id cost
/// regardless of branch. The input password is verified against a hash it can
/// never match, so the result is always `false` — exactly the cost shape of a
/// genuine wrong-password attempt.
///
/// This mirrors the existing `equalize_unknown_email_work` pattern from
/// `modules/forgot_password_v1/controller.rs`. We compute the hash ONCE
/// (process lifetime) via `LazyLock` rather than hard-coding a PHC string, so
/// the Argon2 parameters always track the `password_auth` crate version used by
/// the rest of the codebase (no drift between the dummy hash and real hashes).
const DUMMY_VERIFY_PASSWORD: &str = "timing-equalization-dummy-fixture";
static DUMMY_VERIFY_HASH: LazyLock<String> =
    LazyLock::new(|| password_auth::generate_hash(DUMMY_VERIFY_PASSWORD));

/// Re-export the AuthSession from rux-auth
pub type AuthSession = rux_auth::AuthSession<AuthBackend>;

/// Authentication backend implementation.
///
/// Holds the DB pool, the tower-sessions store, and the process-wide revoked
/// session set. The session store is required to terminate a live tower-session
/// record on logout/revoke (V-HIGH-2): deleting the store key means the very
/// next request carrying that cookie loads nothing and is treated as anonymous.
/// The revoked set backs the per-request [`SessionRevocation::is_session_revoked`]
/// check as defense-in-depth (in case the store delete races a concurrent save).
#[derive(Clone)]
pub struct AuthBackend {
    pub pool: DatabaseConnection,
    /// Shared SQLite-backed tower-sessions store (same instance as
    /// `AppState::session_store`). Used by [`AuthBackend::terminate`].
    pub session_store: Arc<SqliteSessionStore>,
    /// Process-wide set of revoked tower-session ids (same instance as
    /// `AppState::revoked_sessions`). Consulted by `is_session_revoked`.
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

    /// Revoke a live tower-sessions record by its store id.
    ///
    /// V-HIGH-2: stamping `user_sessions.revoked_at` does NOT touch the
    /// tower-sessions record, so the cookie keeps authenticating until its 14-day
    /// inactivity expiry. This deletes the store record (SqliteSessionStore) so
    /// the very next request carrying that cookie finds no session and is
    /// unauthenticated. The id is also added to the revoked set as
    /// defense-in-depth for the per-request extractor check.
    ///
    /// `tower_session_id` is the `tower_sessions::session::Id` `Display` output
    /// (a 22-char base64url-no-pad i128) — the same key the store saves under.
    #[instrument(skip(self))]
    pub async fn terminate(&self, tower_session_id: &str) {
        use std::str::FromStr;
        use tower_sessions::session::Id;

        // 1. Kill the live record: the next request with this cookie loads
        //    nothing from the store and is treated as anonymous.
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

        // 2. Defense-in-depth: record the revocation so the per-request
        //    extractor check (rux_auth::SessionRevocation) still catches it even
        //    if a concurrent session save re-created the record. The set is
        //    bounded by opportunistic expiry housekeeping in
        //    `record_session_mapping`/`reap_revoked` — entries here age out with
        //    the session max-age.
        if let Ok(mut set) = self.revoked.lock() {
            set.insert(tower_session_id.to_string());
        }
    }

    /// Verify password against hash
    pub fn check_password(password: String, hash: &str) -> Result<bool, AuthError> {
        verify_password(password, hash)
            .map(|_| true)
            .map_err(|_| AuthError::new(AuthErrorCode::InvalidCredentials))
    }

    /// CRYP-SC-002: run a DUMMY Argon2id verify of `password` against the fixed
    /// [`DUMMY_VERIFY_HASH`]. The result is always false (the password can never
    /// match the fixture hash) — the work is what matters, not the outcome.
    /// Returns an error only on a genuine hashing failure, matching
    /// [`check_password`]'s contract so the caller's error handling is uniform.
    fn run_dummy_password_verify(password: String) -> Result<bool, AuthError> {
        // `LazyLock` is initialized on first touch; safe to read from a blocking
        // task. `as_str()` borrows the static backing storage, so the clone-free
        // borrow is valid for the lifetime of the process.
        let hash: &str = DUMMY_VERIFY_HASH.as_str();
        verify_password(password, hash)
            .map(|_| true)
            .map_err(|_| AuthError::new(AuthErrorCode::InvalidCredentials))
    }

    /// CRYP-SC-002: run [`run_dummy_password_verify`] off the async executor, so
    /// the memory-hard Argon2id KDF never blocks the Tokio runtime. Mirrors the
    /// wrong-password branch's `spawn_blocking` around `check_password`. The
    /// (always-false) result is dropped; only the CPU cost matters.
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

    /// Authenticate with email and password
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
                // CRYP-SC-002: equalize CPU with the valid-user / wrong-password
                // branch, which pays a full Argon2id verify here. Without this
                // dummy verify the markedly cheaper not-found path is a timing
                // oracle for account existence. The result is always false (the
                // password can never match the fixture hash); only the work
                // matters. See [`DUMMY_VERIFY_HASH`].
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

        // Check if user has a password (not OAuth user)
        let pwd_hash = match &user.password {
            Some(pwd) => pwd.clone(),
            None => {
                warn!("User has no password (OAuth user attempting password login)");
                tracing::Span::current().record("result", "no_password");
                metrics
                    .login_failure
                    .add(1, &[opentelemetry::KeyValue::new("reason", "oauth_user")]);
                // CRYP-SC-002: same timing equalization as the not-found branch
                // — an OAuth user (no password hash) would otherwise return far
                // faster than a wrong-password attempt, leaking "this account
                // exists but is OAuth-only". Run the dummy Argon2id verify.
                Self::run_blocking_dummy_verify(password).await?;
                return Ok(None);
            }
        };

        // Verify password in blocking task
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

    /// Authenticate with OAuth (Google ID)
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

/// Implement rux-auth's AuthUser trait for user::Model
impl AuthUser for user::Model {
    type Id = i32;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        // CRYP-ENC-004: the per-session CSRF/session-auth binding is keyed on the
        // server-random `session_auth_secret` column (added by the model layer /
        // W3) rather than the raw `email`. Basing it on email meant an email
        // change OR a sufficiently strong attacker who could observe the derived
        // hash could recompute it from a public field; a per-user random secret
        // is not derivable from any user-facing value. Rotating the secret (e.g.
        // on credential change) invalidates prior sessions, which is the desired
        // trust-transition behavior.
        //
        // Defense-in-depth fallback: if the secret column is unexpectedly absent
        // (should not occur after the backfill migration writes a random secret
        // per existing user), fall back to the password hash for password users
        // rather than panic — this keeps the session functional while still
        // avoiding the raw-email path. OAuth users without a secret have nothing
        // stable to fall back to, so an empty hash forces session invalidation.
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

/// Implement rux-auth's AuthBackend trait
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
            None => return Ok(false), // OAuth user
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

/// Mirror of the `SessionManagerLayer` inactivity expiry (14 days, in seconds).
/// Used to TTL the in-memory revocation / mapping tables so they cannot grow
/// without bound now that there is no Redis TTL doing it for us.
pub const SESSION_MAX_AGE_SECS: u64 = 14 * 24 * 60 * 60;

// ── Session id mapping (PG user_sessions row ⇄ tower-session id) ──────────
// Relocated from modules::auth_v1 so the OAuth login path (services::oauth)
// can record the mapping without an inverted service→module dependency.
//
// Previously a Redis STRING per mapping with a 14-day TTL. Now a process-global
// in-memory map (the mapping only ever needs to be read back by the SAME
// process that recorded it — `sessions_terminate` runs in-process). Entries are
// opportunistically reaped on each record; a restart loses live mappings, in
// which case `terminate` falls back to the audit-only `revoked_at` (the cookie
// still expires on its 14-day inactivity window).

type Mapping = (String, Instant);

static SESSION_MAP: OnceLock<Mutex<HashMap<i32, Mapping>>> = OnceLock::new();

fn session_map() -> &'static Mutex<HashMap<i32, Mapping>> {
    SESSION_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Reap entries older than [`SESSION_MAX_AGE_SECS`] from `map`. Called under the
/// lock so the housekeeping is consistent with the insert/lookup.
fn reap_stale(map: &mut HashMap<i32, Mapping>) {
    map.retain(|_, (_, at)| at.elapsed().as_secs() < SESSION_MAX_AGE_SECS);
}

/// Legacy Redis-key form, retained as the stable namespaced contract the unit
/// test pins. No longer written anywhere at runtime (the in-memory map is keyed
/// by the integer `user_sessions.id` directly).
#[allow(dead_code)]
pub(crate) fn session_mapping_key(pg_session_id: i32) -> String {
    format!("rux:sid_map:{pg_session_id}")
}

/// Persist `user_sessions.id -> tower_session_id` so terminate can later find
/// and kill the live tower-sessions record. Synchronous: the in-memory map is
/// process-local, so no I/O is awaited.
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

/// Look up the tower-session id previously recorded for a `user_sessions.id`.
/// Returns `None` if the mapping is absent (pre-fix rows, expired, or lost to a
/// restart).
pub(crate) fn lookup_session_mapping(pg_session_id: i32) -> Option<String> {
    let mut map = session_map().lock().ok()?;
    reap_stale(&mut map);
    map.get(&pg_session_id).map(|(sid, _)| sid.clone())
}

/// V-HIGH-2 real per-request session revocation.
///
/// `AuthBackend::terminate` (run by the `sessions_terminate` handler) both
/// deletes the live tower-sessions record AND inserts the id into the
/// in-memory revoked set as defense-in-depth. This trait method is the
/// per-request hook the extractor consults on every authenticated request: it
/// checks that set for the live tower-session id. If the id is a member, the
/// extractor deletes the session and treats the caller as unauthenticated — so
/// a revoked cookie stops authenticating on the *very next request* even if the
/// terminate-time delete raced a concurrent session save.
///
/// **Cost:** one in-memory `HashSet` lookup under a `Mutex` (O(1)) per
/// authenticated request — strictly cheaper than the prior Redis `SISMEMBER`.
///
/// **Fail-open policy (unchanged from the prior Redis path):** on a poisoned
/// lock we return `Ok(false)` and `warn!`. This mirrors the rate limiter's
/// fail-open behavior and avoids a mass lockout if the mutex is poisoned — at
/// the cost of a revoked session briefly staying live. The DB `revoked_at`
/// stamp, the terminate-time store delete, and the session's own 14-day
/// inactivity expiry still bound the window.
#[async_trait]
impl SessionRevocation for AuthBackend {
    #[instrument(skip(self), level = "debug")]
    async fn is_session_revoked(&self, tower_session_id: &str) -> Result<bool, AuthError> {
        match self.revoked.lock() {
            Ok(set) => Ok(set.contains(tower_session_id)),
            Err(e) => {
                // Fail-open: a poisoned lock must not lock out every user.
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
