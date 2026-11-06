use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Notice delivery channel (A21). Every notice is email today. The in-app inbox is
// post-unban copy and is never a delivery channel, so it is not a value here. The
// column has a constant default, so the ADD runs on a populated table. SQLite has
// no ADD COLUMN IF NOT EXISTS, so the add is guarded by a column probe.
const ADD_CHANNEL: &str = "ALTER TABLE enforcement_notices ADD COLUMN notice_channel TEXT NOT NULL DEFAULT 'email' CHECK (notice_channel IN ('email'))";

async fn has_column<C: ConnectionTrait>(
    conn: &C,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    let row = conn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT 1 FROM pragma_table_info(?) WHERE name = ?",
            [table.into(), column.into()],
        ))
        .await?;
    Ok(row.is_some())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        if !has_column(&txn, "enforcement_notices", "notice_channel").await? {
            txn.execute_unprepared(ADD_CHANNEL).await?;
        }
        txn.commit().await
    }

    // Keeps the column and every notice row on rollback.
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection};
    use sea_orm_migration::MigratorTrait;

    async fn column_count(db: &DatabaseConnection) -> i64 {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM pragma_table_info('enforcement_notices') WHERE name = 'notice_channel'"
                    .to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    #[tokio::test]
    async fn up_twice_is_safe_and_defaults_to_email() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        assert_eq!(column_count(&db).await, 1);
        db.execute_unprepared(
            "INSERT INTO enforcement_notices (user_id, ban_id, reason_code, corroborator, \
             decided_by, decided_at, effective_at, expires_at, created_at, retention_class) \
             VALUES (1, 1, 'spam', 'none', 1, 10, 10, 20, 10, 'enforcement_12m')",
        )
        .await
        .unwrap();
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT notice_channel FROM enforcement_notices".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get_by_index::<String>(0).unwrap(), "email");
    }
}
