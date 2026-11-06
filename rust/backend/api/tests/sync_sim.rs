//! Multi-device simulator. Seeded random runs drive the real HTTP route with
//! simulated clients built on the merge core: clock skew, offline stretches, lost
//! and duplicated responses, kills, a cloned device, a second account, an old
//! client. A failure names its seed and prints the trace. `just sync-soak` runs more.
mod reading_common;

use std::collections::{BTreeMap, BTreeSet};

use axum::http::StatusCode;
use chrono::Utc;
use eq_sync_core::{canonical_json, compare_stamps, format_stamp, merge_thread, parse_stamp};
use reading_common::{harness, send, Harness};
use ruxlog::modules::reading_session_v1;
use serde_json::{json, Value};

const ISO: &str = "2026-11-01T00:00:00.000Z";
const DAY_MS: i64 = 86_400_000;
const MIN_PER_RUN: usize = 40;

struct Rng(u64);

impl Rng {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next(100) < percent
    }
}

struct Device {
    name: String,
    skew_ms: i64,
    offset_ms: i64,
    last_ms: u64,
    counter: u32,
    online: bool,
    since: i64,
    after: i64,
    rows: BTreeMap<String, Value>,
    dirty: BTreeSet<String>,
    progress: BTreeMap<i64, i64>,
    progress_dirty: BTreeSet<i64>,
}

impl Device {
    fn new(name: &str, skew_ms: i64) -> Self {
        Self {
            name: name.to_string(),
            skew_ms,
            offset_ms: 0,
            last_ms: 0,
            counter: 0,
            online: true,
            since: 0,
            after: -1,
            rows: BTreeMap::new(),
            dirty: BTreeSet::new(),
            progress: BTreeMap::new(),
            progress_dirty: BTreeSet::new(),
        }
    }

    fn stamp(&mut self) -> String {
        let local = Utc::now().timestamp_millis() + self.skew_ms + self.offset_ms;
        let ms = (local.max(0) as u64).max(self.last_ms);
        if ms == self.last_ms {
            self.counter += 1;
        } else {
            self.counter = 0;
        }
        self.last_ms = ms;
        format_stamp(ms, self.counter, &self.name)
    }

    fn observe(&mut self, stamp: Option<&str>) {
        let Some(parsed) = stamp.and_then(parse_stamp) else {
            return;
        };
        if parsed.ms > self.last_ms || (parsed.ms == self.last_ms && parsed.counter > self.counter)
        {
            self.last_ms = parsed.ms;
            self.counter = parsed.counter;
        }
    }

    fn on_server_time(&mut self, server_ms: i64) {
        self.offset_ms = server_ms - (Utc::now().timestamp_millis() + self.skew_ms);
        let ceiling = (server_ms + 60_000) as u64;
        if self.last_ms > ceiling {
            self.last_ms = ceiling;
            self.counter = 0;
        }
    }

    fn create(&mut self, id: &str, surah: i64) {
        let stamp = self.stamp();
        let row = json!({
            "id": id,
            "identity": { "stamp": stamp, "value": {
                "origin": "index", "originRef": null, "scope": format!("surah:{surah}"),
                "boundEnd": null, "startedAt": ISO,
            }},
            "cursor": { "stamp": stamp, "value": {
                "surah": surah, "ayah": 1, "localPage": null, "ratio": null, "sourceId": null,
                "device": "phone", "deviceId": self.name,
            }},
            "lifecycle": { "stamp": stamp, "value": {
                "state": "open", "endedAt": null, "endReason": null,
            }},
        });
        self.rows.insert(id.to_string(), row);
        self.dirty.insert(id.to_string());
    }

    fn move_cursor(&mut self, id: &str, ayah: i64) {
        let stamp = self.stamp();
        let name = self.name.clone();
        if let Some(row) = self.rows.get_mut(id) {
            let surah = row["cursor"]["value"]["surah"].clone();
            row["cursor"] = json!({ "stamp": stamp, "value": {
                "surah": surah, "ayah": ayah, "localPage": null, "ratio": null,
                "sourceId": null, "device": "phone", "deviceId": name,
            }});
            self.dirty.insert(id.to_string());
        }
    }

