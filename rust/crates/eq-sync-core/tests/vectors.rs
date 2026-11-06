//! Shared vectors plus seeded property tests. A failing seed reproduces exactly.

use eq_sync_core::{
    canonical_json, compare_stamps, converge_bookmarks, format_stamp, is_valid_stamp,
    merge_bookmark, merge_event, merge_folder, merge_layout, merge_progress, merge_rows,
    merge_settings, merge_thread, round_ratio, Domain,
};
use serde_json::{json, Value};
use std::cmp::Ordering;

const VECTORS: &str = include_str!("../../../../contracts/sync-vectors.json");

fn rows(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

fn assert_same(name: &str, got: &Value, want: &Value) {
    assert_eq!(canonical_json(got), canonical_json(want), "{name}");
}

#[test]
fn shared_vectors_pass() {
    let doc: Value = serde_json::from_str(VECTORS).unwrap();
    for v in doc["vectors"].as_array().unwrap() {
        let name = v["name"].as_str().unwrap();
        let op = v["op"].as_str().unwrap();
        let (a, b) = (&v["a"], &v["b"]);
        match op {
            "thread" => assert_same(name, &merge_thread(a, b), &v["expected"]),
            "progress" => assert_same(name, &merge_progress(a, b), &v["expected"]),
            "bookmark" => assert_same(name, &merge_bookmark(a, b), &v["expected"]),
            "folder" => assert_same(name, &merge_folder(a, b), &v["expected"]),
            "settings" => assert_same(name, &merge_settings(a, b), &v["expected"]),
            // Reading goals are one whole document: the same rule as the home layout.
            "layout" | "goals" => assert_same(name, &merge_layout(a, b), &v["expected"]),
            "event" => assert_same(name, &merge_event(a, b), &v["expected"]),
            "threadSet" => assert_same(
                name,
                &Value::Array(merge_rows(Domain::Thread, &rows(a), &rows(b))),
                &v["expected"],
            ),
            "folderSet" => assert_same(
                name,
                &Value::Array(merge_rows(Domain::Folder, &rows(a), &rows(b))),
                &v["expected"],
            ),
            "converge" => assert_same(
                name,
                &Value::Array(converge_bookmarks(
                    &rows(&v["rows"]),
                    &rows(&v["folders"]),
                    v["defaultFolderId"].as_str().unwrap(),
                )),
                &v["expected"],
            ),
            "stampValid" => assert_eq!(
                is_valid_stamp(v["input"].as_str().unwrap()),
                v["expected"].as_bool().unwrap(),
                "{name}"
            ),
            "stampOrder" => {
                let list = rows(&v["ascending"]);
                for pair in list.windows(2) {
                    assert_eq!(
                        compare_stamps(pair[0].as_str(), pair[1].as_str()),
                        Ordering::Less,
                        "{name}"
                    );
                }
            }
            "roundRatio" => {
                assert_eq!(
                    round_ratio(v["input"].as_f64().unwrap()),
                    v["expected"].as_f64().unwrap(),
                    "{name}"
                );
            }
            "canonical" => assert_eq!(
                canonical_json(&v["input"]),
                v["expected"].as_str().unwrap(),
                "{name}"
            ),
            other => panic!("unknown op {other}"),
        }
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }
    fn stamp(&mut self) -> Value {
        if self.next(8) == 0 {
            return Value::Null;
        }
        // A small range forces plenty of identical stamps.
        Value::String(format_stamp(self.next(4), self.next(2) as u32, "dev"))
    }
    fn register(&mut self, range: u64) -> Value {
        json!({ "stamp": self.stamp(), "value": self.next(range) })
    }
}

fn gen_thread(r: &mut Rng) -> Value {
    let mut row = json!({ "id": "t" });
    for key in ["identity", "cursor", "lifecycle", "future"] {
        if r.next(3) > 0 {
            row[key] = r.register(3);
        }
    }
    row
}

fn gen_folder(r: &mut Rng) -> Value {
    let mut row = json!({ "id": "f", "isDefault": r.next(4) == 0 });
    for key in ["name", "position"] {
        if r.next(3) > 0 {
            row[key] = r.register(3);
        }
    }
    if r.next(2) == 0 {
        row["deleted"] = json!({ "stamp": r.stamp(), "value": r.next(2) == 0 });
    }
    row
}

fn gen_bookmark(r: &mut Rng) -> Value {
    json!({
        "id": "b",
        "register": {
            "stamp": r.stamp(),
            "value": { "context": null, "deleted": r.next(2) == 0, "folderId": "f", "key": "1:1" },
        },
    })
}

fn gen_progress(r: &mut Rng) -> Value {
    json!({ "surah": 2, "furthestAyah": r.next(5) })
}

fn gen_settings(r: &mut Rng) -> Value {
    let mut row = json!({});
    for key in ["theme", "font"] {
        if r.next(2) == 0 {
            row[key] = r.register(3);
        }
    }
    row
}

type Merge = fn(&Value, &Value) -> Value;

fn check_laws(name: &str, merge: Merge, gen: fn(&mut Rng) -> Value) {
    for seed in 1..=4000u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let (a, b, c) = (gen(&mut r), gen(&mut r), gen(&mut r));
        let same = |x: &Value, y: &Value, law: &str| {
            assert_eq!(
                canonical_json(x),
                canonical_json(y),
                "{name} {law} seed {seed}"
            );
        };
        same(&merge(&a, &b), &merge(&b, &a), "commutative");
        same(
            &merge(&merge(&a, &b), &c),
            &merge(&a, &merge(&b, &c)),
            "associative",
        );
        let once = merge(&a, &a);
        same(&merge(&once, &once), &once, "idempotent");
        let ab = merge(&a, &b);
        same(&merge(&ab, &b), &ab, "absorbs a replay");
    }
}

