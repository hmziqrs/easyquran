use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "DELETE FROM devices WHERE token IN (
                SELECT token FROM devices GROUP BY token HAVING COUNT(DISTINCT user_id) > 1
            )",
        )
        .await?;
        conn.execute_unprepared(
            "DELETE FROM devices WHERE id NOT IN (SELECT MAX(id) FROM devices GROUP BY token)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_token_owner ON devices (token)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_devices_token_owner")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};

    #[tokio::test]
    async fn ambiguous_legacy_owners_are_revoked_and_single_owner_duplicates_collapsed() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE devices (id INTEGER PRIMARY KEY, user_id INTEGER, token TEXT)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO devices VALUES (1, 1, 'shared'), (2, 2, 'shared'),
                (3, 1, 'same-owner'), (4, 1, 'same-owner'), (5, 2, 'unique')",
        )
        .await
        .unwrap();
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        Migration.up(&manager).await.unwrap();
        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id FROM devices ORDER BY id",
            ))
            .await
            .unwrap();
        let ids: Vec<i64> = rows
            .iter()
            .map(|row| row.try_get_by_index(0).unwrap())
            .collect();
        assert_eq!(ids, vec![4, 5]);
        assert!(db
            .execute_unprepared("INSERT INTO devices VALUES (6, 3, 'unique')")
            .await
            .is_err());
        Migration.down(&manager).await.unwrap();
        db.execute_unprepared("INSERT INTO devices VALUES (6, 3, 'unique')")
            .await
            .unwrap();
    }
}
