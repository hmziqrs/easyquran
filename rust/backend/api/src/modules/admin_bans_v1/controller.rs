use std::collections::HashMap;

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use subtle::ConstantTimeEq;
use tower_sessions::Session;
use tracing::{error, info, instrument, warn};
use validator::Validate;

use rux_auth::{auth_requirements, check_requirements, AuthSession as GenAuthSession};
use rux_request_gate::{BlockScope, BucketSnapshot, RateLimitStore};

use crate::error::{ErrorCode, ErrorResponse};
use crate::extractors::ValidatedJson;
use crate::services::auth::AuthBackend;
use crate::AppState;

use super::dto::{BanListResponse, BanRow, BanUnit, DeleteBanPayload, ExportResponse, ExportRow};

const BAN_KEY_PREFIX: &str = "quran-ban:";

fn epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// Merge L1 (live buckets) with L2 (persisted rows) and keep only active bans:
// keys shaped `quran-ban:{unit}` whose {unit} parses as a canonical BanUnit and
// whose block is still in the future. Email, user-id, totp, and fixed-rate keys
// never start with `quran-ban:` AND parse as a /32 or /64 unit, so they are
// excluded twice over. L1 is the freshest source (L2 lags <= flush interval);
// when both carry a unit, the later block_until wins.
pub(crate) fn filter_active_ban_rows(snaps: &[BucketSnapshot], now: i64) -> Vec<BanRow> {
    let mut merged: HashMap<String, (BlockScope, i64)> = HashMap::new();
    for snap in snaps {
        let Some(unit) = snap.key.strip_prefix(BAN_KEY_PREFIX) else {
            continue;
        };
        if BanUnit::parse(unit).is_none() {
            continue;
        }
        if snap.block_until_at <= now {
            continue;
        }
        let entry = merged
            .entry(unit.to_string())
            .or_insert((snap.block_scope, snap.block_until_at));
        if snap.block_until_at > entry.1 {
            *entry = (snap.block_scope, snap.block_until_at);
        }
    }
    let mut rows: Vec<BanRow> = merged
        .into_iter()
        .map(|(unit, (scope, until))| BanRow {
            ban_unit: unit,
            scope: scope.as_str().to_string(),
            block_until_at: until,
        })
        .collect();
    rows.sort_by(|a, b| a.ban_unit.cmp(&b.ban_unit));
    rows
}

async fn collect_active_ban_rows(state: &AppState) -> Result<Vec<BanRow>, ErrorResponse> {
    let now = epoch_secs();
    let mut snaps: Vec<BucketSnapshot> = state.gate_store.snapshot();
    snaps.extend(crate::services::rate_limit_store::load(&state.sea_db).await);
    Ok(filter_active_ban_rows(&snaps, now))
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct BanListQuery {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub per_page: Option<u32>,
}

#[instrument(skip(state, _auth))]
pub async fn list_bans(
    State(state): State<AppState>,
    _auth: crate::services::auth::AuthSession,
    query: crate::extractors::ValidatedQuery<BanListQuery>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let page = query.page.unwrap_or(1).clamp(1, 10_000);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let rows = collect_active_ban_rows(&state).await?;
    let total = rows.len();
    let start = ((page - 1) as usize).saturating_mul(per_page as usize);
    let data: Vec<BanRow> = rows
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();
    info!(total, page, per_page, "admin bans listed");
    Ok((
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(BanListResponse {
            data,
            total,
            page,
            per_page,
        }),
    ))
}

#[instrument(skip(state, _auth, payload), fields(ban_unit))]
pub async fn delete_ban(
    State(state): State<AppState>,
    _auth: crate::services::auth::AuthSession,
    payload: ValidatedJson<DeleteBanPayload>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let unit = BanUnit::parse(&payload.ban_unit).ok_or_else(|| {
        ErrorResponse::new(ErrorCode::InvalidInput).with_message("invalid banUnit")
    })?;
    let unit_str = unit.canonical();
    tracing::Span::current().record("ban_unit", unit_str.as_str());
    let ban_key = format!("quran-ban:{unit_str}");
    let fixed_key = format!("ratelimit:{unit_str}:quran-v1");

    // Serialize durable writes against the periodic flush: a snapshot taken
    // before this point has already flushed, and one taken after will see the L1
    // bucket gone, so neither can resurrect the row. The bucket mutex is never
    // held across SQLite I/O.
    let _guard = crate::services::rate_limit_store::PERSISTENCE_LOCK
        .lock()
        .await;

    // 1. Durable delete FIRST, in one transaction. A DB failure returns failure
    //    WITHOUT clearing L1 — the ban stays enforced and the op is safe to retry.
    if let Err(e) =
        crate::services::rate_limit_store::delete_ban_rows(&state.sea_db, &ban_key, &fixed_key)
            .await
    {
        error!(error = %e, ban_unit = %unit_str, "L2 ban delete failed; L1 ban preserved");
        return Err(ErrorResponse::new(ErrorCode::ServiceUnavailable)
            .with_message("Failed to lift ban (durable delete failed); ban remains enforced"));
    }

    // 2. L2 committed. Now clear L1: the ban block, the suspicious history
    //    (attempts), and the fixed-window count that would otherwise recreate it.
    let _ = state.gate_store.clear_limit(&ban_key).await;
    let _ = state.gate_store.clear_limit(&fixed_key).await;

    info!(ban_unit = %unit_str, "ban lifted (L2 + L1 cleared)");
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "banUnit": unit_str, "lifted": true })),
    ))
}