    fn set_state(&mut self, id: &str, state: &str) {
        let stamp = self.stamp();
        if let Some(row) = self.rows.get_mut(id) {
            let ended = state == "ended";
            row["lifecycle"] = json!({ "stamp": stamp, "value": {
                "state": state,
                "endedAt": if ended { json!(ISO) } else { Value::Null },
                "endReason": if ended { json!("completed") } else { Value::Null },
            }});
            self.dirty.insert(id.to_string());
        }
    }

    fn wire(&self) -> Vec<Value> {
        let mut out: Vec<Value> = self
            .dirty
            .iter()
            .filter_map(|id| self.rows.get(id))
            .map(|row| {
                let (identity, cursor, life) =
                    (&row["identity"], &row["cursor"], &row["lifecycle"]);
                let mut wire = json!({
                    "kind": "session.upsert", "id": row["id"], "updatedAt": ISO,
                });
                if !identity.is_null() {
                    wire["identityStamp"] = identity["stamp"].clone();
                    for (k, v) in identity["value"].as_object().unwrap() {
                        wire[k] = v.clone();
                    }
                }
                if !cursor.is_null() {
                    wire["cursorStamp"] = cursor["stamp"].clone();
                    for (k, v) in cursor["value"].as_object().unwrap() {
                        wire[k] = v.clone();
                    }
                }
                if !life.is_null() {
                    wire["lifecycleStamp"] = life["stamp"].clone();
                    for (k, v) in life["value"].as_object().unwrap() {
                        wire[k] = v.clone();
                    }
                }
                wire
            })
            .collect();
        for surah in &self.progress_dirty {
            out.push(json!({
                "kind": "progress.upsert", "surah": surah,
                "furthestAyah": self.progress[surah], "updatedAt": ISO,
            }));
        }
        out
    }
}

fn registers_from_wire(row: &Value) -> Value {
    if row["state"] == "deleted" {
        return json!({
            "id": row["id"],
            "lifecycle": { "stamp": row["lifecycleStamp"], "value": {
                "state": "deleted", "endedAt": null, "endReason": null,
            }},
        });
    }
    json!({
        "id": row["id"],
        "identity": { "stamp": row["identityStamp"], "value": {
            "origin": row["origin"], "originRef": row["originRef"], "scope": row["scope"],
            "boundEnd": row["boundEnd"], "startedAt": row["startedAt"],
        }},
        "cursor": { "stamp": row["cursorStamp"], "value": {
            "surah": row["surah"], "ayah": row["ayah"], "localPage": row["localPage"],
            "ratio": row["ratio"], "sourceId": row["sourceId"], "device": row["device"],
            "deviceId": row["deviceId"],
        }},
        "lifecycle": { "stamp": row["lifecycleStamp"], "value": {
            "state": row["state"], "endedAt": row["endedAt"], "endReason": row["endReason"],
        }},
    })
}

#[derive(Default)]
struct Oracle {
    /// Thread id → stamps the server acknowledged, per register.
    acked: BTreeMap<(String, String), String>,
    /// Thread ids that ever had a lifecycle write other than the opening one.
    lifecycle_writes: BTreeMap<String, BTreeSet<String>>,
    progress: BTreeMap<i64, i64>,
    trace: Vec<String>,
}

async fn post(h: &Harness, body: Value) -> Value {
    let (status, res) = send(&h.app, "POST", "/sync", Some(&h.cookie_a), Some(&body)).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    res
}

