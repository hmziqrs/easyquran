use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Account emails become case-insensitive identities: register, login, email
// change and OAuth linking all normalize (trim + lowercase) before comparing
// or storing. Existing rows written with mixed case must be normalized too, or
// a user who registered as "Foo@X.com" could never be found by the normalized
// lookup.
//
// Collision-safe: two accounts can legitimately coexist as "Foo@x.com" and
// "foo@x.com" today (the UNIQUE index is BINARY/case-sensitive). Lowercasing
// both would violate it, so any row whose normalized form already belongs to a
// DIFFERENT row is left untouched. Those are unreachable duplicates awaiting a
// human decision; every other row is normalized.
const LOWERCASE_EMAILS: &str = "UPDATE users SET email = LOWER(TRIM(email)) \
     WHERE email <> LOWER(TRIM(email)) \
     AND NOT EXISTS ( \
         SELECT 1 FROM users AS other \
         WHERE other.id <> users.id AND other.email = LOWER(TRIM(users.email)) \
     )";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(LOWERCASE_EMAILS)
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Original casing is unrecoverable; nothing to revert.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::SchemaManager;

    async fn mem_db() -> sea_orm::DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        Database::connect(opt).await.unwrap()
    }

    async fn email_of(db: &sea_orm::DatabaseConnection, id: i32) -> String {
        db.query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT email FROM users WHERE id = ?",
            [id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get_by_index::<String>(0)
        .unwrap()
    }

    #[tokio::test]
    async fn lowercases_mixed_case_rows_and_skips_collisions() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        // 1: plain mixed case -> normalized. 2+3: collision pair -> both stay.
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (1, 'a', ' Foo@Example.COM '), \
             (2, 'b', 'Dup@X.com'), \
             (3, 'c', 'dup@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(email_of(&db, 1).await, "foo@example.com");
        assert_eq!(
            email_of(&db, 2).await,
            "Dup@X.com",
            "collision left for a human"
        );
        assert_eq!(email_of(&db, 3).await, "dup@x.com");
    }

    #[tokio::test]
    async fn noop_when_everything_already_normalized() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO users (id, name, email) VALUES (1, 'a', 'ok@x.com')")
            .await
            .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(email_of(&db, 1).await, "ok@x.com");
    }
}
