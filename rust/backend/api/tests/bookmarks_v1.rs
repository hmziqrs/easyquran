// Folder-based bookmark sync over HTTP, per contracts/sync.md: the server-ensured
// default folder, multi-folder membership, register merges by stamp, folder delete
// vs bookmark, two ids on one key, per-row results, delta pulls, cross-account
// isolation, validation and quotas.
mod reading_common;

use axum::http::StatusCode;
use chrono::Utc;
use eq_sync_core::format_stamp;
use serde_json::{json, Value};

use reading_common::{harness, send, Harness};
use ruxlog::db::sea_models::bookmark_folder::{default_folder_id, DEFAULT_FOLDER_NAME};
use ruxlog::modules::bookmark_v1;

const ISO: &str = "2026-11-01T00:00:00.000Z";
const HIFZ: &str = "a1a1a1a1-a1a1-7a1a-8a1a-a1a1a1a1a1a1";
const BM1: &str = "b1b1b1b1-b1b1-7b1b-8b1b-b1b1b1b1b1b1";
const BM2: &str = "b2b2b2b2-b2b2-7b2b-8b2b-b2b2b2b2b2b2";

async fn setup() -> Harness {
    harness(bookmark_v1::routes()).await
}

fn st(mins: i64, dev: &str) -> String {
    let now = Utc::now().timestamp_millis() as u64;
    format_stamp(now - (mins * 60_000) as u64, 0, dev)
}

async fn round(h: &Harness, since: i64, mutations: Vec<Value>) -> Value {
    let body = json!({ "expectedUserId": h.uid_a, "mutations": mutations, "since": since });
    let (status, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    res
}

async fn push(h: &Harness, mutations: Vec<Value>) -> Value {
    round(h, 0, mutations).await
}

fn folder(id: &str, name: &str, stamp: &str) -> Value {
    json!({ "kind": "folder.upsert", "id": id, "nameStamp": stamp, "name": name, "updatedAt": ISO })
}

fn folder_position(id: &str, position: i32, stamp: &str) -> Value {
    json!({ "kind": "folder.upsert", "id": id, "positionStamp": stamp, "position": position, "updatedAt": ISO })
}

fn folder_delete(id: &str, stamp: &str) -> Value {
    json!({ "kind": "folder.upsert", "id": id, "deletedStamp": stamp, "deleted": true, "updatedAt": ISO })
}

fn mark(id: &str, folder_id: Option<&str>, surah: i32, ayah: i32, stamp: &str) -> Value {
    json!({
        "kind": "bookmark.upsert", "id": id, "stamp": stamp, "folderId": folder_id,
        "surah": surah, "ayah": ayah, "deleted": false, "updatedAt": ISO,
    })
}

fn unmark(id: &str, folder_id: Option<&str>, surah: i32, ayah: i32, stamp: &str) -> Value {
    let mut row = mark(id, folder_id, surah, ayah, stamp);
    row["deleted"] = json!(true);
    row
}

fn find<'a>(list: &'a Value, id: &str) -> Option<&'a Value> {
    list.as_array().unwrap().iter().find(|v| v["id"] == id)
}

