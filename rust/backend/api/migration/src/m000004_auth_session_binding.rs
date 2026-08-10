use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Durable 1:1 binding from a user_session audit row to its opaque tower session id
// (W8e). tower_session_id is the lookup key (session list termination resolves an
// audit id to a tower id via the reverse direction; cycle_id rotation swaps the
// tower id for the SAME audit row). PK on tower_session_id + UNIQUE on
// user_session_id enforce exactly-one-live-tower-per-audit-row.
const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS auth_session_binding (
    tower_session_id TEXT PRIMARY KEY,
    user_session_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL
)";

const CREATE_USER_INDEX: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_session_binding_user \
     ON auth_session_binding (user_session_id)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        if manager.has_table("auth_session_binding").await? {
            // Idempotent: a prior partial run may have created the table. Ensure
            // the UNIQUE index exists even if the table predated it; SQLite's
            // CREATE UNIQUE INDEX IF NOT EXISTS is a no-op when it already exists.
            conn.execute_unprepared(CREATE_USER_INDEX).await?;
            Ok(())
        } else {
            conn.execute_unprepared(CREATE_TABLE).await?;
            conn.execute_unprepared(CREATE_USER_INDEX).await?;
            Ok(())
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS auth_session_binding")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement, Value};
    use sea_orm_migration::SchemaManager;

    async fn mem_db() -> sea_orm::DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        Database::connect(opt).await.unwrap()
    }

    async fn column_names(db: &sea_orm::DatabaseConnection) -> Vec<String> {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "PRAGMA table_info(auth_session_binding)",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|r| r.try_get_by_index::<String>(1).unwrap())
            .collect()
    }

    async fn index_exists(db: &sea_orm::DatabaseConnection, name: &str) -> bool {
        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "PRAGMA index_list('auth_session_binding')",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .any(|r| {
                let n: Option<String> = r.try_get_by_index(1).ok();
                n.as_deref() == Some(name)
            })
    }

    #[tokio::test]
    async fn fresh_migration_creates_three_column_table() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();

        let cols = column_names(&db).await;
        for expected in ["tower_session_id", "user_session_id", "created_at"] {
            assert!(
                cols.iter().any(|c| c.as_str() == expected),
                "missing column {expected}: {cols:?}"
            );
        }
        assert!(index_exists(&db, "idx_auth_session_binding_user").await);
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();
        m.up(&mgr).await.unwrap();

        let cols = column_names(&db).await;
        assert_eq!(cols.len(), 3, "no duplicate columns on re-run: {cols:?}");
        assert!(index_exists(&db, "idx_auth_session_binding_user").await);
    }

    #[tokio::test]
    async fn unique_user_session_id_enforced() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        db.execute_unprepared(
            "INSERT INTO auth_session_binding (tower_session_id, user_session_id, created_at) \
             VALUES ('t1', 10, 1)",
        )
        .await
        .unwrap();
        // Same user_session_id, different tower — must violate the UNIQUE index.
        let err = db
            .execute_unprepared(
                "INSERT INTO auth_session_binding (tower_session_id, user_session_id, created_at) \
                 VALUES ('t2', 10, 1)",
            )
            .await;
        assert!(err.is_err(), "UNIQUE(user_session_id) must reject a second tower for the same audit row");

        // Distinct user_session_id with its own tower is fine, and tower PK is unique.
        db.execute_unprepared(
            "INSERT INTO auth_session_binding (tower_session_id, user_session_id, created_at) \
             VALUES ('t3', 11, 1)",
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn down_drops_table() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();
        m.down(&mgr).await.unwrap();
        assert!(!mgr.has_table("auth_session_binding").await.unwrap());
    }
}