#[test]
fn merges_are_commutative_associative_idempotent() {
    check_laws("thread", merge_thread, gen_thread);
    check_laws("folder", merge_folder, gen_folder);
    check_laws("bookmark", merge_bookmark, gen_bookmark);
    check_laws("progress", merge_progress, gen_progress);
    check_laws("settings", merge_settings, gen_settings);
}

#[test]
fn bookmark_sets_converge_in_any_order_after_exchange() {
    for seed in 1..=1500u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let versions: Vec<Value> = (0..6)
            .map(|_| {
                let id = format!("b{}", r.next(3));
                let key = format!("1:{}", r.next(2));
                json!({
                    "id": id,
                    "register": {
                        "stamp": format_stamp(r.next(6), 0, "d"),
                        "value": { "context": null, "deleted": r.next(4) == 0, "folderId": "f", "key": key },
                    },
                })
            })
            .collect();
        let folders = vec![
            json!({ "id": "f", "deleted": { "stamp": format_stamp(3, 0, "d"), "value": r.next(2) == 0 } }),
        ];
        let step = |left: &[Value], right: &[Value]| {
            converge_bookmarks(&merge_rows(Domain::Bookmark, left, right), &folders, "dflt")
        };
        // Two replicas see the writes in opposite orders, then exchange state.
        let mut x: Vec<Value> = Vec::new();
        let mut y: Vec<Value> = Vec::new();
        for v in &versions {
            x = step(&x, std::slice::from_ref(v));
        }
        for v in versions.iter().rev() {
            y = step(&y, std::slice::from_ref(v));
        }
        let merged_x = step(&x, &y);
        let merged_y = step(&y, &x);
        assert_eq!(
            canonical_json(&Value::Array(merged_x.clone())),
            canonical_json(&Value::Array(merged_y)),
            "seed {seed}"
        );
        // Settled: one more exchange changes nothing, and no key holds two live rows.
        let again = step(&merged_x, &merged_x);
        assert_eq!(
            canonical_json(&Value::Array(again)),
            canonical_json(&Value::Array(merged_x.clone())),
            "seed {seed} settled"
        );
        let mut live: Vec<String> = merged_x
            .iter()
            .filter(|row| row["register"]["value"]["deleted"] != json!(true))
            .map(|row| {
                format!(
                    "{}|{}",
                    row["register"]["value"]["folderId"], row["register"]["value"]["key"]
                )
            })
            .collect();
        let total = live.len();
        live.sort();
        live.dedup();
        assert_eq!(live.len(), total, "seed {seed} rival");
    }
}
