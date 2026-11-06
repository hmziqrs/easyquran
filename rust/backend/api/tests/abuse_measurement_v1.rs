// A5 abuse measurement, HTTP and scheduler level:
// - GET /admin/abuse/v1/summary is session admin only (ROLE_ADMIN, same guard
//   as /admin/bans) and answers with the ten figures.
// - A periodic job with a real cadence writes one job_runs row per run.
mod reading_common;

use std::time::Duration;

use axum::http::StatusCode;
use migration::{Migrator, MigratorTrait};
use reading_common::{login_cookie, router_with_state, seed_user, send, state};
use ruxlog::db::sea_models::user::{self, UserRole};
use ruxlog::modules::admin_abuse_v1;
use ruxlog::services::scheduler;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseBackend, Set, Statement};

#[tokio::test]
async fn summary_is_admin_only_and_returns_the_ten_figures() {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");

    let plain = seed_user(&state, "reader").await;
    let plain_cookie = login_cookie(&state, &plain).await;

    let ops = seed_user(&state, "ops").await;
    let mut ops_admin: user::ActiveModel = ops.into();
    ops_admin.role = Set(UserRole::Admin);
    let ops = ops_admin.update(&state.sea_db).await.expect("promote");
    let admin_cookie = login_cookie(&state, &ops).await;

    let app = router_with_state(state.clone(), admin_abuse_v1::routes());

    let (anon, _) = send(&app, "GET", "/admin/abuse/v1/summary", None, None).await;
    assert!(
        anon == StatusCode::UNAUTHORIZED || anon == StatusCode::FORBIDDEN,
        "anonymous must be refused, got {anon}"
    );

    let (non_admin, _) = send(
        &app,
        "GET",
        "/admin/abuse/v1/summary",
        Some(&plain_cookie),
        None,
    )
    .await;
    assert!(
        non_admin == StatusCode::UNAUTHORIZED || non_admin == StatusCode::FORBIDDEN,
        "non-admin must be refused, got {non_admin}"
    );

    let (status, body) = send(
        &app,
        "GET",
        "/admin/abuse/v1/summary",
        Some(&admin_cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "admin body: {body}");
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
        "ingressRejectionsSinceBoot",
    ] {
        assert!(!body[key].is_null(), "summary missing {key}: {body}");
    }
    assert!(body["activeAccountBans"].is_number(), "{body}");
    assert_eq!(body["diskCacheHitRate"], "not measurable");
    assert!(body["ingressRejectionsSinceBoot"].is_object(), "{body}");
}

#[tokio::test]
async fn periodic_scheduler_tick_writes_a_job_runs_row() {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");

    // tokio's interval fires its first tick immediately; one run is enough.
    scheduler::start_scheduler(state.clone());
    tokio::time::sleep(Duration::from_millis(400)).await;

    let row = state
        .sea_db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT job, outcome, detail, expires_at - finished_at, retention_class \
             FROM job_runs WHERE job = 'scheduled_post_publish' ORDER BY id LIMIT 1"
                .to_string(),
        ))
        .await
        .expect("query")
        .expect("one job_runs row after the first tick");
    assert_eq!(
        row.try_get_by_index::<String>(0).unwrap(),
        "scheduled_post_publish"
    );
    assert_eq!(row.try_get_by_index::<String>(1).unwrap(), "ok");
    assert_eq!(
        row.try_get_by_index::<String>(2).unwrap(),
        "due=0 published=0"
    );
    assert_eq!(row.try_get_by_index::<i64>(3).unwrap(), 90 * 86_400);
    assert_eq!(row.try_get_by_index::<String>(4).unwrap(), "job_log_90d");
}
