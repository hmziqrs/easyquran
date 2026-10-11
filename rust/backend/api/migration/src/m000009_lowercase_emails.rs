use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Account emails become case-insensitive identities: register, login, email
// change and OAuth linking all normalize (trim + lowercase) before comparing
// or storing. Existing rows written with mixed case must be normalized too, or
// a user who registered as "Foo@X.com" could never be found by the normalized
// lookup.
//
// Two passes, in order:
//   1. SQL pass (LOWERCASE_EMAILS): SQLite LOWER()/TRIM() are ASCII-only, the
//      fast common case. For each distinct LOWER(TRIM(email)) value the LOWEST
//      id is normalized, but only when no row already holds that normalized
//      value — so a pair like 'FOO@X.com' + 'Foo@x.com' can never both UPDATE
//      into the same value (which would violate UNIQUE and panic the migrator).
//   2. Rust pass (normalize_unicode_emails): the app normalizes with Rust
//      Unicode trim() + to_lowercase(), which SQLite cannot express, so
//      non-ASCII mailboxes ('Ä@X.com') only become reachable here. Rows are
//      processed by ascending id; a candidate takes its normalized value only
//      when no row holds it and no earlier candidate claimed it (lowest id
//      wins), so this pass can never hit UNIQUE either.
//
// Contract after both passes: exactly ONE reachable normalized row per mailbox.
// Any other case-variant row stays behind — possibly with its ASCII
// lowercaseable characters already lowered by pass 1 — unreachable by
// normalized lookups: a known limitation awaiting a human decision, never a
// crash.
const LOWERCASE_EMAILS: &str = "UPDATE users SET email = LOWER(TRIM(email)) \
     WHERE id IN ( \
         SELECT MIN(u.id) FROM users AS u \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM users AS n WHERE n.email = LOWER(TRIM(u.email)) \
         ) \
         GROUP BY LOWER(TRIM(u.email)) \
     ) \
     AND email <> LOWER(TRIM(email))";

// The migration crate does not depend on ruxlog, so this is a local copy of
// `utils::email::normalize_email`; keep the two in sync.
fn normalize(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Unicode-aware normalization pass. `held` starts as every row's current email
/// and gains each value this pass claims, so a candidate can only take a value
/// no other row holds and no earlier candidate claimed — an UPDATE can never
/// violate the UNIQUE index. Returns whether any row was updated.
async fn normalize_unicode_emails(conn: &impl sea_orm::ConnectionTrait) -> Result<bool, DbErr> {
    let raw = conn
        .query_all(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT id, email FROM users ORDER BY id",
        ))
        .await?;
    let mut rows: Vec<(i32, String)> = Vec::with_capacity(raw.len());
    for row in &raw {
        rows.push((row.try_get_by_index(0)?, row.try_get_by_index(1)?));
    }

    let mut changed = false;
    let mut held: std::collections::HashSet<String> =
        rows.iter().map(|(_, email)| email.clone()).collect();
    for (id, email) in rows {
        let normalized = normalize(&email);
        if normalized == email || held.contains(&normalized) {
            // Already normalized, or another row already holds this mailbox
            // (already normalized, or normalized earlier in this pass).
            continue;
        }
        conn.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "UPDATE users SET email = ? WHERE id = ?",
            [normalized.clone().into(), id.into()],
        ))
        .await?;
        held.remove(&email);
        held.insert(normalized);
        changed = true;
    }
    Ok(changed)
}

