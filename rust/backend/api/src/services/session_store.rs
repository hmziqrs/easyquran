use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};
use std::collections::HashSet;
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::{session_store, SessionStore};

// Auth sessions are stored under this key inside a tower-session Record's data
// map (source of truth: rux-auth::session::extractor SESSION_KEY). Stable wire
// key — changing it would invalidate every live session, so it is safe to mirror
// here as the marker that distinguishes an authenticated from an anonymous tower
// session during the W8e unbound-session sweep.
const AUTH_SESSION_KEY: &str = "rux_auth";

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    data BLOB NOT NULL,
    expiry_date INTEGER NOT NULL
)";

fn backend_err<E: std::fmt::Display>(e: E) -> session_store::Error {
    session_store::Error::Backend(e.to_string())
}

#[derive(Clone)]
pub struct SqliteSessionStore {
    db: DatabaseConnection,
}

impl std::fmt::Debug for SqliteSessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteSessionStore")
            .field("db", &"DatabaseConnection{...}")
            .finish()
    }
}

impl SqliteSessionStore {
    pub async fn new(db: DatabaseConnection) -> Self {
        if let Err(e) = db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                CREATE_TABLE,
            ))
            .await
        {
            tracing::warn!(error = %e, "failed to create sessions table (non-fatal)");
        }
        Self { db }
    }

    pub async fn delete_expired(&self) -> Result<(), sea_orm::DbErr> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "DELETE FROM sessions WHERE expiry_date <= ?",
                vec![Value::from(now)],
            ))
            .await?;
        Ok(())
    }

    /// W8e reconcile support: enumerate the live `sessions` table and forcibly
    /// delete any tower session that is an AUTH session (`rux_auth` key present)
    /// but has NO durable binding row. This closes the one gap
    /// `reconcile_unbound_sessions` cannot reach by audit-row id — a pre-binding
    /// session has no `auth_session_binding` row, so there is no tower id to
    /// `delete()` by. Enumerating the store and matching on the binding set
    /// removes those legacy live sessions at the cutover, making the restart a
    /// single clean re-authentication boundary instead of leaving them
    /// authenticated until their 14-day TTL. Anonymous (non-auth) sessions are
    /// skipped so CSRF continuity for logged-out visitors survives a reboot.
    ///
    /// Bounded: scans the live `sessions` table (naturally bounded by the active
    /// session count) and skips bound ids without decoding. Non-fatal: a per-row
    /// decode/store error is logged and skipped. Post-cutover every auth session
    /// is bound, so on subsequent boots this is a near-no-op. Returns the tower
    /// ids it removed so the caller can mirror them into its revocation set.
    pub async fn delete_unbound_auth_sessions(
        &self,
        bound_tower_ids: &HashSet<String>,
    ) -> Result<Vec<String>, sea_orm::DbErr> {
        use std::str::FromStr;
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id, data FROM sessions",
                Vec::<Value>::new(),
            ))
            .await?;
        let mut removed = Vec::new();
        for row in rows {
            let id: Option<String> = row.try_get_by_index(0).ok();
            let data: Option<Vec<u8>> = row.try_get_by_index(1).ok();
            let (Some(id), Some(data)) = (id, data) else {
                continue;
            };
            // Bound sessions have a durable mapping — leave them alone without
            // paying for a decode.
            if bound_tower_ids.contains(&id) {
                continue;
            }
            // Decode only to test whether this is an AUTH session. A decode
            // failure means an unreadable/corrupt row — skip rather than guess.
            let is_auth = match rmp_serde::from_slice::<Record>(&data) {
                Ok(rec) => rec.data.contains_key(AUTH_SESSION_KEY),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "reconcile: skipping undecodable tower-session row"
                    );
                    continue;
                }
            };
            if !is_auth {
                continue;
            }
            match Id::from_str(&id) {
                Ok(parsed) => match self.delete(&parsed).await {
                    Ok(_) => removed.push(id),
                    Err(e) => tracing::warn!(
                        error = %e,
                        "reconcile: unbound auth tower-session delete failed"
                    ),
                },
                Err(e) => tracing::warn!(
                    error = %e,
                    "reconcile: unbound tower-session id unparseable"
                ),
            }
        }
        Ok(removed)
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        loop {
            let data = rmp_serde::to_vec(record)
                .map_err(|e| session_store::Error::Encode(e.to_string()))?;
            let expiry = OffsetDateTime::unix_timestamp(record.expiry_date);
            let res = self
                .db
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "INSERT INTO sessions (id, data, expiry_date) VALUES (?, ?, ?) \
                     ON CONFLICT(id) DO NOTHING",
                    vec![
                        Value::from(record.id.to_string()),
                        Value::from(data),
                        Value::from(expiry),
                    ],
                ))
                .await
                .map_err(backend_err)?;
            if res.rows_affected() > 0 {
                return Ok(());
            }
            record.id = Id::default();
        }
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data =
            rmp_serde::to_vec(record).map_err(|e| session_store::Error::Encode(e.to_string()))?;
        let expiry = OffsetDateTime::unix_timestamp(record.expiry_date);
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO sessions (id, data, expiry_date) VALUES (?, ?, ?) \
                 ON CONFLICT(id) DO UPDATE SET \
                 data=excluded.data, \
                 expiry_date=excluded.expiry_date",
                vec![
                    Value::from(record.id.to_string()),
                    Value::from(data),
                    Value::from(expiry),
                ],
            ))
            .await
            .map_err(backend_err)?;
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT data FROM sessions WHERE id = ? AND expiry_date > ?",
                vec![Value::from(session_id.to_string()), Value::from(now)],
            ))
            .await
            .map_err(backend_err)?;
        match row {
            Some(row) => {
                let bytes: Vec<u8> = row
                    .try_get_by_index(0)
                    .map_err(|e| session_store::Error::Decode(e.to_string()))?;
                let record: Record = rmp_serde::from_slice(&bytes)
                    .map_err(|e| session_store::Error::Decode(e.to_string()))?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "DELETE FROM sessions WHERE id = ?",
                vec![Value::from(session_id.to_string())],
            ))
            .await
            .map_err(backend_err)?;
        Ok(())
    }
}
