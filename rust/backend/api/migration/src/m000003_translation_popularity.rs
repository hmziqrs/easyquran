use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS translation_popularity (
    id TEXT PRIMARY KEY,
    score REAL NOT NULL DEFAULT 0,
    hits_total INTEGER NOT NULL DEFAULT 0,
    last_hit_at INTEGER,
    updated_at INTEGER NOT NULL
)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        if manager.has_table("translation_popularity").await? {
            // Idempotent: a prior partial run may have created the table. No column
            // drift is possible (the shape is fixed at introduction), so a no-op is
            // correct here.
            Ok(())
        } else {
            conn.execute_unprepared(CREATE_TABLE).await?;
            Ok(())
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS translation_popularity")
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
                "PRAGMA table_info(translation_popularity)",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap();
        rows.into_iter()
            .map(|r| r.try_get_by_index::<String>(1).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn fresh_migration_creates_five_column_table() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();

        let cols = column_names(&db).await;
        for expected in ["id", "score", "hits_total", "last_hit_at", "updated_at"] {
            assert!(
                cols.iter().any(|c| c.as_str() == expected),
                "missing column {expected}: {cols:?}"
            );
        }

        // Defaults: a row inserted with only an id takes score=0, hits_total=0,
        // updated_at=0 (NOT NULL with no default would have rejected this).
        db.execute_unprepared(
            "INSERT INTO translation_popularity (id, updated_at) VALUES ('en.sahih', 0)",
        )
        .await
        .unwrap();
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT score, hits_total, last_hit_at FROM translation_popularity WHERE id='en.sahih'",
                Vec::<Value>::new(),
            ))
            .await
            .unwrap()
            .unwrap();
        let score: f64 = row.try_get_by_index(0).unwrap();
        let hits_total: i64 = row.try_get_by_index(1).unwrap();
        let last_hit_at: Option<i64> = row.try_get_by_index(2).unwrap();
        assert_eq!(score, 0.0);
        assert_eq!(hits_total, 0);
        assert!(last_hit_at.is_none());
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();
        m.up(&mgr).await.unwrap();

        let cols = column_names(&db).await;
        assert_eq!(cols.len(), 5, "no duplicate columns on re-run: {cols:?}");
    }

    #[tokio::test]
    async fn down_drops_table() {
        let db = mem_db().await;
        let mgr = SchemaManager::new(&db);
        let m = Migration;
        m.up(&mgr).await.unwrap();
        m.down(&mgr).await.unwrap();
        assert!(!mgr.has_table("translation_popularity").await.unwrap());
    }
}