// Safety valve only; unreachable in practice. The loop settles in at most
// three iterations: after iteration 1's Rust pass every normalize-image is
// held (the held set only ever grows — a removed old value is never a
// normalize-image, since images are Rust-stable), so the Rust pass is a no-op
// from iteration 2 on; iteration 2's SQL pass can only lower a value that is
// still not SQL-stable, so iteration 3 changes nothing. (Fuzzed over ~10k
// batches: max 3.) Should the cap ever be hit anyway, each pass remains
// individually collision-safe and the one-reachable-normalized-row invariant
// holds mid-loop — but the run would stop short of the byte-fixpoint and a
// later run would keep converging.
const MAX_NORMALIZATION_PASSES: usize = 10;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        // Loop both passes to a fixpoint in ONE run: pass 1 lowers the ASCII
        // part of a leftover that pass 2's first sweep left alone (its mailbox
        // was already claimed), so a single pair of passes is not idempotent
        // (e.g. 'Ⱥ@q.com' + 'Ⱥ@Q.COM'). Repeat until neither pass changes a row.
        for _ in 0..MAX_NORMALIZATION_PASSES {
            let sql_changed = conn
                .execute_unprepared(LOWERCASE_EMAILS)
                .await?
                .rows_affected()
                > 0;
            let rust_changed = normalize_unicode_emails(conn).await?;
            if !sql_changed && !rust_changed {
                break;
            }
        }
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

    /// Every row as (id, email), ordered by id — byte-level comparison basis.
    async fn snapshot(db: &sea_orm::DatabaseConnection) -> Vec<(i32, String)> {
        let rows = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT id, email FROM users ORDER BY id",
            ))
            .await
            .unwrap();
        rows.iter()
            .map(|row| {
                (
                    row.try_get_by_index::<i32>(0).unwrap(),
                    row.try_get_by_index::<String>(1).unwrap(),
                )
            })
            .collect()
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
        let after_first = snapshot(&db).await;
        assert_eq!(after_first[0].1, "foo@example.com");
        assert_eq!(after_first[1].1, "foo@x.com");
        assert_eq!(after_first[2].1, "Foo@x.com");
        assert_eq!(after_first[3].1, "Dup@X.com");
        assert_eq!(after_first[4].1, "dup@x.com");

        Migration.up(&mgr).await.unwrap();
        let after_second = snapshot(&db).await;

        assert_eq!(
            after_first, after_second,
            "second run must be byte-identical (true fixpoint)"
        );
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

    #[test]
    fn normalize_matches_app_semantics() {
        assert_eq!(normalize(" Foo@X.com "), "foo@x.com");
        assert_eq!(normalize("Ä@X.COM"), "ä@x.com");
        assert_eq!(
            normalize("\u{00A0}Ä@X.com\u{3000}"),
            "ä@x.com",
            "Rust trim() removes non-ASCII whitespace SQLite TRIM() cannot"
        );
    }

    #[tokio::test]
    async fn unicode_single_is_normalized() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO users (id, name, email) VALUES (1, 'a', 'Ä@X.com')")
            .await
            .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            email_of(&db, 1).await,
            "ä@x.com",
            "SQLite LOWER() cannot lowercase Ä; the Rust pass must"
        );
    }

    #[tokio::test]
    async fn unicode_collision_pair_normalizes_lowest_id_only() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        // Both rows normalize to 'ä@x.com'. The SQL pass leaves them at
        // 'ä@x.com' / 'Ä@x.com' (distinct), then the Rust pass must give the
        // value to the lowest id only — never UNIQUE-crash.
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (2, 'b', 'ä@X.com'), \
             (3, 'c', 'Ä@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(email_of(&db, 2).await, "ä@x.com");
        assert_eq!(
            email_of(&db, 3).await,
            "Ä@x.com",
            "non-lowest case-variant stays untouched (unreachable, known limitation)"
        );

        // Rerun with the leftover variant present: must stay a no-op, no UNIQUE.
        Migration.up(&mgr).await.unwrap();
        assert_eq!(email_of(&db, 2).await, "ä@x.com");
        assert_eq!(email_of(&db, 3).await, "Ä@x.com");
    }

    #[tokio::test]
    async fn unicode_pair_with_normalized_row_leaves_mixed_untouched() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        // The normalized form is already held by id 3, so id 2 may not take it.
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (2, 'b', 'Ä@X.com'), \
             (3, 'c', 'ä@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            email_of(&db, 2).await,
            "Ä@x.com",
            "normalized form already owned by another row"
        );
        assert_eq!(email_of(&db, 3).await, "ä@x.com");
    }

    #[tokio::test]
    async fn unicode_whitespace_is_trimmed() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        let raw = "\u{00A0}Ä@X.com\u{3000}";
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO users (id, name, email) VALUES (1, 'a', ?)",
            [raw.into()],
        ))
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(
            email_of(&db, 1).await,
            "ä@x.com",
            "NBSP + ideographic space must be trimmed by the Rust pass"
        );
    }

    #[tokio::test]
    async fn rerun_is_idempotent_with_unicode() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (1, 'a', 'Ä@X.com'), \
             (2, 'b', 'Ö@Y.com'), \
             (3, 'c', ' Ü@Z.com ')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        let after_first = snapshot(&db).await;
        assert_eq!(after_first[0].1, "ä@x.com");
        assert_eq!(after_first[1].1, "ö@y.com");
        assert_eq!(after_first[2].1, "ü@z.com");

        Migration.up(&mgr).await.unwrap();
        let after_second = snapshot(&db).await;

        assert_eq!(
            after_first, after_second,
            "second run must be byte-identical (true fixpoint)"
        );
    }

    #[tokio::test]
    async fn unicode_leftover_ascii_lowering_is_fixed_in_one_run() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        // Round-3 repro: run #1 pass 2 normalizes id 1 and leaves id 2 alone
        // (its mailbox is claimed), then a second run's pass 1 used to lower
        // id 2's ASCII part. One up() must already reach that fixpoint.
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (1, 'a', 'Ⱥ@q.com'), \
             (2, 'b', 'Ⱥ@Q.COM')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();

        assert_eq!(email_of(&db, 1).await, "ⱥ@q.com");
        assert_eq!(
            email_of(&db, 2).await,
            "Ⱥ@q.com",
            "leftover keeps only its ASCII-lowered form after one run"
        );

        let after_first = snapshot(&db).await;
        Migration.up(&mgr).await.unwrap();
        assert_eq!(
            after_first,
            snapshot(&db).await,
            "second run must change zero rows anywhere"
        );
    }

    #[tokio::test]
    async fn mixed_batch_reaches_fixpoint_in_one_run() {
        let db = mem_db().await;
        db.execute_unprepared(
            "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             name TEXT NOT NULL, email TEXT NOT NULL UNIQUE)",
        )
        .await
        .unwrap();
        // Nasty mix: Unicode pair, the repro pair, ASCII pair, whitespace, and
        // an already-normalized row. One up() must settle all of it without
        // hitting the iteration cap (observable: a second up() is a no-op).
        db.execute_unprepared(
            "INSERT INTO users (id, name, email) VALUES \
             (1, 'a', 'Ä@X.com'), \
             (2, 'b', 'ä@X.com'), \
             (3, 'c', 'Ⱥ@q.com'), \
             (4, 'd', 'Ⱥ@Q.COM'), \
             (5, 'e', ' Z@z.com '), \
             (6, 'f', 'z@Z.com'), \
             (7, 'g', 'ok@x.com')",
        )
        .await
        .unwrap();

        let mgr = SchemaManager::new(&db);
        Migration.up(&mgr).await.unwrap();
        let after_first = snapshot(&db).await;

        Migration.up(&mgr).await.unwrap();
        let after_second = snapshot(&db).await;

        assert_eq!(
            after_first, after_second,
            "fixpoint must be reached in the first run, not by a later one"
        );
        assert_eq!(
            after_first[0].1, "Ä@x.com",
            "SQL pass pre-normalized id 2; id 1 stays blocked"
        );
        assert_eq!(after_first[1].1, "ä@x.com");
        assert_eq!(after_first[2].1, "ⱥ@q.com");
        assert_eq!(after_first[3].1, "Ⱥ@q.com");
        assert_eq!(after_first[4].1, "z@z.com");
        assert_eq!(after_first[5].1, "z@Z.com");
        assert_eq!(after_first[6].1, "ok@x.com");
    }
}