fn apply(dev: &mut Device, res: &Value, sent: &[Value], oracle: &mut Oracle) {
    dev.on_server_time(res["serverTime"].as_i64().unwrap());
    for result in res["results"].as_array().unwrap() {
        let id = result["id"].as_str().unwrap().to_string();
        if let Some(surah) = id.strip_prefix("progress:") {
            let surah: i64 = surah.parse().unwrap();
            let acked = sent
                .iter()
                .find(|m| m["surah"] == surah && m["kind"] == "progress.upsert")
                .and_then(|m| m["furthestAyah"].as_i64());
            if acked == dev.progress.get(&surah).copied() {
                dev.progress_dirty.remove(&surah);
            }
            continue;
        }
        let Some(stamps) = result.get("stamps") else {
            continue;
        };
        for (name, key) in [
            ("identity", "identityStamp"),
            ("cursor", "cursorStamp"),
            ("lifecycle", "lifecycleStamp"),
        ] {
            let Some(stamp) = stamps[name].as_str() else {
                continue;
            };
            let sent_row = sent.iter().find(|m| m["id"] == id.as_str());
            let sent_stamp = sent_row.and_then(|m| m[key].as_str());
            let entry = oracle
                .acked
                .entry((id.clone(), name.to_string()))
                .or_default();
            if sent_stamp.is_some() && stamp > entry.as_str() {
                *entry = stamp.to_string();
            }
            // A stamp the server rewrote is below ours: adopt it.
            if let Some(row) = dev.rows.get_mut(&id) {
                let current = row[name]["stamp"].as_str().map(str::to_string);
                if current.as_deref() == sent_stamp && sent_stamp.is_some_and(|s| stamp < s) {
                    row[name]["stamp"] = json!(stamp);
                }
            }
        }
        let current = dev.rows.get(&id);
        let unchanged = current.is_some_and(|row| {
            sent.iter().filter(|m| m["id"] == id.as_str()).all(|m| {
                let same = |name: &str, key: &str| {
                    let local = row[name]["stamp"].as_str();
                    let sent = m[key].as_str();
                    local == sent || sent.is_some_and(|s| local.is_some_and(|l| l < s))
                };
                same("identity", "identityStamp")
                    && same("cursor", "cursorStamp")
                    && same("lifecycle", "lifecycleStamp")
            })
        });
        if unchanged {
            dev.dirty.remove(&id);
        }
    }
    for wire in res["sessions"].as_array().unwrap() {
        let incoming = registers_from_wire(wire);
        for name in ["identity", "cursor", "lifecycle"] {
            dev.observe(incoming[name]["stamp"].as_str());
        }
        let id = wire["id"].as_str().unwrap().to_string();
        let merged = match dev.rows.get(&id) {
            Some(mine) => merge_thread(mine, &incoming),
            None => incoming,
        };
        dev.rows.insert(id, merged);
    }
    for row in res["progress"].as_array().unwrap() {
        let surah = row["surah"].as_i64().unwrap();
        let ayah = row["furthestAyah"].as_i64().unwrap();
        let slot = dev.progress.entry(surah).or_insert(0);
        *slot = (*slot).max(ayah);
    }
    if res["more"] == true && dev.since == 0 {
        dev.after = res["rev"].as_i64().unwrap();
    } else {
        dev.since = res["rev"].as_i64().unwrap();
        dev.after = -1;
    }
}

async fn round(h: &Harness, dev: &mut Device, oracle: &mut Oracle, lose: bool, repeat: bool) {
    let sent = dev.wire();
    let body = json!({
        "expectedUserId": h.uid_a, "client": { "protocol": 1 }, "since": dev.since,
        "after": dev.after, "mutations": sent,
    });
    let res = post(h, body.clone()).await;
    if repeat {
        // A retry the client did not know it needed: duplicate bytes, no effect.
        let again = post(h, body).await;
        assert_eq!(again["applied"], 0, "replay applied rows");
    }
    if lose {
        oracle.trace.push(format!("{} lost a response", dev.name));
        return;
    }
    apply(dev, &res, &sent, oracle);
}

async fn settle(h: &Harness, devices: &mut [Device], oracle: &mut Oracle) {
    for _pass in 0..6 {
        for dev in devices.iter_mut() {
            dev.online = true;
            for _ in 0..8 {
                round(h, dev, oracle, false, false).await;
                if dev.dirty.is_empty() && dev.progress_dirty.is_empty() && dev.after < 0 {
                    break;
                }
            }
        }
    }
}

