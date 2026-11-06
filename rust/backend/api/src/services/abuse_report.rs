//! Abuse measurement summary (A5). Ten figures, each a value or the string
//! "not measurable". A figure is never invented: a failed query, a missing table
//! or a counter this process cannot see reads as "not measurable".
//!
//! Two callers share this code. The live route (`GET /admin/abuse/v1/summary`)
//! supplies in-process counters. The offline report (`just abuse-report`) runs
//! on a WAL-safe copy with no process behind it, so it passes `None` for the
//! in-process counters and those figures read "not measurable".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr,
    EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Statement, Value,
};
use serde_json::{json, Map, Value as Json};
use tracing::warn;

use crate::db::sea_models::{comment_flag, email_suppression, user, user_ban};

pub const NOT_MEASURABLE: &str = "not measurable";
pub const WINDOW_DAYS: i64 = 7;

/// Counters that exist only inside a running api process.
pub struct InProcessCounters {
    /// `(missing, mismatch, untrusted_peer)` from `telemetry::ingress_rejection_totals`.
    pub ingress_rejections: (u64, u64, u64),
    /// Route-level 429s by path label since boot, from `rux_request_gate::blocked_by_path`.
    pub rate_limited_by_path: Vec<(String, u64)>,
}

fn not_measurable() -> Json {
    Json::String(NOT_MEASURABLE.to_string())
}

fn figure(result: Result<Json, DbErr>, name: &str) -> Json {
    match result {
        Ok(value) => value,
        Err(e) => {
            warn!(figure = name, error = %e, "abuse summary figure not measurable");
            not_measurable()
        }
    }
}

/// Path of the SQLite file behind a `sqlite:` URL, or `None` for in-memory or
/// non-SQLite URLs. Query parameters are dropped.
pub fn sqlite_file_path(url: &str) -> Option<PathBuf> {
    let path = url.strip_prefix("sqlite:")?.split('?').next()?;
    if path.is_empty() || path == ":memory:" {
        return None;
    }
    Some(PathBuf::from(path))
}

