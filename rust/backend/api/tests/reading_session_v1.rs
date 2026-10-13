// Reading-thread sync over HTTP (docs/reading-model.md §2): whole-row LWW
// threads with the cursor inline, ended threads leaving the open snapshot,
// stale threads derived from updated_at, the archive keyset pagination,
// MAX-merged account progress, cross-account isolation, the clock clamp, and
// validation of scopes and verses.
mod reading_common;

use axum::http::StatusCode;
use chrono::{Duration, SecondsFormat, Utc};
use serde_json::{json, Value};

use reading_common::{harness, send, Harness};
use ruxlog::modules::reading_session_v1;

const S1: &str = "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1";
const S2: &str = "c2c2c2c2-c2c2-4c2c-8c2c-c2c2c2c2c2c2";
const S3: &str = "c3c3c3c3-c3c3-4c3c-8c3c-c3c3c3c3c3c3";

async fn setup() -> Harness {
    harness(reading_session_v1::routes()).await
}

/// RFC3339 `days` ago (negative = future) — open/stale depend on real now.
fn ago(days: i64) -> String {
    (Utc::now() - Duration::days(days)).to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn mins_ago(mins: i64) -> String {
    (Utc::now() - Duration::minutes(mins)).to_rfc3339_opts(SecondsFormat::Micros, true)
}

async fn sync(h: &Harness, cookie: &str, uid: i32, mutations: Vec<Value>) -> (StatusCode, Value) {
    let body = json!({ "expectedUserId": uid, "mutations": mutations });
    send(&h.app, "POST", "/sync", Some(cookie), Some(&body)).await
}

fn thread(id: &str, surah: i32, ayah: i32, ts: &str) -> Value {
    json!({
        "kind": "session.upsert", "id": id, "origin": "index",
        "scope": format!("surah:{surah}"), "surah": surah, "ayah": ayah,
        "device": "phone", "startedAt": ts, "updatedAt": ts,
    })
}

fn with(mut v: Value, extra: Value) -> Value {
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    v
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn requires_auth_and_matching_account() {
    let h = setup().await;
    let body = json!({ "expectedUserId": h.uid_a, "mutations": [] });
    let (status, _) = send(&h.app, "POST", "/sync", None, Some(&body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = sync(&h, &h.cookie_a, h.uid_b, vec![]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn thread_round_trips_and_cursor_moves_lww() {
    let h = setup().await;
    let t0 = mins_ago(30);
    let t1 = mins_ago(20);
    let (status, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![thread(S1, 73, 1, &t0)]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snap["sessions"][0]["origin"], "index");
    assert_eq!(snap["sessions"][0]["scope"], "surah:73");
    assert_eq!(snap["sessions"][0]["device"], "phone");
    // Cursor advances on another device (handoff): newer wins.
    let moved = with(
        thread(S1, 73, 12, &t1),
        json!({ "device": "desktop", "startedAt": t0 }),
    );
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![moved]).await;
    assert_eq!(snap["sessions"][0]["ayah"], 12);
    assert_eq!(snap["sessions"][0]["device"], "desktop");
    // A stale replay cannot drag it back.
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![thread(S1, 73, 3, &t0)]).await;
    assert_eq!(snap["sessions"][0]["ayah"], 12);
    assert_eq!(snap["skipped"], 1);
}

#[tokio::test]
async fn open_snapshot_is_newest_first_and_excludes_ended_and_stale() {
    let h = setup().await;
    let ended = with(
        thread(S2, 112, 4, &mins_ago(5)),
        json!({ "endedAt": mins_ago(5), "endReason": "completed",
                "scope": "snippet:three-quls", "origin": "snippet", "boundEnd": "114:6" }),
    );
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![
            thread(S1, 73, 12, &mins_ago(10)),
            ended,
            thread(S3, 18, 1, &ago(31)),
        ],
    )
    .await;
    assert_eq!(ids(&snap["sessions"]), vec![S1.to_string()]);
}

#[tokio::test]
async fn archive_pages_ended_and_stale_threads() {
    let h = setup().await;
    let end = |id: &str, ts: &str, reason: &str| {
        with(
            thread(id, 1, 7, ts),
            json!({ "endedAt": ts, "endReason": reason }),
        )
    };
    sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![
            end(S1, &mins_ago(1), "restarted"),
            end(S2, &mins_ago(2), "dismissed"),
            thread(S3, 18, 1, &ago(40)),
        ],
    )
    .await;
    let (status, page1) = send(&h.app, "GET", "/archive?limit=2", Some(&h.cookie_a), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ids(&page1["sessions"]),
        vec![S1.to_string(), S2.to_string()]
    );
    let next = page1["next"].as_str().expect("more pages").to_string();
    let uri = format!("/archive?limit=2&before={}", urlencoding(&next));
    let (_, page2) = send(&h.app, "GET", &uri, Some(&h.cookie_a), None).await;
    assert_eq!(ids(&page2["sessions"]), vec![S3.to_string()]);
    assert!(page2["next"].is_null());
    // Reopening from history: a newer upsert clears ended_at.
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![thread(S1, 1, 7, &mins_ago(0))],
    )
    .await;
    assert_eq!(ids(&snap["sessions"]), vec![S1.to_string()]);
}

fn urlencoding(s: &str) -> String {
    s.replace(':', "%3A")
        .replace('|', "%7C")
        .replace('+', "%2B")
}

#[tokio::test]
async fn delete_hides_from_everything() {
    let h = setup().await;
    sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![thread(S1, 2, 1, &mins_ago(10))],
    )
    .await;
    let del = json!({ "kind": "session.delete", "id": S1, "updatedAt": mins_ago(5) });
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![del]).await;
    assert!(snap["sessions"].as_array().unwrap().is_empty());
    let (_, archive) = send(&h.app, "GET", "/archive", Some(&h.cookie_a), None).await;
    assert!(archive["sessions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn progress_merges_with_max() {
    let h = setup().await;
    let progress = |ayah: i32| json!({ "kind": "progress.upsert", "surah": 2, "furthestAyah": ayah, "updatedAt": mins_ago(1) });
    sync(&h, &h.cookie_a, h.uid_a, vec![progress(100)]).await;
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![progress(40)]).await;
    assert_eq!(snap["progress"][0]["furthestAyah"], 100);
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![progress(150)]).await;
    assert_eq!(snap["progress"][0]["furthestAyah"], 150);
}