#[instrument(skip(state))]
pub async fn export_bans(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let rows = collect_active_ban_rows(&state).await?;
    let bans: Vec<ExportRow> = rows
        .into_iter()
        .map(|r| ExportRow {
            ban_unit: r.ban_unit,
            scope: r.scope,
            expires_at: r.block_until_at,
        })
        .collect();
    info!(count = bans.len(), "ban export emitted");
    Ok((
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(ExportResponse { bans }),
    ))
}

// Export accepts EITHER the admin session ACL (human) OR a read-only bearer
// token (machine). The token is compared constant-time and is NEVER accepted by
// the mutation routes — list/delete sit behind the session admin ACL, not this
// guard. An invalid token rejects without falling through to the session check,
// so a leaked token guess cannot be probed via the session path.
pub async fn admin_or_export_token(
    Extension(state): Extension<AppState>,
    session: Session,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    if let Some(provided) = bearer_token(&req) {
        let configured = state.settings.rate_limit.ban_export_token.as_bytes();
        if !configured.is_empty() && token_matches(provided.as_bytes(), configured) {
            return Ok(next.run(req).await);
        }
        warn!("invalid or unset BAN_EXPORT_TOKEN presented for /admin/bans/export");
        return Err(unauthorized());
    }

    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    let mut auth = GenAuthSession::new(backend, session).await;
    if let Err(e) = check_requirements(
        &mut auth,
        &auth_requirements()
            .authenticated()
            .verified()
            .not_banned()
            .role_min(crate::middlewares::auth_guard::ROLE_ADMIN),
    )
    .await
    {
        return Err(e.into_response());
    }
    Ok(next.run(req).await)
}

fn bearer_token(req: &Request) -> Option<String> {
    let raw = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let stripped = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    Some(stripped.to_string())
}

fn token_matches(provided: &[u8], configured: &[u8]) -> bool {
    if configured.is_empty() {
        return false;
    }
    provided.len() == configured.len() && bool::from(provided.ct_eq(configured))
}