/// Builds the summary. `now` anchors every window. `db_path` feeds the file-size
/// figure; `None` makes that figure "not measurable".
pub async fn collect(
    db: &DatabaseConnection,
    db_path: Option<&Path>,
    in_process: Option<&InProcessCounters>,
    now: DateTime<Utc>,
) -> Json {
    let now_fixed: DateTime<FixedOffset> = now.fixed_offset();
    let cutoff_fixed = now_fixed - ChronoDuration::days(WINDOW_DAYS);
    let cutoff_epoch = now.timestamp() - WINDOW_DAYS * 86_400;

    let mut out = Map::new();
    out.insert("generatedAt".into(), json!(now.timestamp()));
    out.insert("windowDays".into(), json!(WINDOW_DAYS));

    // 1. Active account bans.
    out.insert(
        "activeAccountBans".into(),
        figure(
            active_account_bans(db, now_fixed).await.map(|n| json!(n)),
            "activeAccountBans",
        ),
    );

    // 2. Bans created and revoked in the window.
    out.insert(
        "bansCreatedAndRevokedLast7d".into(),
        figure(
            bans_created_and_revoked(db, cutoff_fixed).await,
            "bansCreatedAndRevokedLast7d",
        ),
    );

    // 3. Blocks issued in the window, by key class (from the durable ledger).
    out.insert(
        "blocksByKeyClassLast7d".into(),
        figure(
            blocks_by_key_class(db, cutoff_epoch).await,
            "blocksByKeyClassLast7d",
        ),
    );

    // 4. Route-level 429s. Process-local, so only a live process can answer.
    out.insert(
        "rateLimited429ByRouteSinceBoot".into(),
        match in_process {
            Some(c) => Json::Object(
                c.rate_limited_by_path
                    .iter()
                    .map(|(path, n)| (path.clone(), json!(n)))
                    .collect(),
            ),
            None => not_measurable(),
        },
    );

    // 5. Signups by day and email domain in the window.
    out.insert(
        "signupsByDayAndDomainLast7d".into(),
        figure(
            signups_by_day_and_domain(db, cutoff_fixed).await,
            "signupsByDayAndDomainLast7d",
        ),
    );

    // 6. Open comment flags. A flag row exists until its comment is cleared
    //    (`comment_flag::clear_flags`), so the row count is the open count.
    out.insert(
        "openCommentFlags".into(),
        figure(
            comment_flag::Entity::find()
                .count(db)
                .await
                .map(|n| json!(n)),
            "openCommentFlags",
        ),
    );

    // 7. Mail suppression entries.
    out.insert(
        "mailSuppressions".into(),
        figure(mail_suppressions(db).await, "mailSuppressions"),
    );

    // 8. Retention sweeper deletions. The sweeper ships with A32, so no row has
    //    ever been deleted by one: the true count is zero.
    out.insert("retentionSweeperDeletions".into(), json!(0));
    out.insert(
        "retentionSweeperNote".into(),
        json!("sweeper ships with A32; 0 is the count of sweeper deletes to date"),
    );

    // 9. Database and WAL bytes.
    out.insert("dbAndWalBytes".into(), db_and_wal_bytes(db_path));

    // 10. Disk-cache hit rate. The api has no disk cache counter; the reader
    //     disk cache is web-side and reported by its own health route.
    out.insert("diskCacheHitRate".into(), not_measurable());

    // Ingress rejections are part of the process-local picture.
    out.insert(
        "ingressRejectionsSinceBoot".into(),
        match in_process {
            Some(c) => {
                let (missing, mismatch, untrusted) = c.ingress_rejections;
                json!({
                    "markerMissing": missing,
                    "markerMismatch": mismatch,
                    "untrustedPeer": untrusted,
                })
            }
            None => not_measurable(),
        },
    );

    Json::Object(out)
}

async fn active_account_bans(
    db: &DatabaseConnection,
    now: DateTime<FixedOffset>,
) -> Result<i64, DbErr> {
    let n = user_ban::Entity::find()
        .filter(user_ban::Column::RevokedAt.is_null())
        .filter(
            Condition::any()
                .add(user_ban::Column::ExpiresAt.is_null())
                .add(user_ban::Column::ExpiresAt.gt(now)),
        )
        .count(db)
        .await?;
    Ok(n as i64)
}

async fn bans_created_and_revoked(
    db: &DatabaseConnection,
    cutoff: DateTime<FixedOffset>,
) -> Result<Json, DbErr> {
    let created = user_ban::Entity::find()
        .filter(user_ban::Column::CreatedAt.gte(cutoff))
        .count(db)
        .await?;
    let revoked = user_ban::Entity::find()
        .filter(user_ban::Column::RevokedAt.gte(cutoff))
        .count(db)
        .await?;
    Ok(json!({ "created": created, "revoked": revoked }))
}

async fn blocks_by_key_class(db: &DatabaseConnection, cutoff_epoch: i64) -> Result<Json, DbErr> {
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT key_class, COUNT(*) FROM rate_limit_events \
             WHERE event = 'block_created' AND issued_at >= ? \
             GROUP BY key_class ORDER BY key_class",
            vec![Value::from(cutoff_epoch)],
        ))
        .await?;
    let mut map = Map::new();
    for row in rows {
        let class: String = row.try_get_by_index(0)?;
        let n: i64 = row.try_get_by_index(1)?;
        map.insert(class, json!(n));
    }
    Ok(Json::Object(map))
}

