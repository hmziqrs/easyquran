// Daily cross-domain write ceiling and the full-pull snapshot guard (A7,
// contracts/sync.md). The ceiling is one counter per user shared by reading
// session sync, bookmark sync and analytics events. Pulls never charge it.
mod reading_common;

use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, Request, StatusCode};
use chrono::{SecondsFormat, Utc};
use eq_sync_core::format_stamp;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::{json, Value};
use tower::ServiceExt;

use reading_common::{harness, send, Harness};
use ruxlog::modules::{bookmark_v1, reading_session_v1, user_analytics_v1};
use ruxlog::services::write_ceiling::{charge, writes_key, CEILING_EVENT, DAILY_WRITE_CEILING};

const ISO: &str = "2026-11-01T00:00:00.000Z";
const SESSION: &str = "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1";

fn st(mins: i64) -> String {
    let now = Utc::now().timestamp_millis() as u64;
    format_stamp(now - (mins * 60_000) as u64, 0, "a")
}

fn bid(i: u32) -> String {
    format!("c0c0c0c0-c0c0-7c0c-8c0c-{i:012x}")
}

/// Burn `n` slots of today's ceiling for one user, as if writes had been accepted.
async fn spend(h: &Harness, uid: i32, n: u64) {
    for _ in 0..n {
        let c = charge(&h.state.gate_store, uid, Utc::now()).await.unwrap();
        assert!(c.accepted, "fixture spends only within the ceiling");
    }
}