fn status_of(res: &Value, id: &str) -> String {
    let found = find(&res["results"], id).expect("result for row");
    found["status"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn requires_auth_and_matching_account() {
    let h = setup().await;
    let body = json!({ "expectedUserId": h.uid_a });
    let (status, _) = send(&h.app, "POST", "/sync", None, Some(&body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let body = json!({ "expectedUserId": h.uid_b });
    let (status, _) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn default_folder_is_ensured_and_a_rename_beats_the_seed() {
    let h = setup().await;
    let default_id = default_folder_id(h.uid_a);
    let res = push(&h, vec![]).await;
    assert_eq!(res["defaultFolderId"], default_id.as_str());
    let seeded = find(&res["folders"], &default_id).unwrap();
    assert_eq!(seeded["name"], DEFAULT_FOLDER_NAME);
    assert_eq!(seeded["isDefault"], true);

    let res = push(&h, vec![folder(&default_id, "Saved", &st(5, "a"))]).await;
    assert_eq!(find(&res["folders"], &default_id).unwrap()["name"], "Saved");
}

#[tokio::test]
async fn default_folder_cannot_be_deleted() {
    let h = setup().await;
    let default_id = default_folder_id(h.uid_a);
    let res = push(&h, vec![folder_delete(&default_id, &st(5, "a"))]).await;
    assert_eq!(
        find(&res["folders"], &default_id).unwrap()["deleted"],
        false
    );
}

#[tokio::test]
async fn rename_and_move_on_two_devices_both_survive() {
    let h = setup().await;
    push(&h, vec![folder(HIFZ, "Hifz", &st(30, "a"))]).await;
    // Either order: each register merges alone.
    push(
        &h,
        vec![
            folder_position(HIFZ, 9, &st(10, "b")),
            folder(HIFZ, "Memorise", &st(20, "a")),
        ],
    )
    .await;
    let res = push(&h, vec![folder(HIFZ, "Old name", &st(25, "c"))]).await;
    let row = find(&res["folders"], HIFZ).unwrap();
    assert_eq!(row["name"], "Memorise");
    assert_eq!(row["position"], 9);
}

#[tokio::test]
async fn folder_delete_kills_older_bookmarks_and_rehomes_newer_ones() {
    let h = setup().await;
    let default_id = default_folder_id(h.uid_a);
    push(
        &h,
        vec![
            folder(HIFZ, "Hifz", &st(60, "a")),
            mark(BM1, Some(HIFZ), 2, 255, &st(50, "a")),
        ],
    )
    .await;
    // Written later on another device than the delete below.
    let newer = st(5, "b");
    push(&h, vec![mark(BM2, Some(HIFZ), 3, 1, &newer)]).await;

    let res = push(&h, vec![folder_delete(HIFZ, &st(10, "a"))]).await;
    assert!(find(&res["bookmarks"], BM1).is_none(), "older one died");
    assert_eq!(
        find(&res["bookmarks"], BM2).unwrap()["folderId"],
        default_id.as_str()
    );
    assert!(find(&res["folders"], HIFZ).is_none());

    // Arriving after the delete: same rule by stamp.
    let late = "b3b3b3b3-b3b3-7b3b-8b3b-b3b3b3b3b3b3";
    let older = "b4b4b4b4-b4b4-7b4b-8b4b-b4b4b4b4b4b4";
    let res = push(
        &h,
        vec![
            mark(late, Some(HIFZ), 4, 1, &st(1, "b")),
            mark(older, Some(HIFZ), 5, 1, &st(30, "b")),
        ],
    )
    .await;
    assert_eq!(
        find(&res["bookmarks"], late).unwrap()["folderId"],
        default_id.as_str()
    );
    assert!(find(&res["bookmarks"], older).is_none());
}

#[tokio::test]
async fn two_ids_on_one_key_converge_to_one() {
    let h = setup().await;
    let res = push(
        &h,
        vec![
            folder(HIFZ, "Hifz", &st(60, "a")),
            mark(BM1, Some(HIFZ), 2, 255, &st(30, "a")),
            mark(BM2, Some(HIFZ), 2, 255, &st(20, "b")),
        ],
    )
    .await;
    assert!(find(&res["bookmarks"], BM1).is_none());
    assert!(find(&res["bookmarks"], BM2).is_some());

    // Arriving in the other order lands the same way.
    let h2 = setup().await;
    let res = push(
        &h2,
        vec![
            folder(HIFZ, "Hifz", &st(60, "a")),
            mark(BM2, Some(HIFZ), 2, 255, &st(20, "b")),
            mark(BM1, Some(HIFZ), 2, 255, &st(30, "a")),
        ],
    )
    .await;
    assert!(find(&res["bookmarks"], BM1).is_none());
    assert!(find(&res["bookmarks"], BM2).is_some());
}

#[tokio::test]
async fn one_verse_may_sit_in_many_folders() {
    let h = setup().await;
    let res = push(
        &h,
        vec![
            folder(HIFZ, "Hifz", &st(60, "a")),
            mark(BM1, Some(HIFZ), 2, 255, &st(30, "a")),
            mark(BM2, None, 2, 255, &st(30, "a")),
        ],
    )
    .await;
    assert!(find(&res["bookmarks"], BM1).is_some());
    assert!(find(&res["bookmarks"], BM2).is_some());
}

#[tokio::test]
async fn stale_writes_lose_undo_with_a_newer_stamp_restores() {
    let h = setup().await;
    push(&h, vec![mark(BM1, None, 2, 255, &st(30, "a"))]).await;
    push(&h, vec![unmark(BM1, None, 2, 255, &st(20, "a"))]).await;
    let res = push(&h, vec![mark(BM1, None, 2, 255, &st(25, "b"))]).await;
    assert_eq!(status_of(&res, BM1), "superseded");
    assert!(find(&res["bookmarks"], BM1).is_none());
    let res = push(&h, vec![mark(BM1, None, 2, 255, &st(5, "a"))]).await;
    assert!(find(&res["bookmarks"], BM1).is_some());
}

#[tokio::test]
async fn delta_pull_delivers_tombstones_with_their_key() {
    let h = setup().await;
    let first = push(&h, vec![mark(BM1, None, 2, 255, &st(30, "a"))]).await;
    let since = first["rev"].as_i64().unwrap();
    let idle = round(&h, since, vec![]).await;
    assert_eq!(idle["bookmarks"].as_array().unwrap().len(), 0);
    push(&h, vec![unmark(BM1, None, 2, 255, &st(10, "a"))]).await;
    let res = round(&h, since, vec![]).await;
    let row = find(&res["bookmarks"], BM1).unwrap();
    assert_eq!(row["deleted"], true);
    assert_eq!(row["surah"], 2);
    assert!(row.get("sourceId").is_none());
}

#[tokio::test]
async fn one_bad_row_is_refused_alone() {
    let h = setup().await;
    let res = push(
        &h,
        vec![
            mark(BM1, None, 1, 8, &st(5, "a")),
            mark("not-a-uuid", None, 2, 1, &st(5, "a")),
            mark(BM2, None, 2, 255, &st(5, "a")),
        ],
    )
    .await;
    assert_eq!(status_of(&res, BM1), "rejected");
    assert_eq!(status_of(&res, BM2), "applied");
    assert_eq!(res["rejected"], 2);
}

#[tokio::test]
async fn range_rules_hold_per_row() {
    let h = setup().await;
    let span = json!({
        "kind": "bookmark.upsert", "id": BM1, "stamp": st(5, "a"),
        "surah": 2, "ayah": 1, "endSurah": 2, "endAyah": 100,
    });
    let back = json!({
        "kind": "bookmark.upsert", "id": BM2, "stamp": st(5, "a"),
        "surah": 2, "ayah": 5, "endSurah": 2, "endAyah": 1,
    });
    let res = push(&h, vec![span, back]).await;
    assert_eq!(res["rejected"], 2);
}

#[tokio::test]
async fn folder_cap_refuses_a_new_id_and_allows_a_rename() {
    let h = setup().await;
    // The ensured default folder counts: 199 more reach the cap of 200.
    let stamp = st(30, "a");
    let mut first = None;
    for chunk in (0..199).collect::<Vec<i32>>().chunks(100) {
        let rows: Vec<Value> = chunk
            .iter()
            .map(|i| {
                let id = format!("a1a1a1a1-a1a1-7a1a-8a1a-{i:012x}");
                first.get_or_insert_with(|| id.clone());
                folder(&id, "F", &stamp)
            })
            .collect();
        push(&h, rows).await;
    }
    let extra = "a2a2a2a2-a2a2-7a2a-8a2a-a2a2a2a2a2a2";
    let res = push(&h, vec![folder(extra, "Too many", &stamp)]).await;
    assert_eq!(status_of(&res, extra), "capped");
    assert_eq!(res["capped"], 1);

    let rename = first.unwrap();
    let res = push(&h, vec![folder(&rename, "Renamed", &st(5, "a"))]).await;
    assert_eq!(status_of(&res, &rename), "applied");
}

#[tokio::test]
async fn crafted_ids_stay_per_account() {
    let h = setup().await;
    push(&h, vec![mark(BM1, None, 2, 255, &st(30, "a"))]).await;
    let body =
        json!({ "expectedUserId": h.uid_b, "mutations": [mark(BM1, None, 3, 1, &st(1, "b"))] });
    let (_, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_b), Some(&body)).await;
    assert_eq!(find(&res["bookmarks"], BM1).unwrap()["surah"], 3);
    let mine = push(&h, vec![]).await;
    assert_eq!(find(&mine["bookmarks"], BM1).unwrap()["surah"], 2);
}

#[tokio::test]
async fn a_client_below_the_minimum_gets_426() {
    let h = setup().await;
    let body = json!({ "expectedUserId": h.uid_a, "client": { "protocol": 0 } });
    let (status, _) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::UPGRADE_REQUIRED);
}