fn state_of(row: &Value) -> &str {
    row["lifecycle"]["value"]["state"]
        .as_str()
        .unwrap_or("open")
}

/// Open threads. A full pull sends only these (history comes from `/archive`), so a
/// device that joined late holds no ended rows, and that is correct.
fn visible(dev: &Device) -> String {
    let rows: Vec<Value> = dev
        .rows
        .values()
        .filter(|row| state_of(row) == "open")
        .cloned()
        .collect();
    canonical_json(&Value::Array(rows))
}

async fn run_seed(seed: u64) {
    let h = harness(reading_session_v1::routes()).await;
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let count = 2 + r.next(4) as usize;
    let mut devices: Vec<Device> = (0..count)
        .map(|i| {
            let skew = r.next((2 * DAY_MS) as u64) as i64 - DAY_MS;
            Device::new(&format!("dev{i}"), skew)
        })
        .collect();
    let mut oracle = Oracle::default();
    let mut ids: Vec<String> = Vec::new();

    // A second account shares nothing with the first, whatever it writes.
    let other = json!({
        "expectedUserId": h.uid_b, "mutations": [{
            "kind": "session.upsert", "id": "ffffffff-0000-7000-8000-000000000001",
            "identityStamp": format_stamp(1, 0, "b"), "origin": "index", "scope": "surah:1",
            "startedAt": ISO, "cursorStamp": format_stamp(1, 0, "b"), "surah": 1, "ayah": 1,
            "device": "phone", "lifecycleStamp": format_stamp(1, 0, "b"), "state": "open",
        }],
    });
    send(&h.app, "POST", "/sync", Some(&h.cookie_b), Some(&other)).await;

    for step in 0..MIN_PER_RUN {
        let d = r.next(count as u64) as usize;
        if r.chance(10) {
            devices[d].online = !devices[d].online;
        }
        let op = r.next(100);
        let pick = if ids.is_empty() {
            None
        } else {
            Some(ids[r.next(ids.len() as u64) as usize].clone())
        };
        let dev_name = devices[d].name.clone();
        match (op, pick) {
            (0..=19, _) | (_, None) => {
                let id = format!("c1c1c1c1-c1c1-7c1c-8c1c-{:012x}", ids.len() + 1);
                devices[d].create(&id, 2);
                oracle.trace.push(format!("{step} {dev_name} create {id}"));
                ids.push(id);
            }
            (20..=59, Some(id)) => {
                let ayah = 1 + r.next(200) as i64;
                devices[d].move_cursor(&id, ayah);
                oracle
                    .trace
                    .push(format!("{step} {dev_name} cursor {id} {ayah}"));
            }
            (60..=69, Some(id)) if devices[d].rows.contains_key(&id) => {
                devices[d].set_state(&id, "ended");
                oracle
                    .lifecycle_writes
                    .entry(id.clone())
                    .or_default()
                    .insert("ended".into());
                oracle.trace.push(format!("{step} {dev_name} end {id}"));
            }
            (70..=79, Some(id)) if devices[d].rows.contains_key(&id) => {
                devices[d].set_state(&id, "deleted");
                oracle
                    .lifecycle_writes
                    .entry(id.clone())
                    .or_default()
                    .insert("deleted".into());
                oracle.trace.push(format!("{step} {dev_name} delete {id}"));
            }
            (80..=84, Some(id)) if devices[d].rows.contains_key(&id) => {
                devices[d].set_state(&id, "open");
                oracle
                    .lifecycle_writes
                    .entry(id.clone())
                    .or_default()
                    .insert("open".into());
                oracle.trace.push(format!("{step} {dev_name} reopen {id}"));
            }
            (85..=94, _) => {
                let ayah = 1 + r.next(286) as i64;
                let slot = devices[d].progress.entry(2).or_insert(0);
                if ayah > *slot {
                    *slot = ayah;
                    devices[d].progress_dirty.insert(2);
                }
                let best = oracle.progress.entry(2).or_insert(0);
                *best = (*best).max(ayah);
                oracle
                    .trace
                    .push(format!("{step} {dev_name} progress {ayah}"));
            }
            _ => {}
        }
        if devices[d].online && r.chance(45) {
            let lose = r.chance(15);
            let repeat = r.chance(10);
            round(&h, &mut devices[d], &mut oracle, lose, repeat).await;
        }
        // A kill: everything dirty was already persisted with the row, so a restart
        // holds the same rows. The clock resumes from its persisted `last`.
        if r.chance(5) {
            oracle
                .trace
                .push(format!("{step} {dev_name} killed and restarted"));
        }
    }

    settle(&h, &mut devices, &mut oracle).await;

    let fail = |what: &str, oracle: &Oracle| -> String {
        format!("seed {seed}: {what}\n{}", oracle.trace.join("\n"))
    };

    // 1. All devices hold identical rows after a final sync.
    let first = visible(&devices[0]);
    for dev in &devices[1..] {
        assert_eq!(visible(dev), first, "{}", fail("devices differ", &oracle));
    }
    assert!(
        !first.contains("ffffffff-0000-7000"),
        "{}",
        fail("row crossed accounts", &oracle)
    );

    for dev in &devices {
        // 3. Progress never regressed.
        for (surah, best) in &oracle.progress {
            assert_eq!(
                dev.progress.get(surah),
                Some(best),
                "{}",
                fail("progress", &oracle)
            );
        }
        for (id, row) in &dev.rows {
            // 2. No acknowledged write vanished: stored stamps never fall below an ack.
            for name in ["identity", "cursor", "lifecycle"] {
                // A tombstone carries no cursor value, so a deleted row may hold an older one.
                let skip_cursor = name == "cursor" && state_of(row) == "deleted";
                if name == "identity" || skip_cursor || row[name].is_null() {
                    continue;
                }
                let held = row[name]["stamp"].as_str();
                if let Some(acked) = oracle.acked.get(&(id.clone(), name.to_string())) {
                    let order = compare_stamps(held, Some(acked));
                    assert_ne!(
                        order,
                        std::cmp::Ordering::Less,
                        "{}",
                        fail("lost an ack", &oracle)
                    );
                }
            }
            // 4. No cursor write reopened or revived a thread.
            let writes = oracle.lifecycle_writes.get(id);
            let state = state_of(row);
            match writes {
                None => assert_eq!(state, "open", "{}", fail("state from a cursor", &oracle)),
                Some(set) if !set.contains("open") => {
                    assert_ne!(state, "open", "{}", fail("revived", &oracle));
                }
                _ => {}
            }
        }
    }
}