async fn signups_by_day_and_domain(
    db: &DatabaseConnection,
    cutoff: DateTime<FixedOffset>,
) -> Result<Json, DbErr> {
    // Read through the entity so the stored timestamp format is decoded by
    // sea-orm, never compared as text. Only the day and domain leave this fn.
    let rows: Vec<(DateTime<FixedOffset>, String)> = user::Entity::find()
        .select_only()
        .column(user::Column::CreatedAt)
        .column(user::Column::Email)
        .filter(user::Column::CreatedAt.gte(cutoff))
        .into_tuple()
        .all(db)
        .await?;

    let mut counts: BTreeMap<(String, String), u64> = BTreeMap::new();
    for (created_at, email) in rows {
        let day = created_at
            .with_timezone(&Utc)
            .format("%Y-%m-%d")
            .to_string();
        let domain = email.rsplit('@').next().unwrap_or("").to_ascii_lowercase();
        *counts.entry((day, domain)).or_insert(0) += 1;
    }
    let list: Vec<Json> = counts
        .into_iter()
        .map(|((day, domain), n)| json!({ "day": day, "domain": domain, "count": n }))
        .collect();
    Ok(Json::Array(list))
}

async fn mail_suppressions(db: &DatabaseConnection) -> Result<Json, DbErr> {
    let total = email_suppression::Entity::find().count(db).await?;
    let permanent = email_suppression::Entity::find()
        .filter(email_suppression::Column::Permanent.eq(true))
        .count(db)
        .await?;
    Ok(json!({ "total": total, "permanent": permanent }))
}