#[tokio::test]
async fn crafted_ids_stay_per_account() {
    let h = setup().await;
    sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![thread(S1, 73, 12, &mins_ago(10))],
    )
    .await;
    let (_, snap_b) = sync(
        &h,
        &h.cookie_b,
        h.uid_b,
        vec![thread(S1, 1, 1, &mins_ago(1))],
    )
    .await;
    assert_eq!(snap_b["sessions"][0]["surah"], 1);
    let (_, snap_a) = sync(&h, &h.cookie_a, h.uid_a, vec![]).await;
    assert_eq!(snap_a["sessions"][0]["surah"], 73);
}

#[tokio::test]
async fn future_clocks_are_clamped() {
    let h = setup().await;
    sync(&h, &h.cookie_a, h.uid_a, vec![thread(S1, 2, 1, &ago(-365))]).await;
    // A normal write a few seconds later still wins over the clamped future.
    let later = (Utc::now() + Duration::seconds(120)).to_rfc3339_opts(SecondsFormat::Micros, true);
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![thread(S1, 2, 9, &later)]).await;
    assert_eq!(snap["sessions"][0]["ayah"], 9);
}

#[tokio::test]
async fn validation_rejects_bad_shapes() {
    let h = setup().await;
    let now = mins_ago(0);
    for bad in [
        with(thread(S1, 1, 8, &now), json!({})),
        with(thread(S1, 1, 1, &now), json!({ "scope": "chapter:1" })),
        with(
            thread(S1, 1, 1, &now),
            json!({ "scope": "range:2:285-2:300" }),
        ),
        with(thread(S1, 1, 1, &now), json!({ "origin": "telepathy" })),
        with(thread(S1, 1, 1, &now), json!({ "device": "watch" })),
        with(thread(S1, 1, 1, &now), json!({ "endReason": "completed" })),
        with(thread(S1, 1, 1, &now), json!({ "boundEnd": "1:99" })),
        json!({ "kind": "session.upsert", "id": S1, "updatedAt": now }),
    ] {
        let (status, _) = sync(&h, &h.cookie_a, h.uid_a, vec![bad.clone()]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "accepted {bad}");
    }
    let ok = with(
        thread(S1, 2, 285, &now),
        json!({ "scope": "range:2:285-2:286", "boundEnd": "2:286", "origin": "card",
                "originRef": "card-1", "sourceId": "en.sahih", "localPage": 3, "ratio": 0.5 }),
    );
    let (status, _) = sync(&h, &h.cookie_a, h.uid_a, vec![ok]).await;
    assert_eq!(status, StatusCode::OK);
}
