// Reading-thread sync over HTTP, per contracts/sync.md: registers merged by stamp,
// per-row results, delta pulls with tombstones, the archive keyset, MAX-merged
// progress, cross-account isolation, and old clients.
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use serde_json::{json, Value};

use eq_sync_core::format_stamp;
use reading_common::{harness, send, Harness};
use ruxlog::modules::reading_session_v1;

const S1: &str = "c1c1c1c1-c1c1-7c1c-8c1c-c1c1c1c1c1c1";
const S2: &str = "c2c2c2c2-c2c2-7c2c-8c2c-c2c2c2c2c2c2";
const S3: &str = "c3c3c3c3-c3c3-7c3c-8c3c-c3c3c3c3c3c3";
const ISO: &str = "2026-11-01T00:00:00.000Z";

async fn setup() -> Harness {
    harness(reading_session_v1::routes()).await
}

fn now_ms() -> u64 {
    Utc::now().timestamp_millis() as u64
}

/// A stamp `mins` minutes ago, from device `dev`.
fn st(mins: i64, dev: &str) -> String {
    format_stamp(now_ms() - (mins * 60_000) as u64, 0, dev)
}

async fn pull(h: &Harness, since: i64, mutations: Vec<Value>) -> Value {
    let body = json!({ "expectedUserId": h.uid_a, "mutations": mutations, "since": since });
    let (status, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    res
}

async fn push(h: &Harness, mutations: Vec<Value>) -> Value {
    pull(h, 0, mutations).await
}

fn thread(id: &str, surah: i32, ayah: i32, stamp: &str) -> Value {
    json!({
        "kind": "session.upsert", "id": id,
        "identityStamp": stamp, "origin": "index", "scope": format!("surah:{surah}"),
        "startedAt": ISO,
        "cursorStamp": stamp, "surah": surah, "ayah": ayah, "device": "phone", "deviceId": "dev-a",
        "lifecycleStamp": stamp, "state": "open",
        "updatedAt": ISO,
    })
}

fn cursor(id: &str, surah: i32, ayah: i32, stamp: &str) -> Value {
    json!({
        "kind": "session.upsert", "id": id, "cursorStamp": stamp, "surah": surah, "ayah": ayah,
        "device": "phone", "updatedAt": ISO,
    })
}

fn lifecycle(id: &str, state: &str, stamp: &str) -> Value {
    let mut row = json!({
        "kind": "session.upsert", "id": id, "lifecycleStamp": stamp, "state": state,
        "updatedAt": ISO,
    });
    if state == "ended" {
        row["endedAt"] = json!(ISO);
        row["endReason"] = json!("completed");
    }
    row
}

fn row<'a>(res: &'a Value, id: &str) -> Option<&'a Value> {
    res["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
}

