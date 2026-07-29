//! SQLite-backed `tower-sessions` session store.
//!
//! Replaces the prior `RedisStore` for the default (no-Redis) build. Backed by
//! the SAME shared sea-orm `DatabaseConnection` the rest of the app uses, so
//! sessions live alongside the canonical SQLite database. The `sessions` table
//! is created HERE at construction (`CREATE TABLE IF NOT EXISTS`) — it is
//! deliberately NOT part of the SeaORM migration suite, mirroring the
//! `rate_limit_store` L2 table pattern (self-contained, bootstrapped at startup,
//! never dropped by the migrator).
//!
//! Records are serialized with MessagePack (`rmp_serde`) — the exact codec the
//! previous `tower-sessions-redis-store` used — so the on-disk shape of a record
//! is unchanged. The row key is the tower-session `Id` `Display` string (the
//! 22-char base64url-no-pad i128), matching the key the old `RedisStore` saved
//! under; `expiry_date` is the record's unix timestamp (INTEGER) so expired rows
//! can be purged by a periodic sweep and filtered on load.

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::{session_store, SessionStore};

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    data BLOB NOT NULL,
    expiry_date INTEGER NOT NULL
)";

/// Convert any error into a `tower_sessions` backend error.
fn backend_err<E: std::fmt::Display>(e: E) -> session_store::Error {
    session_store::Error::Backend(e.to_string())
}

/// `tower-sessions` `SessionStore` backed by the shared SQLite connection.
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
    /// Construct the store over the shared connection and ensure the backing
    /// `sessions` table exists. The `CREATE TABLE IF NOT EXISTS` is best-effort:
    /// a failure logs and continues (the first save will then surface a real
    /// error rather than boot looping the process).
    pub async fn new(db: DatabaseConnection) -> Self {
        if let Err(e) = db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, CREATE_TABLE))
            .await
        {
            tracing::warn!(error = %e, "failed to create sessions table (non-fatal)");
        }
        Self { db }
    }

    /// Delete every row whose `expiry_date` has passed. Intended to be driven by
    /// a periodic background task (see `main`); not required by the
    /// `SessionStore` trait but keeps the table from growing without bound now
    /// that there is no Redis TTL doing it for us.
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
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    /// Insert a brand-new record, regenerating the id on the (astronomically
    /// rare) collision. Mirrors `RedisStore::create`'s `NX` semantics.
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
            // Collision: regenerate and retry.
            record.id = Id::default();
        }
    }

    /// Upsert an existing record by id (the layer only saves records it has
    /// either created or loaded, so this is the steady-state write path).
    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data = rmp_serde::to_vec(record)
            .map_err(|e| session_store::Error::Encode(e.to_string()))?;
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
