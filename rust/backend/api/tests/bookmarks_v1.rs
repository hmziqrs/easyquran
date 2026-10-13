// Folder-based bookmark sync over HTTP (docs/reading-model.md §3): the
// server-ensured default folder, multi-folder membership, LWW upserts and
// tombstone deletes, folder delete cascading to its bookmarks (with newer
// rows re-homed to the default folder), the default-folder delete guard,
// cross-account isolation, and validation.
mod reading_common;

use axum::http::StatusCode;
use serde_json::{json, Value};

use reading_common::{harness, send, Harness};
use ruxlog::db::sea_models::bookmark_folder::{default_folder_id, DEFAULT_FOLDER_NAME};
use ruxlog::modules::bookmark_v1;

const T0: &str = "2026-08-26T10:00:00.000000Z";
const T1: &str = "2026-08-27T10:00:00.000000Z";
const T2: &str = "2026-08-28T10:00:00.000000Z";
const T3: &str = "2026-08-29T10:00:00.000000Z";

const HIFZ: &str = "a1a1a1a1-a1a1-4a1a-8a1a-a1a1a1a1a1a1";
const BM1: &str = "b1b1b1b1-b1b1-4b1b-8b1b-b1b1b1b1b1b1";
const BM2: &str = "b2b2b2b2-b2b2-4b2b-8b2b-b2b2b2b2b2b2";
const BM3: &str = "b3b3b3b3-b3b3-4b3b-8b3b-b3b3b3b3b3b3";
const BM4: &str = "b4b4b4b4-b4b4-4b4b-8b4b-b4b4b4b4b4b4";

async fn setup() -> Harness {
    harness(bookmark_v1::routes()).await
}

async fn sync(h: &Harness, cookie: &str, uid: i32, mutations: Vec<Value>) -> (StatusCode, Value) {
    let body = json!({ "expectedUserId": uid, "mutations": mutations });
    send(&h.app, "POST", "/sync", Some(cookie), Some(&body)).await
}

fn folder(id: &str, name: &str, ts: &str) -> Value {
    json!({ "kind": "folder.upsert", "id": id, "name": name, "updatedAt": ts })
}

fn mark(id: &str, folder_id: Option<&str>, surah: i32, ayah: i32, ts: &str) -> Value {
    json!({
        "kind": "bookmark.upsert", "id": id, "folderId": folder_id,
        "surah": surah, "ayah": ayah, "updatedAt": ts,
    })
}

fn del(kind: &str, id: &str, ts: &str) -> Value {
    json!({ "kind": kind, "id": id, "updatedAt": ts })
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect()
}

fn folder_of<'a>(snap: &'a Value, bookmark_id: &str) -> &'a str {
    snap["bookmarks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == bookmark_id)
        .and_then(|b| b["folderId"].as_str())
        .expect("bookmark present")
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
async fn first_round_seeds_the_default_folder() {
    let h = setup().await;
    let (status, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    let folders = snap["folders"].as_array().unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0]["id"], default_folder_id(h.uid_a));
    assert_eq!(folders[0]["name"], DEFAULT_FOLDER_NAME);
    assert_eq!(folders[0]["isDefault"], true);
    // Deterministic per account, distinct across accounts.
    assert_ne!(default_folder_id(h.uid_a), default_folder_id(h.uid_b));
}

#[tokio::test]
async fn bookmark_without_folder_lands_in_default_and_renaming_default_works() {
    let h = setup().await;
    let default = default_folder_id(h.uid_a);
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![
            mark(BM1, None, 2, 255, T0),
            // Unknown folder ids also land in the default.
            mark(BM2, Some(HIFZ), 1, 1, T0),
            folder(&default, "Saved", T1),
        ],
    )
    .await;
    assert_eq!(folder_of(&snap, BM1), default);
    assert_eq!(folder_of(&snap, BM2), default);
    assert_eq!(snap["folders"][0]["name"], "Saved");
    assert_eq!(snap["folders"][0]["isDefault"], true);
}

#[tokio::test]
async fn a_verse_can_live_in_several_folders() {
    let h = setup().await;
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![
            folder(HIFZ, "Hifz", T0),
            mark(BM1, None, 2, 255, T0),
            mark(BM2, Some(HIFZ), 2, 255, T0),
        ],
    )
    .await;
    assert_eq!(snap["bookmarks"].as_array().unwrap().len(), 2);
    assert_eq!(folder_of(&snap, BM2), HIFZ);
}

#[tokio::test]
async fn same_key_twice_collapses_to_the_newest_id() {
    let h = setup().await;
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![mark(BM1, None, 2, 255, T0), mark(BM2, None, 2, 255, T1)],
    )
    .await;
    assert_eq!(ids(&snap["bookmarks"]), vec![BM2.to_string()]);
    // A stale replay of the loser cannot take the key back.
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![mark(BM1, None, 2, 255, T0)]).await;
    assert_eq!(ids(&snap["bookmarks"]), vec![BM2.to_string()]);
}

