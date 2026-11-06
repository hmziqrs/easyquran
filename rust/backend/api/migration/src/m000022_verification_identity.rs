use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Verification identity (A10): one keyed HMAC per account, over the canonical
// mailbox (Gmail dots and `+tag` folded). It exists only to count accounts per
// mailbox for registration caps. Nullable and additive: rows that predate this
// migration are filled by the boot backfill
// (`user::Entity::backfill_verification_identity`), which only touches NULL rows.
const COLUMNS: &[(&str, &str, &str)] = &[("users", "verification_identity_hash", "TEXT")];

const STMTS: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_users_verification_identity \
     ON users (verification_identity_hash)",
];

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
        for (table, column, ty) in COLUMNS {
            if !has_column(&txn, table, column).await? {
                txn.execute_unprepared(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty}"))
                    .await?;
            }
        }
        for stmt in STMTS {
            txn.execute_unprepared(stmt).await?;
        }
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        txn.execute_unprepared("DROP INDEX IF EXISTS idx_users_verification_identity")
            .await?;
        for (table, column, _) in COLUMNS {
            if has_column(&txn, table, column).await? {
                txn.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN {column}"))
                    .await?;
            }
        }
        txn.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Migrator;
    use sea_orm_migration::sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    #[tokio::test]
    async fn up_twice_is_safe_after_a_full_run() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        Migration.up(&mgr).await.unwrap();
        assert!(has_column(&db, "users", "verification_identity_hash")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn down_then_up_restores_the_column() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let mgr = SchemaManager::new(&db);
        Migration.down(&mgr).await.unwrap();
        assert!(!has_column(&db, "users", "verification_identity_hash")
            .await
            .unwrap());
        Migration.up(&mgr).await.unwrap();
        assert!(has_column(&db, "users", "verification_identity_hash")
            .await
            .unwrap());
    }
}
