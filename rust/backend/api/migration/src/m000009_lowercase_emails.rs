use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Account emails become case-insensitive identities: register, login, email
// change and OAuth linking all normalize (trim + lowercase) before comparing
// or storing. Existing rows written with mixed case must be normalized too, or
// a user who registered as "Foo@X.com" could never be found by the normalized
// lookup.
//
// Contract after this migration: exactly ONE reachable normalized row per
// mailbox. For each distinct LOWER(TRIM(email)) value the LOWEST id is
// normalized, but only when no row already holds that normalized value — so a
// pair like 'FOO@X.com' + 'Foo@x.com' can never both UPDATE into the same
// value (which would violate UNIQUE and panic the migrator). Any other
// case-variant row stays exactly as it was and is unreachable by normalized
// lookups: a known limitation awaiting a human decision, never a crash.
const LOWERCASE_EMAILS: &str = "UPDATE users SET email = LOWER(TRIM(email)) \
     WHERE id IN ( \
         SELECT MIN(u.id) FROM users AS u \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM users AS n WHERE n.email = LOWER(TRIM(u.email)) \
         ) \
         GROUP BY LOWER(TRIM(u.email)) \
     ) \
     AND email <> LOWER(TRIM(email))";

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
    async fn singles_are_normalized() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (1, 'a', ' Foo@Example.COM '), \
             (4, 'd', 'MiXeD@Y.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(email_of(&db, 1).await, "foo@example.com");
        assert_eq!(email_of(&db, 4).await, "mixed@y.com");
    }

    #[tokio::test]
    async fn both_non_normalized_pair_normalizes_lowest_id_only() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        // Neither row is normalized; the old statement updated both and crashed
        // on the UNIQUE index. Exactly the lowest id must be normalized.
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (2, 'b', 'FOO@X.com'), \
             (3, 'c', 'Foo@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(email_of(&db, 2).await, "foo@x.com");
        assert_eq!(
            email_of(&db, 3).await,
            "Foo@x.com",
            "non-lowest case-variant stays untouched (unreachable, known limitation)"
        );
    }

    #[tokio::test]
    async fn existing_lowercase_row_leaves_mixed_untouched() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (2, 'b', 'Dup@X.com'), \
             (3, 'c', 'dup@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            email_of(&db, 2).await,
            "Dup@X.com",
            "normalized form already owned by another row"
        );
        assert_eq!(email_of(&db, 3).await, "dup@x.com");
    }

    #[tokio::test]
    async fn rerun_is_idempotent() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (1, 'a', ' Foo@Example.COM '), \
             (2, 'b', 'FOO@X.com'), \
             (3, 'c', 'Foo@x.com'), \
             (4, 'd', 'Dup@X.com'), \
             (5, 'e', 'dup@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        let after_first = [
            email_of(&db, 1).await,
            email_of(&db, 2).await,
            email_of(&db, 3).await,
            email_of(&db, 4).await,
            email_of(&db, 5).await,
        ];

        Migration.up(&mgr).await.unwrap();
        let after_second = [
            email_of(&db, 1).await,
            email_of(&db, 2).await,
            email_of(&db, 3).await,
            email_of(&db, 4).await,
            email_of(&db, 5).await,
        ];

        assert_eq!(after_first, after_second, "second run must be a no-op");
        assert_eq!(after_first[0], "foo@example.com");
        assert_eq!(after_first[1], "foo@x.com");
        assert_eq!(after_first[2], "Foo@x.com");
        assert_eq!(after_first[3], "Dup@X.com");
        assert_eq!(after_first[4], "dup@x.com");
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