#[tokio::test]
async fn delete_is_a_tombstone_that_beats_stale_upserts() {
    let h = setup().await;
    sync(&h, &h.cookie_a, h.uid_a, vec![mark(BM1, None, 2, 255, T0)]).await;
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![del("bookmark.delete", BM1, T1)],
    )
    .await;
    assert!(snap["bookmarks"].as_array().unwrap().is_empty());
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![mark(BM1, None, 2, 255, T0)]).await;
    assert!(
        snap["bookmarks"].as_array().unwrap().is_empty(),
        "stale upsert resurrected"
    );
    // A newer upsert of the same id restores it (undo).
    let (_, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![mark(BM1, None, 2, 255, T2)]).await;
    assert_eq!(ids(&snap["bookmarks"]), vec![BM1.to_string()]);
}

#[tokio::test]
async fn folder_delete_cascades_and_rehomes_newer_rows() {
    let h = setup().await;
    let default = default_folder_id(h.uid_a);
    sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![
            folder(HIFZ, "Hifz", T0),
            mark(BM1, Some(HIFZ), 2, 255, T0),
            // Written after the delete instant below (another device's newer intent).
            mark(BM2, Some(HIFZ), 18, 10, T3),
            mark(BM3, None, 36, 1, T0),
            // Newer too, but its key is already live in the default folder
            // (BM3): it is tombstoned instead of re-homed.
            mark(BM4, Some(HIFZ), 36, 1, T3),
        ],
    )
    .await;
    let (status, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![del("folder.delete", HIFZ, T2)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&snap["folders"]), vec![default.clone()]);
    let mut live = ids(&snap["bookmarks"]);
    live.sort();
    assert_eq!(live, vec![BM2.to_string(), BM3.to_string()]);
    assert_eq!(folder_of(&snap, BM2), default);
    // Undo: a newer folder upsert + bookmark upsert bring it all back.
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![folder(HIFZ, "Hifz", T3), mark(BM1, Some(HIFZ), 2, 255, T3)],
    )
    .await;
    assert_eq!(snap["folders"].as_array().unwrap().len(), 2);
    assert_eq!(folder_of(&snap, BM1), HIFZ);
}

#[tokio::test]
async fn default_folder_cannot_be_deleted() {
    let h = setup().await;
    let default = default_folder_id(h.uid_a);
    let (_, snap) = sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![
            mark(BM1, None, 2, 255, T0),
            del("folder.delete", &default, T1),
        ],
    )
    .await;
    assert_eq!(snap["skipped"], 1);
    assert_eq!(ids(&snap["folders"]), vec![default]);
    assert_eq!(ids(&snap["bookmarks"]), vec![BM1.to_string()]);
}

#[tokio::test]
async fn accounts_are_isolated_even_with_crafted_ids() {
    let h = setup().await;
    sync(
        &h,
        &h.cookie_a,
        h.uid_a,
        vec![folder(HIFZ, "Hifz", T0), mark(BM1, Some(HIFZ), 2, 255, T0)],
    )
    .await;
    // B reuses A's ids: B gets its own rows, A's stay untouched.
    let (_, snap_b) = sync(
        &h,
        &h.cookie_b,
        h.uid_b,
        vec![
            mark(BM1, Some(HIFZ), 1, 1, T1),
            del("folder.delete", HIFZ, T2),
        ],
    )
    .await;
    assert_eq!(folder_of(&snap_b, BM1), default_folder_id(h.uid_b));
    let (_, snap_a) = sync(&h, &h.cookie_a, h.uid_a, vec![]).await;
    assert_eq!(snap_a["folders"].as_array().unwrap().len(), 2);
    assert_eq!(folder_of(&snap_a, BM1), HIFZ);
}

#[tokio::test]
async fn ranges_validate_against_the_quran_meta() {
    let h = setup().await;
    let range = |end_surah: i32, end_ayah: i32| {
        json!({
            "kind": "bookmark.upsert", "id": BM1, "surah": 2, "ayah": 285,
            "endSurah": end_surah, "endAyah": end_ayah, "updatedAt": T0,
        })
    };
    let (status, snap) = sync(&h, &h.cookie_a, h.uid_a, vec![range(2, 286)]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snap["bookmarks"][0]["endAyah"], 286);
    let (status, _) = sync(&h, &h.cookie_a, h.uid_a, vec![range(2, 284)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = sync(&h, &h.cookie_a, h.uid_a, vec![mark(BM2, None, 1, 8, T0)]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let bad_id =
        json!({ "kind": "bookmark.upsert", "id": "nope", "surah": 1, "ayah": 1, "updatedAt": T0 });
    let (status, _) = sync(&h, &h.cookie_a, h.uid_a, vec![bad_id]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