fn status_of(res: &Value, id: &str) -> String {
    let results = res["results"].as_array().unwrap();
    let found = results
        .iter()
        .find(|r| r["id"] == id)
        .expect("result for row");
    found["status"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn requires_auth_and_matching_account() {
    let h = setup().await;
    let body = json!({ "expectedUserId": h.uid_a, "mutations": [] });
    let (status, _) = send(&h.app, "POST", "/sync", None, Some(&body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let body = json!({ "expectedUserId": h.uid_b });
    let (status, _) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn thread_round_trips_and_the_higher_cursor_stamp_wins() {
    let h = setup().await;
    let res = push(&h, vec![thread(S1, 73, 1, &st(30, "a"))]).await;
    assert_eq!(status_of(&res, S1), "applied");
    assert_eq!(row(&res, S1).unwrap()["ayah"], 1);

    let res = push(&h, vec![cursor(S1, 73, 9, &st(20, "b"))]).await;
    assert_eq!(status_of(&res, S1), "applied");
    assert_eq!(row(&res, S1).unwrap()["ayah"], 9);

    // Older stamp: supersede, and the result carries the row's stamps to adopt.
    let older = st(25, "c");
    let res = push(&h, vec![cursor(S1, 73, 2, &older)]).await;
    assert_eq!(status_of(&res, S1), "superseded");
    assert_eq!(row(&res, S1).unwrap()["ayah"], 9);
    let stamp = res["results"][0]["stamps"]["cursor"].as_str().unwrap();
    assert!(stamp > older.as_str());
}

#[tokio::test]
async fn moving_back_on_purpose_is_allowed() {
    let h = setup().await;
    push(&h, vec![thread(S1, 2, 200, &st(30, "a"))]).await;
    let res = push(&h, vec![cursor(S1, 2, 5, &st(20, "a"))]).await;
    assert_eq!(row(&res, S1).unwrap()["ayah"], 5);
}

#[tokio::test]
async fn identity_is_set_once() {
    let h = setup().await;
    push(&h, vec![thread(S1, 2, 1, &st(30, "a"))]).await;
    let mut later = thread(S1, 2, 1, &st(10, "b"));
    later["scope"] = json!("juz:1");
    let res = push(&h, vec![later]).await;
    assert_eq!(row(&res, S1).unwrap()["scope"], "surah:2");
}

#[tokio::test]
async fn a_device_an_hour_fast_converges_with_the_server() {
    let h = setup().await;
    let ahead = format_stamp(now_ms() + 3_600_000, 0, "fast");
    let res = push(&h, vec![thread(S1, 2, 1, &ahead)]).await;
    let stored = row(&res, S1).unwrap()["cursorStamp"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(stored < ahead, "stamp rewritten below the device's clock");
    assert!(stored.ends_with("-fast"), "device id kept: {stored}");

    // The rewritten stamp comes back in the result, for the client to adopt.
    assert_eq!(res["results"][0]["stamps"]["cursor"], stored.as_str());

    // A later honest edit beats it.
    let honest = format_stamp(now_ms() + 120_000, 0, "slow");
    let res = push(&h, vec![cursor(S1, 2, 7, &honest)]).await;
    assert_eq!(row(&res, S1).unwrap()["ayah"], 7);
}

#[tokio::test]
async fn an_ended_thread_stays_ended_while_a_stale_cursor_lands() {
    let h = setup().await;
    push(&h, vec![thread(S1, 2, 1, &st(30, "a"))]).await;
    push(&h, vec![lifecycle(S1, "ended", &st(20, "a"))]).await;
    let res = pull(&h, 1, vec![cursor(S1, 2, 50, &st(10, "b"))]).await;
    let stored = row(&res, S1).unwrap();
    assert_eq!(stored["state"], "ended");
    assert_eq!(stored["ayah"], 50, "the stale reader's cursor still lands");
}

#[tokio::test]
async fn delete_beats_a_stale_reader_and_undo_restores() {
    let h = setup().await;
    push(&h, vec![thread(S1, 2, 1, &st(30, "a"))]).await;
    push(&h, vec![lifecycle(S1, "deleted", &st(20, "a"))]).await;
    let res = pull(&h, 1, vec![cursor(S1, 2, 9, &st(10, "b"))]).await;
    assert_eq!(row(&res, S1).unwrap()["state"], "deleted");

    let res = pull(&h, 1, vec![lifecycle(S1, "open", &st(5, "a"))]).await;
    let stored = row(&res, S1).unwrap();
    assert_eq!(stored["state"], "open");
    assert_eq!(stored["ayah"], 9);
}

#[tokio::test]
async fn progress_never_regresses() {
    let h = setup().await;
    let prog = |ayah: i32| json!({ "kind": "progress.upsert", "surah": 2, "furthestAyah": ayah });
    let res = push(&h, vec![prog(50)]).await;
    assert_eq!(status_of(&res, "progress:2"), "applied");
    let res = push(&h, vec![prog(7)]).await;
    assert_eq!(status_of(&res, "progress:2"), "superseded");
    assert_eq!(res["progress"][0]["furthestAyah"], 50);
}

#[tokio::test]
async fn one_bad_row_is_refused_alone() {
    let h = setup().await;
    let bad_verse = cursor(S2, 1, 8, &st(1, "a"));
    let bad_stamp = thread(S3, 2, 1, "nope");
    let bad_date = json!({ "kind": "session.upsert", "id": S2, "startedAt": "not a date" });
    let good = thread(S1, 2, 1, &st(10, "a"));
    let res = push(&h, vec![bad_verse, bad_stamp, bad_date, good]).await;
    assert_eq!(res["rejected"], 3);
    assert_eq!(res["applied"], 1);
    assert_eq!(status_of(&res, S3), "rejected");
    assert_eq!(status_of(&res, S1), "applied");
    assert!(row(&res, S1).is_some());
}

#[tokio::test]
async fn a_new_id_without_identity_cannot_create_a_row() {
    let h = setup().await;
    let res = push(&h, vec![cursor(S1, 2, 1, &st(1, "a"))]).await;
    assert_eq!(status_of(&res, S1), "rejected");
}

#[tokio::test]
async fn replay_changes_nothing() {
    let h = setup().await;
    let write = thread(S1, 2, 3, &st(10, "a"));
    let first = push(&h, vec![write.clone()]).await;
    let rev = first["latestRev"].as_i64().unwrap();
    let again = push(&h, vec![write]).await;
    assert_eq!(status_of(&again, S1), "superseded");
    assert_eq!(again["latestRev"].as_i64().unwrap(), rev);
}

#[tokio::test]
async fn delta_pull_delivers_changes_and_tombstones_only() {
    let h = setup().await;
    let first = push(
        &h,
        vec![
            thread(S1, 2, 1, &st(30, "a")),
            thread(S2, 3, 1, &st(30, "a")),
        ],
    )
    .await;
    let since = first["rev"].as_i64().unwrap();

    // Nothing changed: an empty delta, which means nothing for local rows.
    let idle = pull(&h, since, vec![]).await;
    assert_eq!(idle["sessions"].as_array().unwrap().len(), 0);
    assert_eq!(idle["rev"].as_i64().unwrap(), since);

    push(&h, vec![lifecycle(S2, "deleted", &st(10, "a"))]).await;
    let res = pull(&h, since, vec![]).await;
    let rows = res["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let tombstone = &rows[0];
    assert_eq!(tombstone["id"], S2);
    assert_eq!(tombstone["state"], "deleted");
    assert!(
        tombstone.get("surah").is_none(),
        "a tombstone carries no content"
    );
    assert!(tombstone["lifecycleStamp"].is_string());
}

#[tokio::test]
async fn pull_pages_resume_without_loss() {
    let h = setup().await;
    let ids: Vec<String> = (0..450)
        .map(|i| format!("c1c1c1c1-c1c1-7c1c-8c1c-{i:012x}"))
        .collect();
    for chunk in ids.chunks(200) {
        let rows = chunk
            .iter()
            .map(|id| thread(id, 2, 1, &st(5, "a")))
            .collect();
        push(&h, rows).await;
    }
    let mut seen = std::collections::HashSet::new();
    let mut since = 1;
    loop {
        let res = pull(&h, since, vec![]).await;
        for s in res["sessions"].as_array().unwrap() {
            seen.insert(s["id"].as_str().unwrap().to_string());
        }
        since = res["rev"].as_i64().unwrap();
        if res["more"] != true {
            break;
        }
    }
    assert!(seen.len() >= 449, "saw {}", seen.len());
}

#[tokio::test]
async fn a_full_pull_sends_open_threads_only() {
    let h = setup().await;
    push(
        &h,
        vec![
            thread(S1, 2, 1, &st(30, "a")),
            thread(S2, 3, 1, &st(30, "a")),
            thread(S3, 4, 1, &st(30, "a")),
        ],
    )
    .await;
    push(
        &h,
        vec![
            lifecycle(S2, "ended", &st(10, "a")),
            lifecycle(S3, "deleted", &st(10, "a")),
        ],
    )
    .await;
    let res = push(&h, vec![]).await;
    assert!(row(&res, S1).is_some());
    assert!(row(&res, S2).is_none());
    assert!(row(&res, S3).is_none());
    assert_eq!(res["rev"], res["latestRev"]);
}

#[tokio::test]
async fn archive_pages_ended_and_stale_threads_on_the_cursor_stamp() {
    let h = setup().await;
    let stale = 31 * 24 * 60;
    push(
        &h,
        vec![
            thread(S1, 2, 1, &st(60, "a")),
            thread(S2, 3, 1, &st(stale, "a")),
            thread(S3, 4, 1, &st(stale + 1, "a")),
        ],
    )
    .await;
    push(&h, vec![lifecycle(S1, "ended", &st(5, "a"))]).await;

    let (status, page) = send(&h.app, "GET", "/archive?limit=2", Some(&h.cookie_a), None).await;
    assert_eq!(status, StatusCode::OK);
    let got: Vec<&str> = page["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(got, vec![S1, S2]);
    let next = page["next"].as_str().unwrap();
    let uri = format!("/archive?limit=2&before={}", urlencode(next));
    let (_, page) = send(&h.app, "GET", &uri, Some(&h.cookie_a), None).await;
    assert_eq!(page["sessions"][0]["id"], S3);
    assert!(page["next"].is_null());
}

fn urlencode(raw: &str) -> String {
    raw.replace('|', "%7C")
}

#[tokio::test]
async fn crafted_ids_stay_per_account() {
    let h = setup().await;
    push(&h, vec![thread(S1, 2, 1, &st(30, "a"))]).await;
    let body = json!({ "expectedUserId": h.uid_b, "mutations": [thread(S1, 5, 1, &st(1, "b"))], "since": 0 });
    let (_, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_b), Some(&body)).await;
    assert_eq!(row(&res, S1).unwrap()["surah"], 5);
    let mine = push(&h, vec![]).await;
    assert_eq!(row(&mine, S1).unwrap()["surah"], 2);
}

#[tokio::test]
async fn thread_ids_must_be_uuid_v7() {
    let h = setup().await;
    let stamp = st(1, "a");
    let v4 = "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1";
    let res = push(&h, vec![thread(v4, 2, 1, &stamp)]).await;
    assert_eq!(status_of(&res, v4), "rejected", "v4 id is refused");

    let bad_variant = "c1c1c1c1-c1c1-7c1c-cc1c-c1c1c1c1c1c1";
    let res = push(&h, vec![thread(bad_variant, 2, 1, &stamp)]).await;
    assert_eq!(status_of(&res, bad_variant), "rejected");

    // A far-off timestamp is a wrong device clock, never a rejection.
    let odd_clock = "ffffffff-ffff-7c1c-8c1c-c1c1c1c1c1c1";
    let res = push(&h, vec![thread(odd_clock, 2, 1, &stamp)]).await;
    assert_eq!(status_of(&res, odd_clock), "applied");
}

#[tokio::test]
async fn sync_response_carries_server_time_and_epoch() {
    let h = setup().await;
    let before = Utc::now().timestamp_millis();
    let res = push(&h, vec![]).await;
    let server = res["serverTime"].as_i64().expect("serverTime");
    assert!(server >= before && server <= Utc::now().timestamp_millis());
    assert!(res["syncEpoch"].is_string());
    assert_eq!(res["tombstoneFloor"], 0);
}

#[tokio::test]
async fn a_client_below_the_minimum_gets_426_and_applies_nothing() {
    let h = setup().await;
    let body = json!({
        "expectedUserId": h.uid_a,
        "client": { "protocol": 0, "platform": "web", "appVersion": "0" },
        "mutations": [thread(S1, 2, 1, &st(1, "a"))],
    });
    let (status, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::UPGRADE_REQUIRED, "{res}");
    let after = push(&h, vec![]).await;
    assert!(row(&after, S1).is_none());
}