fn db_and_wal_bytes(db_path: Option<&Path>) -> Json {
    let Some(path) = db_path else {
        return not_measurable();
    };
    let db_len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => {
            warn!(error = %e, "abuse summary: db file size not measurable");
            return not_measurable();
        }
    };
    let mut wal_os = path.as_os_str().to_owned();
    wal_os.push("-wal");
    // A missing WAL file means the WAL is empty (checkpointed), so zero is a
    // measured value here, not a guess.
    let wal_len = match std::fs::metadata(PathBuf::from(wal_os)) {
        Ok(m) => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => {
            warn!(error = %e, "abuse summary: wal file size not measurable");
            return json!({ "db": db_len, "wal": NOT_MEASURABLE });
        }
    };
    json!({ "db": db_len, "wal": wal_len })
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectOptions, Database};

    async fn migrated_db() -> DatabaseConnection {
        let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
        opt.max_connections(1);
        let db = Database::connect(opt).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    fn at(epoch: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(epoch, 0).unwrap()
    }

    #[test]
    fn sqlite_path_strips_scheme_and_query() {
        assert_eq!(
            sqlite_file_path("sqlite:./data/easyquran.db?mode=rwc"),
            Some(PathBuf::from("./data/easyquran.db"))
        );
        assert_eq!(sqlite_file_path("sqlite::memory:"), None);
        assert_eq!(sqlite_file_path("postgres://x"), None);
    }

    #[tokio::test]
    async fn offline_report_marks_process_counters_not_measurable() {
        let db = migrated_db().await;
        let report = collect(&db, None, None, at(1_800_000_000)).await;
        assert_eq!(report["rateLimited429ByRouteSinceBoot"], NOT_MEASURABLE);
        assert_eq!(report["ingressRejectionsSinceBoot"], NOT_MEASURABLE);
        assert_eq!(report["diskCacheHitRate"], NOT_MEASURABLE);
        assert_eq!(report["dbAndWalBytes"], NOT_MEASURABLE);
        assert_eq!(report["activeAccountBans"], json!(0));
        assert_eq!(report["openCommentFlags"], json!(0));
        assert_eq!(report["retentionSweeperDeletions"], json!(0));
    }

    #[tokio::test]
    async fn every_figure_is_a_value_or_the_marker() {
        let db = migrated_db().await;
        let counters = InProcessCounters {
            ingress_rejections: (1, 2, 3),
            rate_limited_by_path: vec![("/quran/x".into(), 4)],
        };
        let report = collect(&db, None, Some(&counters), at(1_800_000_000)).await;
        let obj = report.as_object().unwrap();
        for key in [
            "activeAccountBans",
            "bansCreatedAndRevokedLast7d",
            "blocksByKeyClassLast7d",
            "rateLimited429ByRouteSinceBoot",
            "signupsByDayAndDomainLast7d",
            "openCommentFlags",
            "mailSuppressions",
            "retentionSweeperDeletions",
            "dbAndWalBytes",
            "diskCacheHitRate",
        ] {
            let v = &obj[key];
            let ok = v.is_number() || v.is_object() || v.is_array() || v == NOT_MEASURABLE;
            assert!(ok, "{key} is neither a value nor the marker: {v}");
        }
        assert_eq!(
            report["rateLimited429ByRouteSinceBoot"]["/quran/x"],
            json!(4)
        );
        assert_eq!(
            report["ingressRejectionsSinceBoot"]["untrustedPeer"],
            json!(3)
        );
    }

    #[tokio::test]
    async fn ledger_counts_only_block_created_inside_window() {
        let db = migrated_db().await;
        let now = 1_800_000_000i64;
        let rows = [
            ("block_created", "ip4_unit", now - 10),
            ("block_created", "ip4_unit", now - 20),
            ("block_created", "ip6_unit", now - 30),
            ("block_expired", "ip4_unit", now - 40),
            ("block_created", "ip4_unit", now - 8 * 86_400),
        ];
        for (event, class, issued) in rows {
            db.execute_unprepared(&format!(
                "INSERT INTO rate_limit_events \
                 (key, key_class, scope, event, issued_at, expires_at, retention_class) \
                 VALUES ('k', '{class}', 'Temp', '{event}', {issued}, {issued}, 'abuse_180d')"
            ))
            .await
            .unwrap();
        }
        let report = collect(&db, None, None, at(now)).await;
        assert_eq!(
            report["blocksByKeyClassLast7d"],
            json!({ "ip4_unit": 2, "ip6_unit": 1 })
        );
    }

    #[tokio::test]
    async fn signups_group_by_day_and_domain() {
        let db = migrated_db().await;
        let now = at(1_800_000_000);
        db.execute_unprepared(&format!(
            "INSERT INTO users (name, email, password, is_verified, role, two_fa_enabled, \
             session_auth_secret, created_at, updated_at) VALUES \
             ('a', 'a@gmail.com', NULL, 1, 'user', 0, 's1', '{t}', '{t}'), \
             ('b', 'b@gmail.com', NULL, 1, 'user', 0, 's2', '{t}', '{t}'), \
             ('c', 'c@example.org', NULL, 1, 'user', 0, 's3', '{t}', '{t}'), \
             ('old', 'old@example.org', NULL, 1, 'user', 0, 's4', '{old}', '{old}')",
            t = now.to_rfc3339(),
            old = (now - ChronoDuration::days(30)).to_rfc3339(),
        ))
        .await
        .unwrap();
        let report = collect(&db, None, None, now).await;
        let list = report["signupsByDayAndDomainLast7d"].as_array().unwrap();
        let total: u64 = list.iter().map(|r| r["count"].as_u64().unwrap()).sum();
        assert_eq!(total, 3, "the 30-day-old signup is outside the window");
        assert!(list
            .iter()
            .any(|r| r["domain"] == "gmail.com" && r["count"] == json!(2)));
    }

    #[test]
    fn file_bytes_report_zero_wal_when_absent() {
        let dir = std::env::temp_dir().join(format!("abuse-bytes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_file = dir.join("copy.db");
        std::fs::write(&db_file, vec![0u8; 4096]).unwrap();
        let _ = std::fs::remove_file(dir.join("copy.db-wal"));
        let bytes = db_and_wal_bytes(Some(&db_file));
        assert_eq!(bytes, json!({ "db": 4096, "wal": 0 }));
        assert_eq!(
            db_and_wal_bytes(Some(&dir.join("missing.db"))),
            NOT_MEASURABLE
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
