// Preference sync over HTTP: one register per key, so settings changed apart both
// survive; delta pulls; refused keys; owner mismatch; the daily ceiling.
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use eq_sync_core::format_stamp;
use serde_json::{json, Value};

use reading_common::{harness, send, Harness};
use ruxlog::modules::preference_v1;

fn st(mins: i64, dev: &str) -> String {
    let now = Utc::now().timestamp_millis() as u64;
    format_stamp(now - (mins * 60_000) as u64, 0, dev)
}

async fn round(h: &Harness, since: i64, rows: Vec<Value>) -> Value {
    let body = json!({ "expectedUserId": h.uid_a, "since": since, "mutations": rows });
    let (status, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    res
}

fn pref(key: &str, value: Value, stamp: &str) -> Value {
    json!({ "key": key, "value": value, "stamp": stamp })
}

fn value_of<'a>(res: &'a Value, key: &str) -> Option<&'a Value> {
    let rows = res["preferences"].as_array().unwrap();
    rows.iter().find(|r| r["key"] == key).map(|r| &r["value"])
}

#[tokio::test]
async fn two_settings_changed_on_two_devices_both_survive() {
    let h = harness(preference_v1::routes()).await;
    round(
        &h,
        0,
        vec![pref("settings.theme", json!("dark"), &st(20, "a"))],
    )
    .await;
    round(
        &h,
        0,
        vec![pref("settings.fontSize", json!(36), &st(30, "b"))],
    )
    .await;
    let res = round(&h, 0, vec![]).await;
    assert_eq!(value_of(&res, "settings.theme"), Some(&json!("dark")));
    assert_eq!(value_of(&res, "settings.fontSize"), Some(&json!(36)));
}

#[tokio::test]
async fn the_higher_stamp_wins_and_replay_changes_nothing() {
    let h = harness(preference_v1::routes()).await;
    let newer = pref("home.layout", json!({ "widgets": ["a"] }), &st(5, "a"));
    round(&h, 0, vec![newer.clone()]).await;
    let stale = pref("home.layout", json!({ "widgets": ["b"] }), &st(10, "b"));
    let res = round(&h, 0, vec![stale]).await;
    assert_eq!(res["results"][0]["status"], "superseded");
    assert_eq!(
        value_of(&res, "home.layout"),
        Some(&json!({ "widgets": ["a"] }))
    );
    let rev = res["latestRev"].clone();
    let res = round(&h, 0, vec![newer]).await;
    assert_eq!(res["results"][0]["status"], "superseded");
    assert_eq!(res["latestRev"], rev);
}

#[tokio::test]
async fn a_device_clock_far_ahead_is_rewritten_and_replays_cleanly() {
    let h = harness(preference_v1::routes()).await;
    let ahead = format_stamp(Utc::now().timestamp_millis() as u64 + 3_600_000, 0, "fast");
    let row = pref("settings.theme", json!("dark"), &ahead);
    let res = round(&h, 0, vec![row.clone()]).await;
    let stored = res["results"][0]["stamps"]["row"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(stored < ahead && stored.ends_with("-fast"));
    let again = round(&h, 0, vec![row]).await;
    assert_eq!(again["results"][0]["status"], "superseded");
}

#[tokio::test]
async fn delta_pull_delivers_only_changes() {
    let h = harness(preference_v1::routes()).await;
    let first = round(
        &h,
        0,
        vec![pref("settings.theme", json!("dark"), &st(9, "a"))],
    )
    .await;
    let since = first["rev"].as_i64().unwrap();
    let idle = round(&h, since, vec![]).await;
    assert_eq!(idle["preferences"].as_array().unwrap().len(), 0);
    round(
        &h,
        0,
        vec![pref(
            "reading.bookmark",
            json!({ "surah": 2, "ayah": 5 }),
            &st(5, "b"),
        )],
    )
    .await;
    let res = round(&h, since, vec![]).await;
    assert_eq!(res["preferences"].as_array().unwrap().len(), 1);
    assert_eq!(res["preferences"][0]["key"], "reading.bookmark");
}

#[tokio::test]
async fn clearing_a_key_is_a_newer_null() {
    let h = harness(preference_v1::routes()).await;
    round(
        &h,
        0,
        vec![pref("reading.bookmark", json!({ "surah": 2 }), &st(9, "a"))],
    )
    .await;
    let res = round(
        &h,
        0,
        vec![pref("reading.bookmark", Value::Null, &st(5, "a"))],
    )
    .await;
    assert_eq!(value_of(&res, "reading.bookmark"), Some(&Value::Null));
}

#[tokio::test]
async fn reading_goals_is_one_synced_key_and_nothing_else_under_goals() {
    let h = harness(preference_v1::routes()).await;
    let goals = json!({ "goals": [{ "id": "a", "kind": "finish_quran" }] });
    let res = round(
        &h,
        0,
        vec![
            pref("goals.reading", goals.clone(), &st(5, "a")),
            pref("goals.other", json!(1), &st(5, "a")),
        ],
    )
    .await;
    let statuses: Vec<&str> = res["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, vec!["applied", "rejected"]);
    let pulled = round(&h, 0, vec![]).await;
    assert_eq!(value_of(&pulled, "goals.reading"), Some(&goals));
}

#[tokio::test]
async fn bad_rows_are_refused_alone() {
    let h = harness(preference_v1::routes()).await;
    let huge = "x".repeat(17 * 1024);
    let res = round(
        &h,
        0,
        vec![
            pref("anything.else", json!(1), &st(1, "a")),
            pref("settings.theme", json!("dark"), "not-a-stamp"),
            pref("settings.big", json!(huge), &st(1, "a")),
            pref("settings.ok", json!(1), &st(1, "a")),
        ],
    )
    .await;
    let statuses: Vec<&str> = res["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        vec!["rejected", "rejected", "rejected", "applied"]
    );
}

#[tokio::test]
async fn owner_mismatch_and_accounts_stay_apart() {
    let h = harness(preference_v1::routes()).await;
    let body = json!({ "expectedUserId": h.uid_b });
    let (status, _) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(&h.app, "POST", "/sync", None, Some(&body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    round(
        &h,
        0,
        vec![pref("settings.theme", json!("dark"), &st(5, "a"))],
    )
    .await;
    let body = json!({ "expectedUserId": h.uid_b, "since": 0 });
    let (_, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_b), Some(&body)).await;
    assert_eq!(res["preferences"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn an_old_client_gets_426_and_applies_nothing() {
    let h = harness(preference_v1::routes()).await;
    let body = json!({
        "expectedUserId": h.uid_a, "client": { "protocol": 0 },
        "mutations": [pref("settings.theme", json!("dark"), &st(1, "a"))],
    });
    let (status, _) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::UPGRADE_REQUIRED);
    let res = round(&h, 0, vec![]).await;
    assert_eq!(res["preferences"].as_array().unwrap().len(), 0);
}