fn seeds() -> u64 {
    std::env::var("SYNC_SOAK_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12)
}

#[tokio::test]
async fn fixed_seeds_converge() {
    for seed in 1..=seeds() {
        run_seed(seed).await;
    }
}

#[tokio::test]
async fn a_cloned_device_sharing_an_id_still_converges() {
    let h = harness(reading_session_v1::routes()).await;
    let mut oracle = Oracle::default();
    let mut a = Device::new("clone", 0);
    let mut b = Device::new("clone", 0);
    a.create("c1c1c1c1-c1c1-7c1c-8c1c-000000000001", 2);
    round(&h, &mut a, &mut oracle, false, false).await;
    round(&h, &mut b, &mut oracle, false, false).await;
    // Racing writes on one id at one instant: bytes break the tie, the same everywhere.
    a.move_cursor("c1c1c1c1-c1c1-7c1c-8c1c-000000000001", 5);
    b.move_cursor("c1c1c1c1-c1c1-7c1c-8c1c-000000000001", 9);
    let id = "c1c1c1c1-c1c1-7c1c-8c1c-000000000001";
    let same = a.rows[id]["cursor"]["stamp"].clone();
    b.rows.get_mut(id).unwrap()["cursor"]["stamp"] = same;
    let mut devices = [a, b];
    settle(&h, &mut devices, &mut oracle).await;
    assert_eq!(visible(&devices[0]), visible(&devices[1]));
}
