// Reading insights over HTTP: auth + account
// check, idempotent ingest keyed by the client event id, both rollups, the
// retention-proof summary (top surahs/ayahs read from rollups, not raw
// events), per-account isolation, and validation.
mod reading_common;

use axum::http::StatusCode;
use chrono::{SecondsFormat, Utc};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::{json, Value};

use reading_common::{harness, send, Harness};
use ruxlog::modules::user_analytics_v1;

const SESSION: &str = "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1";

async fn setup() -> Harness {
    harness(user_analytics_v1::routes()).await
}

fn event_id(n: u32) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}

fn read(n: u32, surah: i32, ayah: i32, ms: i64) -> Value {
    json!({
        "id": event_id(n), "sessionId": SESSION, "surah": surah, "ayah": ayah,
        "sourceId": null, "ms": ms, "localDay": "2026-10-14",
        "at": Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true),
    })
}

async fn post(h: &Harness, cookie: &str, uid: i32, events: Vec<Value>) -> (StatusCode, Value) {
    let body = json!({ "expectedUserId": uid, "events": events });
    send(&h.app, "POST", "/events", Some(cookie), Some(&body)).await
}

async fn summary(h: &Harness, cookie: &str) -> Value {
    let (status, body) = send(&h.app, "GET", "/summary", Some(cookie), None).await;
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
async fn requires_auth_and_matching_account() {
    let h = setup().await;
    let body = json!({ "expectedUserId": h.uid_a, "events": [] });
    let (status, _) = send(&h.app, "POST", "/events", None, Some(&body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post(&h, &h.cookie_a, h.uid_b, vec![]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ingest_is_idempotent_and_rolls_up() {
    let h = setup().await;
    let batch = vec![
        read(1, 2, 255, 4_000),
        read(2, 2, 255, 3_000),
        read(3, 112, 1, 900),
    ];
    let (status, body) = post(&h, &h.cookie_a, h.uid_a, batch.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["accepted"], 3);
    // Retried batch: accepted, never double-counted.
    post(&h, &h.cookie_a, h.uid_a, batch).await;
    let s = summary(&h, &h.cookie_a).await;
    assert_eq!(s["totals"]["ayahs"], 3);
    assert_eq!(s["totals"]["ms"], 7_900);
    assert_eq!(s["totals"]["days"], 1);
    assert_eq!(
        s["topAyahs"][0],
        json!({ "surah": 2, "ayah": 255, "reads": 2, "ms": 7_000 })
    );
    assert_eq!(s["topSurahs"][0]["surah"], 2);
    assert_eq!(s["days"][0]["ayahsRead"], 3);
}

#[tokio::test]
async fn summary_survives_the_raw_event_purge() {
    let h = setup().await;
    post(&h, &h.cookie_a, h.uid_a, vec![read(1, 36, 1, 1_000)]).await;
    h.state
        .sea_db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "DELETE FROM reading_events",
        ))
        .await
        .unwrap();
    let s = summary(&h, &h.cookie_a).await;
    assert_eq!(s["topAyahs"][0]["surah"], 36);
    assert_eq!(s["totals"]["ayahs"], 1);
}

#[tokio::test]
async fn accounts_are_isolated() {
    let h = setup().await;
    post(&h, &h.cookie_a, h.uid_a, vec![read(1, 2, 255, 1_000)]).await;
    // Same event id under B is B's own event.
    post(&h, &h.cookie_b, h.uid_b, vec![read(1, 1, 1, 1_000)]).await;
    let a = summary(&h, &h.cookie_a).await;
    let b = summary(&h, &h.cookie_b).await;
    assert_eq!(a["topAyahs"][0]["surah"], 2);
    assert_eq!(b["topAyahs"][0]["surah"], 1);
}

async fn days(h: &Harness, cookie: &str, query: &str) -> (StatusCode, Value) {
    send(&h.app, "GET", &format!("/days?{query}"), Some(cookie), None).await
}

#[tokio::test]
async fn days_returns_each_local_day_in_range_for_one_account() {
    let h = setup().await;
    let mut later = read(2, 2, 255, 1_000);
    later["localDay"] = json!("2026-10-15");
    let mut outside = read(3, 1, 1, 500);
    outside["localDay"] = json!("2026-11-01");
    post(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![read(1, 2, 255, 4_000), later, outside],
    )
    .await;
    post(&h, &h.cookie_b, h.uid_b, vec![read(1, 1, 1, 9_000)]).await;

    let (status, body) = days(&h, &h.cookie_a, "from=2026-10-14&to=2026-10-15").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["days"],
        json!([
            { "localDay": "2026-10-14", "ayahsRead": 1, "ms": 4_000 },
            { "localDay": "2026-10-15", "ayahsRead": 1, "ms": 1_000 },
        ])
    );
    // B sees only B's read on that day.
    let (_, other) = days(&h, &h.cookie_b, "from=2026-10-14&to=2026-10-14").await;
    assert_eq!(
        other["days"],
        json!([{ "localDay": "2026-10-14", "ayahsRead": 1, "ms": 9_000 }])
    );
}

#[tokio::test]
async fn days_refuses_reversed_bad_and_over_wide_ranges() {
    let h = setup().await;
    let (status, _) = days(&h, &h.cookie_a, "from=2026-10-15&to=2026-10-14").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = days(&h, &h.cookie_a, "from=2026-13-01&to=2026-10-14").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = days(&h, &h.cookie_a, "from=2025-01-01&to=2026-10-14").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = days(&h, &h.cookie_a, "from=2025-10-01&to=2026-10-14").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["days"], json!([]));
}

#[tokio::test]
async fn validation_rejects_the_whole_batch() {
    let h = setup().await;
    let mut bad_id = read(2, 1, 1, 10);
    bad_id["id"] = json!("nope");
    let mut bad_ms = read(3, 1, 1, 10);
    bad_ms["ms"] = json!(3_600_001);
    let mut bad_day = read(4, 1, 1, 10);
    bad_day["localDay"] = json!("2026-13-01");
    for bad in [read(1, 1, 8, 10), bad_id, bad_ms, bad_day] {
        let (status, _) = post(&h, &h.cookie_a, h.uid_a, vec![read(9, 1, 1, 10), bad]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let s = summary(&h, &h.cookie_a).await;
    assert_eq!(
        s["totals"]["ayahs"], 0,
        "a rejected batch must not partially apply"
    );
}
