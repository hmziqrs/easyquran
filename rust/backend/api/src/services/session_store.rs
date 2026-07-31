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
            .execute(Statement::from_string(DatabaseBackend::Sqlite, CREATE_TABLE))
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