fn unauthorized() -> Response {
    let mut r = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    r.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        "Bearer realm=\"admin-bans\""
            .parse()
            .expect("static header value"),
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use rux_request_gate::{AbuseLimiterConfig, InMemoryStore, RateLimitStore};

    const CFG: AbuseLimiterConfig = AbuseLimiterConfig {
        temp_block_attempts: 3,
        temp_block_range: 60,
        temp_block_duration: 300,
        block_retry_limit: 5,
        block_range: 3600,
        block_duration: 86400,
    };

    #[test]
    fn ban_unit_round_trips_through_json_without_url_encoding() {
        let v4 = BanUnit::parse("203.0.113.5/32").unwrap();
        let s = serde_json::to_string(&v4).unwrap();
        assert_eq!(s, "\"203.0.113.5/32\"");
        let back: BanUnit = serde_json::from_str(&s).unwrap();
        assert_eq!(back, v4);

        let v6 = BanUnit::parse("2001:db8::/64").unwrap();
        let s6 = serde_json::to_string(&v6).unwrap();
        assert_eq!(s6, "\"2001:db8::/64\"");
        let back6: BanUnit = serde_json::from_str(&s6).unwrap();
        assert_eq!(back6, v6);
    }

    #[test]
    fn ipv6_unit_truncates_host_bits_to_canonical_64() {
        let u = BanUnit::from_ip("2001:db8::1".parse().unwrap()).unwrap();
        assert_eq!(u.canonical(), "2001:db8::/64");
        // All addresses in the same /64 collapse to one unit.
        let u2 = BanUnit::from_ip("2001:db8::abcd".parse().unwrap()).unwrap();
        assert_eq!(u, u2);
    }

    #[test]
    fn ipv4_unit_is_canonical_32() {
        let u = BanUnit::from_ip("203.0.113.5".parse().unwrap()).unwrap();
        assert_eq!(u.canonical(), "203.0.113.5/32");
    }

    #[test]
    fn ban_unit_rejects_wrong_widths_and_non_ip_keys() {
        assert!(BanUnit::parse("203.0.113.5/24").is_none());
        assert!(BanUnit::parse("2001:db8::/48").is_none());
        assert!(BanUnit::parse("2001:db8::/128").is_none());
        assert!(BanUnit::parse("user@example.com").is_none());
        assert!(BanUnit::parse("totp:42").is_none());
        assert!(BanUnit::parse("login:someone").is_none());
        assert!(BanUnit::parse("not even a cidr").is_none());
    }

    fn snap(key: &str, block_until_at: i64, scope: BlockScope) -> BucketSnapshot {
        BucketSnapshot {
            key: key.to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at,
            block_scope: scope,
        }
    }

    #[test]
    fn filter_excludes_email_user_id_totp_and_fixed_rate_keys() {
        let now = 1000;
        let future = now + 3600;
        let snaps = vec![
            snap("quran-ban:203.0.113.5/32", future, BlockScope::Long),
            snap("quran-ban:2001:db8::/64", future, BlockScope::Temp),
            // Every other key class must be excluded — even if "active".
            snap("login:user@example.com", future, BlockScope::Temp),
            snap("login:42", future, BlockScope::Temp),
            snap("totp:7", future, BlockScope::Temp),
            snap(
                "ratelimit:203.0.113.5/32:quran-v1",
                future,
                BlockScope::Temp,
            ),
            // A quran-ban: prefix whose suffix is NOT a valid unit is excluded too.
            snap("quran-ban:garbage", future, BlockScope::Temp),
            // Expired ban row is excluded.
            snap("quran-ban:198.51.100.7/32", now - 10, BlockScope::Temp),
        ];

        let rows = filter_active_ban_rows(&snaps, now);
        let units: Vec<String> = rows.iter().map(|r| r.ban_unit.clone()).collect();
        assert!(units.contains(&"203.0.113.5/32".to_string()));
        assert!(units.contains(&"2001:db8::/64".to_string()));
        assert_eq!(
            rows.len(),
            2,
            "only the two valid canonical ban units should appear"
        );
    }

    #[test]
    fn filter_dedupes_l1_and_l2_keeping_the_fresher_block() {
        let now = 1000;
        let l2_until = now + 100;
        let l1_until = now + 3600;
        let snaps = vec![
            snap("quran-ban:203.0.113.5/32", l2_until, BlockScope::Temp),
            snap("quran-ban:203.0.113.5/32", l1_until, BlockScope::Long),
        ];
        let rows = filter_active_ban_rows(&snaps, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].block_until_at, l1_until);
        assert_eq!(rows[0].scope, "Long");
    }

    #[test]
    fn token_matches_is_constant_time_and_exact() {
        let configured = b"super-secret-export-token";
        assert!(token_matches(b"super-secret-export-token", configured));
        assert!(!token_matches(b"wrong", configured));
        assert!(!token_matches(b"super-secret-export-tokem", configured));
        // An unset token authenticates nothing.
        assert!(!token_matches(b"anything", b""));
        assert!(!token_matches(b"", b""));
    }

    #[tokio::test]
    async fn successful_delete_clears_l1_l2_history_and_fixed_count() {
        use crate::services::rate_limit_store;
        use migration::{Migrator, MigratorTrait};
        use sea_orm::{ConnectOptions, Database, DatabaseConnection};

        async fn mem_db() -> DatabaseConnection {
            let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
            opt.max_connections(1);
            let db = Database::connect(opt).await.unwrap();
            // Fixture schema comes from the real m000002 migration, not the
            // store's duplicate ensure_table() DDL — see rate_limit_store tests.
            Migrator::up(&db, None).await.unwrap();
            db
        }

        let db = mem_db().await;
        let store = InMemoryStore::default();
        let now = epoch_secs();
        let fut = now + 3600;

        // Plant an active Long ban + a fixed-rate count in BOTH layers, plus a
        // couple of qualifying attempts (suspicious history) in L1.
        store.restore(vec![BucketSnapshot {
            key: "quran-ban:203.0.113.5/32".to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at: fut,
            block_scope: BlockScope::Long,
        }]);
        // abuse_check appends an attempt; run it once the block is active so the
        // bucket carries suspicious history we can later prove is cleared.
        let _ = store
            .abuse_check("quran-ban:203.0.113.5/32", CFG)
            .await
            .unwrap();
        let _ = store
            .incr_expire(
                "ratelimit:203.0.113.5/32:quran-v1",
                std::time::Duration::from_secs(60),
            )
            .await
            .unwrap();
        // Bump the fixed count several times so a reset is observable: if the
        // delete failed to clear the L1 fixed bucket, a fresh increment would
        // continue from this count instead of restarting at 1.
        for _ in 0..4 {
            let _ = store
                .incr_expire(
                    "ratelimit:203.0.113.5/32:quran-v1",
                    std::time::Duration::from_secs(60),
                )
                .await
                .unwrap();
        }
        rate_limit_store::flush(&db, store.snapshot())
            .await
            .unwrap();
        assert!(store
            .ban_status("quran-ban:203.0.113.5/32")
            .await
            .unwrap()
            .is_some());

        let ban_key = "quran-ban:203.0.113.5/32";
        let fixed_key = "ratelimit:203.0.113.5/32:quran-v1";
        let _guard = rate_limit_store::PERSISTENCE_LOCK.lock().await;
        rate_limit_store::delete_ban_rows(&db, ban_key, fixed_key)
            .await
            .unwrap();
        let _ = store.clear_limit(ban_key).await;
        let _ = store.clear_limit(fixed_key).await;

        // L1 ban gone, suspicious history gone, fixed count reset.
        assert!(store.ban_status(ban_key).await.unwrap().is_none());
        assert!(store.snapshot().iter().all(|s| s.key != ban_key));
        assert!(store.snapshot().iter().all(|s| s.key != fixed_key));
        // L2 rows gone.
        let loaded = rate_limit_store::load(&db).await;
        assert!(loaded.iter().all(|r| r.key != ban_key));
        assert!(loaded.iter().all(|r| r.key != fixed_key));
        // The fixed count was reset: a fresh increment starts at 1, not 10.
        let (c, _) = store
            .incr_expire(fixed_key, std::time::Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(c, 1);
    }

    #[tokio::test]
    async fn failed_l2_transaction_preserves_the_active_l1_ban() {
        use crate::services::rate_limit_store;
        use migration::{Migrator, MigratorTrait};
        use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

        async fn mem_db() -> DatabaseConnection {
            let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
            opt.max_connections(1);
            let db = Database::connect(opt).await.unwrap();
            Migrator::up(&db, None).await.unwrap();
            db
        }

        let db = mem_db().await;
        let store = InMemoryStore::default();
        let fut = epoch_secs() + 3600;
        store.restore(vec![BucketSnapshot {
            key: "quran-ban:198.51.100.7/32".to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at: fut,
            block_scope: BlockScope::Temp,
        }]);

        // Force the L2 transaction to fail by dropping the table out from under it.
        db.execute_unprepared("DROP TABLE rate_limit_state")
            .await
            .unwrap();

        let ban_key = "quran-ban:198.51.100.7/32";
        let fixed_key = "ratelimit:198.51.100.7/32:quran-v1";
        let _guard = rate_limit_store::PERSISTENCE_LOCK.lock().await;
        let res = rate_limit_store::delete_ban_rows(&db, ban_key, fixed_key).await;

        // DB failure: the handler returns failure and MUST NOT clear L1 — so we
        // do not call clear_limit here, mirroring delete_ban's control flow.
        assert!(res.is_err(), "L2 delete on a dropped table must fail");

        // The ban is still actively enforced in L1.
        assert!(store.ban_status(ban_key).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn flush_after_delete_cannot_resurrect_the_row() {
        use crate::services::rate_limit_store;
        use migration::{Migrator, MigratorTrait};
        use sea_orm::{ConnectOptions, Database, DatabaseConnection};

        async fn mem_db() -> DatabaseConnection {
            let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
            opt.max_connections(1);
            let db = Database::connect(opt).await.unwrap();
            Migrator::up(&db, None).await.unwrap();
            db
        }

        let db = mem_db().await;
        let store = InMemoryStore::default();
        let fut = epoch_secs() + 3600;
        store.restore(vec![BucketSnapshot {
            key: "quran-ban:203.0.113.5/32".to_string(),
            fixed_count: 0,
            fixed_expires_at: 0,
            block_until_at: fut,
            block_scope: BlockScope::Long,
        }]);
        rate_limit_store::flush(&db, store.snapshot())
            .await
            .unwrap();

        let ban_key = "quran-ban:203.0.113.5/32";
        let fixed_key = "ratelimit:203.0.113.5/32:quran-v1";
        {
            let _guard = rate_limit_store::PERSISTENCE_LOCK.lock().await;
            rate_limit_store::delete_ban_rows(&db, ban_key, fixed_key)
                .await
                .unwrap();
            let _ = store.clear_limit(ban_key).await;
            let _ = store.clear_limit(fixed_key).await;
        }

        // A periodic flush that runs AFTER the delete snapshots the (now empty)
        // L1 and cannot re-insert the lifted ban; its DELETE only reaps expired
        // rows, so the active row stays gone.
        rate_limit_store::flush(&db, store.snapshot())
            .await
            .unwrap();
        let loaded = rate_limit_store::load(&db).await;
        assert!(loaded.iter().all(|r| r.key != ban_key));
    }
}
