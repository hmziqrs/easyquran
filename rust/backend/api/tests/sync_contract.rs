//! Old clients talk to new servers for years. These frozen v1 requests must keep
//! passing against every later build; change a fixture only with a new protocol.
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use eq_sync_core::format_stamp;
use reading_common::{harness, send, Harness};
use ruxlog::modules::{bookmark_v1, reading_session_v1};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::{json, Value};

const SESSIONS: &str = include_str!("fixtures/sync-v1/sessions.json");
const BOOKMARKS: &str = include_str!("fixtures/sync-v1/bookmarks.json");
const THREAD: &str = "019a0000-0000-7000-8000-00000000000a";

async fn post(h: &Harness, mut body: Value) -> (StatusCode, Value) {
    body["expectedUserId"] = json!(h.uid_a);
    send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await
}

fn stamp_ago(hours: i64) -> String {
    format_stamp(
        (Utc::now().timestamp_millis() - hours * 3_600_000) as u64,
        0,
        "x",
    )
}

#[tokio::test]
async fn frozen_v1_session_request_still_lands() {
    let h = harness(reading_session_v1::routes()).await;
    let (status, res) = post(&h, serde_json::from_str(SESSIONS).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["applied"], 2);
    assert_eq!(res["results"][0]["id"], THREAD);
    // The frozen stamps age past staleness, so the row reads back from the archive.
    let (_, page) = send(&h.app, "GET", "/archive", Some(&h.cookie_a), None).await;
    assert_eq!(page["sessions"][0]["id"], THREAD);
    assert_eq!(page["sessions"][0]["ratio"], 0.25);
}

#[tokio::test]
async fn frozen_v1_bookmark_request_still_lands() {
    let h = harness(bookmark_v1::routes()).await;
    let (status, res) = post(&h, serde_json::from_str(BOOKMARKS).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["applied"], 2);
    assert_eq!(res["rejected"], 0);
}

#[tokio::test]
async fn a_client_that_omits_a_register_loses_nothing() {
    let h = harness(reading_session_v1::routes()).await;
    post(&h, serde_json::from_str(SESSIONS).unwrap()).await;
    // An older build knows only the lifecycle: the cursor it never sent stays.
    let write = json!({
        "mutations": [{
            "kind": "session.upsert", "id": THREAD,
            "lifecycleStamp": "1790000000001-0000-old", "state": "ended",
            "endedAt": "2026-09-02T00:00:00.000Z", "endReason": "completed",
        }],
        "since": 1,
    });
    let (_, res) = post(&h, write).await;
    let row = &res["sessions"][0];
    assert_eq!(row["state"], "ended");
    assert_eq!(row["ayah"], 5);
    assert_eq!(row["localPage"], 3);
}

#[tokio::test]
async fn an_unknown_row_below_the_floor_is_superseded_and_revives_nothing() {
    let h = harness(reading_session_v1::routes()).await;
    let floor_ms = Utc::now().timestamp_millis();
    h.state
        .sea_db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO sync_counters (user_id, domain, rev, floor_rev, floor_ms) VALUES (?, 'sessions', 40, 30, ?)",
            [h.uid_a.into(), floor_ms.into()],
        ))
        .await
        .unwrap();
    let old = stamp_ago(24);
    let row = json!({
        "kind": "session.upsert", "id": THREAD, "identityStamp": old, "origin": "index",
        "scope": "surah:2", "startedAt": "2026-09-01T00:00:00.000Z", "cursorStamp": old,
        "surah": 2, "ayah": 1, "device": "phone", "lifecycleStamp": old, "state": "open",
    });
    let (_, res) = post(&h, json!({ "mutations": [row], "since": 5 })).await;
    assert_eq!(res["results"][0]["status"], "superseded");
    assert!(res["results"][0].get("stamps").is_none());
    assert_eq!(res["tombstoneFloor"], 30);
    assert_eq!(res["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn a_scrubbed_tombstone_ignores_writes_and_undo_works_before_it() {
    let h = harness(reading_session_v1::routes()).await;
    post(&h, serde_json::from_str(SESSIONS).unwrap()).await;
    let delete = json!({
        "kind": "session.upsert", "id": THREAD, "lifecycleStamp": stamp_ago(2), "state": "deleted",
    });
    post(&h, json!({ "mutations": [delete] })).await;

    // Within the window: nothing to scrub.
    let n = ruxlog::services::sync_sweep::scrub_tombstones(&h.state.sea_db)
        .await
        .unwrap();
    assert_eq!(n, 0);

    let later = Utc::now() + chrono::Duration::hours(1);
    let n = ruxlog::services::sync_sweep::scrub_older_than(&h.state.sea_db, later)
        .await
        .unwrap();
    assert_eq!(n, 1);

    let undo = json!({
        "kind": "session.upsert", "id": THREAD, "lifecycleStamp": stamp_ago(1), "state": "open",
    });
    let (_, res) = post(&h, json!({ "mutations": [undo], "since": 1 })).await;
    assert_eq!(res["results"][0]["status"], "superseded");
    assert_eq!(res["sessions"][0]["state"], "deleted");
    assert!(res["sessions"][0].get("surah").is_none());
}