/// POST that also returns the response headers (for Retry-After).
async fn post(
    app: &axum::Router,
    cookie: &str,
    uri: &str,
    body: &Value,
) -> (StatusCode, HeaderMap, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn ledger_rows(h: &Harness, uid: i32) -> i64 {
    let row = h
        .state
        .sea_db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM rate_limit_events WHERE key = ? AND event = ?",
            [writes_key(uid).into(), CEILING_EVENT.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    row.try_get::<i64>("", "n").unwrap()
}

fn mark(i: u32) -> Value {
    json!({
        "kind": "bookmark.upsert", "id": bid(i), "stamp": st(5), "folderId": null,
        "surah": 2, "ayah": (i % 280) as i32 + 1, "deleted": false, "updatedAt": ISO,
    })
}

async fn bookmark_round(
    h: &Harness,
    since: i64,
    mutations: Vec<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let body = json!({ "expectedUserId": h.uid_a, "mutations": mutations, "since": since });
    post(&h.app, &h.cookie_a, "/sync", &body).await
}

#[tokio::test]
async fn five_thousand_and_first_write_is_429_with_retry_after_to_midnight() {
    let h = harness(bookmark_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING).await;

    let (status, headers, body) = bookmark_round(&h, 0, vec![mark(1)]).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["type"], "SRV_004");
    let retry: u64 = headers["retry-after"].to_str().unwrap().parse().unwrap();
    assert!(
        (1..=86_400).contains(&retry),
        "retry-after {retry} is within one day"
    );
    assert_eq!(ledger_rows(&h, h.uid_a).await, 1);
}

#[tokio::test]
async fn refusals_write_exactly_one_ledger_row_per_day() {
    let h = harness(bookmark_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING).await;
    for i in 0..4 {
        let (status, _, _) = bookmark_round(&h, 0, vec![mark(i)]).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }
    assert_eq!(ledger_rows(&h, h.uid_a).await, 1);
    // Another user is untouched by this user's day.
    assert_eq!(ledger_rows(&h, h.uid_b).await, 0);
}

#[tokio::test]
async fn pull_only_rounds_pass_after_the_ceiling() {
    let h = harness(bookmark_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING).await;
    let (status, _, body) = bookmark_round(&h, 0, vec![mark(1)]).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    let (status, _, body) = bookmark_round(&h, 0, vec![]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, body) = bookmark_round(&h, 1, vec![]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn a_round_that_part_fits_caps_the_rest_and_keeps_them_for_later() {
    let h = harness(bookmark_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING - 2).await;
    let (status, _, body) = bookmark_round(&h, 0, vec![mark(1), mark(2), mark(3)]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let statuses: Vec<&str> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["applied", "applied", "capped"]);
    assert_eq!(body["capped"], 1);
    assert_eq!(
        ledger_rows(&h, h.uid_a).await,
        1,
        "the crossing round logs once"
    );
}

#[tokio::test]
async fn rejected_rows_take_no_slot() {
    let h = harness(bookmark_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING - 1).await;
    let bad = json!({ "kind": "bookmark.upsert", "id": "not-a-uuid" });
    let (status, _, body) = bookmark_round(&h, 0, vec![bad, mark(1)]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["rejected"], 1);
    assert_eq!(
        body["applied"], 1,
        "the one free slot went to the valid row"
    );
    assert_eq!(ledger_rows(&h, h.uid_a).await, 0);
}

#[tokio::test]
async fn reading_session_sync_shares_the_same_budget() {
    let h = harness(reading_session_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING).await;
    let progress =
        json!({ "kind": "progress.upsert", "surah": 1, "furthestAyah": 1, "updatedAt": ISO });
    let body = json!({ "expectedUserId": h.uid_a, "mutations": [progress], "since": 0 });
    let (status, _, res) = post(&h.app, &h.cookie_a, "/sync", &body).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{res}");
    assert_eq!(ledger_rows(&h, h.uid_a).await, 1);

    let pull = json!({ "expectedUserId": h.uid_a, "mutations": [], "since": 0 });
    let (status, _, res) = post(&h.app, &h.cookie_a, "/sync", &pull).await;
    assert_eq!(status, StatusCode::OK, "{res}");
}

#[tokio::test]
async fn analytics_batch_is_all_or_nothing_under_the_ceiling() {
    let h = harness(user_analytics_v1::routes()).await;
    spend(&h, h.uid_a, DAILY_WRITE_CEILING - 1).await;
    let event = |n: u32| {
        json!({
            "id": format!("{n:08x}-0000-4000-8000-000000000000"), "sessionId": SESSION,
            "surah": 2, "ayah": 255, "sourceId": null, "ms": 1000, "localDay": "2026-10-14",
            "at": Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true),
        })
    };
    let body = json!({ "expectedUserId": h.uid_a, "events": [event(1), event(2)] });
    let (status, _, res) = post(&h.app, &h.cookie_a, "/events", &body).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{res}");
    assert_eq!(ledger_rows(&h, h.uid_a).await, 1);

    let summary = send(&h.app, "GET", "/summary", Some(&h.cookie_a), None)
        .await
        .1;
    assert_eq!(
        summary["totals"]["ayahs"], 0,
        "a refused batch stores nothing"
    );

    let body = json!({ "expectedUserId": h.uid_a, "events": [event(3)] });
    let (status, _, res) = post(&h.app, &h.cookie_a, "/events", &body).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "the day's last slot is spent: {res}"
    );
}

async fn insert_folders(h: &Harness, from: u32, to: u32) {
    let sql = format!(
        "WITH RECURSIVE n(i) AS (SELECT {from} UNION ALL SELECT i + 1 FROM n WHERE i < {to}) \
         INSERT INTO bookmark_folders (user_id, id, name, is_default, position, created_at, updated_at, rev) \
         SELECT {uid}, printf('f1f1f1f1-f1f1-7f1f-8f1f-%012d', i), 'Seeded', 0, NULL, \
                '2026-11-01T00:00:00+00:00', '2026-11-01T00:00:00+00:00', 0 FROM n",
        uid = h.uid_a,
    );
    h.state.sea_db.execute_unprepared(&sql).await.unwrap();
}

#[tokio::test]
async fn full_pull_at_the_folder_quota_passes_and_one_over_is_507() {
    let h = harness(bookmark_v1::routes()).await;
    // The ensured default folder counts toward the 200-folder quota: 199 + 1 fits.
    insert_folders(&h, 1, 199).await;
    let (status, _, body) = bookmark_round(&h, 0, vec![]).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    insert_folders(&h, 200, 200).await;
    let (status, _, body) = bookmark_round(&h, 0, vec![]).await;
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE, "{body}");
    assert_eq!(body["type"], "SRV_006");
    assert!(
        body.get("folders").is_none(),
        "no truncated snapshot is returned"
    );

    // A delta pull never runs the snapshot guard, so incremental sync keeps going.
    let (status, _, body) = bookmark_round(&h, 1, vec![]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
