use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS rate_limit_state (
    key TEXT PRIMARY KEY,
    fixed_count INTEGER NOT NULL DEFAULT 0,
    fixed_expires_at INTEGER NOT NULL DEFAULT 0,
    block_until_at INTEGER NOT NULL DEFAULT 0,
    block_scope TEXT NOT NULL DEFAULT 'Temp'
)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        if manager.has_table("rate_limit_state").await? {
            // Legacy boot-owned (4-column) or prior-partial table: inspect and
            // conditionally add block_scope without touching existing rows.
            let cols = conn
                .query_all(sea_orm::Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Sqlite,
                    "PRAGMA table_info(rate_limit_state)",
                    Vec::<Value>::new(),
                ))
                .await?;
            let has_scope = cols.iter().any(|row| {
                let name: Option<String> = row.try_get_by_index(1).ok();
                name.as_deref() == Some("block_scope")
            });
            if !has_scope {
                conn.execute_unprepared(
                    "ALTER TABLE rate_limit_state \
                     ADD COLUMN block_scope TEXT NOT NULL DEFAULT 'Temp'",
                )
                .await?;
            }
        } else {
            conn.execute_unprepared(CREATE_TABLE).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS rate_limit_state")
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
                "PRAGMA table_info(rate_limit_state)",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|r| r.try_get_by_index::<String>(1).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn fresh_migration_creates_five_column_table_with_scope() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();

        let cols = column_names(&db).await;
        for expected in [
            "key",
            "fixed_count",
            "fixed_expires_at",
            "block_until_at",
            "block_scope",
        ] {
            assert!(
                cols.iter().any(|c| c.as_str() == expected),
                "missing column {expected}: {cols:?}"
            );
        }

        // block_scope defaults to 'Temp' for new rows that omit it.
        db.execute_unprepared("INSERT INTO rate_limit_state (key) VALUES ('k')")
            .await
            .unwrap();
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT block_scope FROM rate_limit_state WHERE key='k'",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap();
        let scope: String = row.try_get_by_index(0).unwrap();
        assert_eq!(scope, "Temp");
    }

    #[tokio::test]
    async fn legacy_table_gains_scope_without_data_loss() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE rate_limit_state (\
             key TEXT PRIMARY KEY, \
             fixed_count INTEGER NOT NULL DEFAULT 0, \
             fixed_expires_at INTEGER NOT NULL DEFAULT 0, \
             block_until_at INTEGER NOT NULL DEFAULT 0)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO rate_limit_state (key, fixed_count, fixed_expires_at, block_until_at) \
             VALUES ('ip1', 5, 111, 222), ('ip2', 7, 333, 444)",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();

        let rows = db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT key, fixed_count, fixed_expires_at, block_until_at, block_scope \
                 FROM rate_limit_state ORDER BY key",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);

        let (k, fc, fe, bu, sc): (String, i64, i64, i64, String) = (
            rows[0].try_get_by_index(0).unwrap(),
            rows[0].try_get_by_index(1).unwrap(),
            rows[0].try_get_by_index(2).unwrap(),
            rows[0].try_get_by_index(3).unwrap(),
            rows[0].try_get_by_index(4).unwrap(),
        );
        assert_eq!(
            (k.as_str(), fc, fe, bu, sc.as_str()),
            ("ip1", 5, 111, 222, "Temp")
        );

        let (k, fc, fe, bu, sc): (String, i64, i64, i64, String) = (
            rows[1].try_get_by_index(0).unwrap(),
            rows[1].try_get_by_index(1).unwrap(),
            rows[1].try_get_by_index(2).unwrap(),
            rows[1].try_get_by_index(3).unwrap(),
            rows[1].try_get_by_index(4).unwrap(),
        );
        assert_eq!(
            (k.as_str(), fc, fe, bu, sc.as_str()),
            ("ip2", 7, 333, 444, "Temp")
        );
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();
        // Second run must not error and must not duplicate block_scope.
        m.up(&mgr).await.unwrap();

        let cols = column_names(&db).await;
        let scope_count = cols.iter().filter(|c| c.as_str() == "block_scope").count();
        assert_eq!(scope_count, 1);
    }
}
