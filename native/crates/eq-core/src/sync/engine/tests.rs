//! Scenario suite: real stores against a fake server that merges with the
//! same `eq-sync-core` rules, answers the status table, and can lose
//! responses after committing. The costliest bugs live here.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, VecDeque},
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use serde_json::{Value, json};

use super::*;
use crate::{
    merge::{canonical_json, merge_bookmark, merge_folder, merge_settings, merge_thread},
    net::{NetError, NetFuture},
    store::{
        AppStore, bookmarks, preferences,
        sessions::{self, ReaderTarget},
        test_support::{T0, TestClock},
    },
    sync::{ids::SeededRandom, transport::SyncPost},
};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("the fake server never pends"),
    }
}

// --- fake server ---

enum Script {
    Fail(NetError),
    /// Commit the request, then lose the response.
    LoseResponse,
}

#[derive(Default)]
struct UserData {
    revs: HashMap<&'static str, i64>,
    threads: BTreeMap<String, (Value, i64, String)>,
    progress: BTreeMap<i64, (i64, String, i64)>,
    folders: BTreeMap<String, (Value, i64, String)>,
    marks: BTreeMap<String, (Value, i64, Value)>,
    prefs: BTreeMap<String, (Value, i64)>,
}

impl UserData {
    fn bump(&mut self, domain: &'static str) -> i64 {
        let rev = self.revs.entry(domain).or_insert(0);
        *rev += 1;
        *rev
    }
    fn rev(&self, domain: &'static str) -> i64 {
        self.revs.get(domain).copied().unwrap_or(0)
    }
}

struct Server {
    now: Cell<i64>,
    epoch: RefCell<String>,
    users: RefCell<HashMap<i64, UserData>>,
    script: RefCell<VecDeque<Script>>,
    /// Accepted mutations left today (`None`: unlimited).
    ceiling: Cell<Option<usize>>,
    /// Every request body, by path.
    log: RefCell<Vec<(String, Value)>>,
    /// Extra fields a "newer server" adds to each pulled thread.
    future_fields: RefCell<Option<Value>>,
    /// The account the session belongs to.
    session_user: Cell<Option<i64>>,
}

impl Server {
    fn new() -> Self {
        Self {
            now: Cell::new(T0),
            epoch: RefCell::new("e1".into()),
            users: RefCell::default(),
            script: RefCell::default(),
            ceiling: Cell::new(None),
            log: RefCell::default(),
            future_fields: RefCell::new(None),
            session_user: Cell::new(None),
        }
    }

    fn push_script(&self, s: Script) {
        self.script.borrow_mut().push_back(s);
    }

    fn requests(&self, path: &str) -> Vec<Value> {
        self.log
            .borrow()
            .iter()
            .filter(|(p, _)| p == path)
            .map(|(_, b)| b.clone())
            .collect()
    }

    fn handle(&self, path: &str, body: Value) -> Result<Value, NetError> {
        self.log.borrow_mut().push((path.to_owned(), body.clone()));
        let user = body["expectedUserId"].as_i64().unwrap();
        if let Some(session) = self.session_user.get()
            && session != user
        {
            let message = match path {
                "/bookmark/v1/sync" => bookmarks::MISMATCH_MESSAGE,
                "/reading-session/v1/sync" => sessions::MISMATCH_MESSAGE,
                _ => preferences::MISMATCH_MESSAGE,
            };
            return Err(NetError::http(403, None, Some(message.to_owned())));
        }
        let scripted = self.script.borrow_mut().pop_front();
        if let Some(Script::Fail(err)) = scripted {
            return Err(err);
        }
        let mutations = body["mutations"].as_array().cloned().unwrap_or_default();
        // Daily ceiling: nothing fits → 429; some fit → the rest `capped`.
        let mut room = self.ceiling.get();
        if room == Some(0) && !mutations.is_empty() {
            return Err(NetError::http(429, Some(3600), None));
        }
        let mut users = self.users.borrow_mut();
        let data = users.entry(user).or_default();
        let mut results = Vec::new();
        for m in &mutations {
            if room == Some(0) {
                let id = m
                    .get("id")
                    .or_else(|| m.get("key"))
                    .cloned()
                    .unwrap_or(Value::Null);
                results.push(json!({ "id": id, "status": "capped" }));
                continue;
            }
            let result = match path {
                "/reading-session/v1/sync" => self.apply_session(data, m),
                "/bookmark/v1/sync" => self.apply_bookmark(data, user, m),
                _ => self.apply_pref(data, m),
            };
            if result["status"] != "rejected" {
                room = room.map(|r| r - 1);
            }
            results.push(result);
        }
        self.ceiling.set(room);
        let page = match path {
            "/reading-session/v1/sync" => self.pull_sessions(data, &body),
            "/bookmark/v1/sync" => self.pull_bookmarks(data, user, &body),
            _ => self.pull_prefs(data, &body),
        };
        let mut response = page;
        response["serverTime"] = json!(self.now.get());
        response["syncEpoch"] = json!(self.epoch.borrow().clone());
        response["results"] = json!(results);
        response["tombstoneFloor"] = json!(0);
        if let Some(Script::LoseResponse) = scripted {
            return Err(NetError::Offline("response lost".into()));
        }
        Ok(response)
    }

    fn bounded(&self, stamp: &Value) -> Value {
        let Some(parsed) = stamp.as_str().and_then(crate::merge::parse_stamp) else {
            return stamp.clone();
        };
        let ceiling = (self.now.get() + 60_000) as u64;
        if parsed.ms > ceiling {
            return json!(crate::merge::format_stamp(
                ceiling,
                parsed.counter,
                &parsed.device
            ));
        }
        stamp.clone()
    }

    fn apply_session(&self, data: &mut UserData, m: &Value) -> Value {
        if m["kind"] == "progress.upsert" {
            let surah = m["surah"].as_i64().unwrap();
            let ayah = m["furthestAyah"].as_i64().unwrap();
            let id = format!("progress:{surah}");
            let current = data.progress.get(&surah).map_or(0, |p| p.0);
            if ayah <= current {
                return json!({ "id": id, "status": "superseded" });
            }
            let rev = data.bump("sessions");
            data.progress.insert(
                surah,
                (ayah, m["updatedAt"].as_str().unwrap_or("").into(), rev),
            );
            return json!({ "id": id, "status": "applied" });
        }
        let id = m["id"].as_str().unwrap().to_owned();
        let mut incoming = json!({ "id": id });
        if !m["identityStamp"].is_null() {
            incoming["identity"] = json!({ "stamp": self.bounded(&m["identityStamp"]), "value": {
                "origin": m["origin"], "originRef": m["originRef"], "scope": m["scope"],
                "boundEnd": m["boundEnd"], "startedAt": m["startedAt"],
            }});
        }
        if !m["cursorStamp"].is_null() {
            incoming["cursor"] = json!({ "stamp": self.bounded(&m["cursorStamp"]), "value": {
                "surah": m["surah"], "ayah": m["ayah"], "localPage": m["localPage"],
                "ratio": m["ratio"], "sourceId": m["sourceId"], "device": m["device"],
                "deviceId": m["deviceId"],
            }});
        }
        if !m["lifecycleStamp"].is_null() {
            incoming["lifecycle"] = json!({ "stamp": self.bounded(&m["lifecycleStamp"]), "value": {
                "state": m["state"], "endedAt": m["endedAt"], "endReason": m["endReason"],
            }});
        }
        let current = data.threads.get(&id).map(|t| t.0.clone());
        if current.is_none()
            && (incoming.get("identity").is_none() || incoming.get("cursor").is_none())
        {
            return json!({ "id": id, "status": "rejected", "reason": "a new thread needs all registers" });
        }
        let merged = match &current {
            Some(cur) => merge_thread(cur, &incoming),
            None => incoming,
        };
        let stamps = json!({
            "identity": merged["identity"]["stamp"], "cursor": merged["cursor"]["stamp"],
            "lifecycle": merged["lifecycle"]["stamp"],
        });
        if current
            .as_ref()
            .is_some_and(|c| canonical_json(c) == canonical_json(&merged))
        {
            return json!({ "id": id, "status": "superseded", "stamps": stamps });
        }
        let rev = data.bump("sessions");
        data.threads.insert(
            id.clone(),
            (merged, rev, m["updatedAt"].as_str().unwrap_or("").into()),
        );
        json!({ "id": id, "status": "applied", "stamps": stamps, "rev": rev })
    }

    fn thread_wire(&self, id: &str, regs: &Value, rev: i64, updated: &str) -> Value {
        let stamps = json!({
            "identityStamp": regs["identity"]["stamp"], "cursorStamp": regs["cursor"]["stamp"],
            "lifecycleStamp": regs["lifecycle"]["stamp"],
        });
        if regs["lifecycle"]["value"]["state"] == "deleted" {
            let mut t = json!({ "id": id, "state": "deleted", "rev": rev });
            t.as_object_mut()
                .unwrap()
                .extend(stamps.as_object().unwrap().clone());
            return t;
        }
        let mut row = json!({ "id": id, "rev": rev, "updatedAt": updated, "deviceName": null });
        let o = row.as_object_mut().unwrap();
        for group in ["identity", "cursor", "lifecycle"] {
            if let Some(fields) = regs[group]["value"].as_object() {
                o.extend(fields.clone());
            }
        }
        o.extend(stamps.as_object().unwrap().clone());
        if let Some(extra) = self
            .future_fields
            .borrow()
            .as_ref()
            .and_then(Value::as_object)
        {
            o.extend(extra.clone());
        }
        row
    }

    fn page(&self, data_rev: i64, body: &Value, revs: Vec<i64>) -> (i64, i64, bool) {
        let since = body["since"].as_i64().unwrap_or(0);
        let after = if since == 0 {
            body["after"].as_i64().unwrap_or(-1)
        } else {
            since
        };
        let _ = revs;
        (since, after, data_rev > 0)
    }

    fn pull_sessions(&self, data: &UserData, body: &Value) -> Value {
        let (since, after, _) = self.page(data.rev("sessions"), body, Vec::new());
        let sessions: Vec<Value> = data
            .threads
            .iter()
            .filter(|(_, t)| t.1 > after)
            .map(|(id, t)| self.thread_wire(id, &t.0, t.1, &t.2))
            .collect();
        let progress: Vec<Value> = data
            .progress
            .iter()
            .filter(|(_, p)| p.2 > after)
            .map(|(s, p)| json!({ "surah": s, "furthestAyah": p.0, "updatedAt": p.1, "rev": p.2 }))
            .collect();
        let latest = data.rev("sessions");
        let _ = since;
        json!({ "sessions": sessions, "progress": progress, "rev": latest, "latestRev": latest, "more": false })
    }

    fn apply_bookmark(&self, data: &mut UserData, user: i64, m: &Value) -> Value {
        let id = m["id"].as_str().unwrap().to_owned();
        let default_id = bookmarks::default_folder_id_for(Some(user));
        if m["kind"] == "folder.upsert" {
            let mut incoming = json!({ "id": id, "isDefault": id == default_id });
            for (reg, field) in [
                ("name", "name"),
                ("position", "position"),
                ("deleted", "deleted"),
            ] {
                let stamp_key = format!("{reg}Stamp");
                if !m[&stamp_key].is_null() {
                    incoming[reg] =
                        json!({ "stamp": self.bounded(&m[&stamp_key]), "value": m[field] });
                }
            }
            let current = data.folders.get(&id).map(|f| f.0.clone());
            let merged = match &current {
                Some(cur) => merge_folder(cur, &incoming),
                None => incoming,
            };
            let stamps = json!({
                "name": merged["name"]["stamp"], "position": merged["position"]["stamp"],
                "deleted": merged["deleted"]["stamp"],
            });
            if current
                .as_ref()
                .is_some_and(|c| canonical_json(c) == canonical_json(&merged))
            {
                return json!({ "id": id, "status": "superseded", "stamps": stamps });
            }
            let rev = data.bump("bookmarks");
            data.folders.insert(
                id.clone(),
                (merged, rev, m["updatedAt"].as_str().unwrap_or("").into()),
            );
            return json!({ "id": id, "status": "applied", "stamps": stamps });
        }
        let known_folder = data
            .folders
            .contains_key(m["folderId"].as_str().unwrap_or(""));
        let folder_id = if known_folder {
            m["folderId"].clone()
        } else {
            json!(default_id)
        };
        let key = format!(
            "{}:{}-{}:{}",
            m["surah"],
            m["ayah"],
            m["endSurah"]
                .as_i64()
                .unwrap_or(m["surah"].as_i64().unwrap()),
            m["endAyah"].as_i64().unwrap_or(m["ayah"].as_i64().unwrap())
        );
        let incoming = json!({ "id": id, "register": { "stamp": self.bounded(&m["stamp"]), "value": {
            "context": { "sourceId": m["sourceId"] }, "deleted": m["deleted"],
            "folderId": folder_id, "key": key,
        }}});
        let current = data.marks.get(&id).map(|b| b.0.clone());
        let merged = match &current {
            Some(cur) => merge_bookmark(cur, &incoming),
            None => incoming.clone(),
        };
        let stamps = json!({ "row": merged["register"]["stamp"] });
        if current
            .as_ref()
            .is_some_and(|c| canonical_json(c) == canonical_json(&merged))
        {
            return json!({ "id": id, "status": "superseded", "stamps": stamps });
        }
        let rev = data.bump("bookmarks");
        let mut fields = m.clone();
        fields["folderId"] = folder_id;
        data.marks.insert(id.clone(), (merged, rev, fields));
        json!({ "id": id, "status": "applied", "stamps": stamps })
    }

    fn pull_bookmarks(&self, data: &UserData, user: i64, body: &Value) -> Value {
        let (_, after, _) = self.page(0, body, Vec::new());
        let folders: Vec<Value> = data
            .folders
            .iter()
            .filter(|(_, f)| f.1 > after)
            .map(|(id, f)| {
                let regs = &f.0;
                let stamps = |k: &str| regs[k]["stamp"].clone();
                if regs["deleted"]["value"] == true {
                    return json!({ "id": id, "deleted": true, "rev": f.1, "nameStamp": stamps("name"),
                        "positionStamp": stamps("position"), "deletedStamp": stamps("deleted") });
                }
                json!({ "id": id, "name": regs["name"]["value"], "isDefault": regs["isDefault"],
                    "position": regs["position"]["value"], "deleted": false, "rev": f.1,
                    "updatedAt": f.2, "nameStamp": stamps("name"), "positionStamp": stamps("position"),
                    "deletedStamp": stamps("deleted") })
            })
            .collect();
        let marks: Vec<Value> = data
            .marks
            .iter()
            .filter(|(_, b)| b.1 > after)
            .map(|(id, b)| {
                let value = &b.0["register"]["value"];
                let f = &b.2;
                json!({ "id": id, "folderId": value["folderId"], "surah": f["surah"], "ayah": f["ayah"],
                    "endSurah": f["endSurah"], "endAyah": f["endAyah"], "deleted": value["deleted"],
                    "stamp": b.0["register"]["stamp"], "rev": b.1, "sourceId": f["sourceId"],
                    "createdAt": f["updatedAt"], "updatedAt": f["updatedAt"] })
            })
            .collect();
        let latest = data.rev("bookmarks");
        json!({ "folders": folders, "bookmarks": marks, "defaultFolderId": bookmarks::default_folder_id_for(Some(user)),
            "rev": latest, "latestRev": latest, "more": false })
    }

    fn apply_pref(&self, data: &mut UserData, m: &Value) -> Value {
        let key = m["key"].as_str().unwrap_or_default().to_owned();
        let incoming = json!({ key.clone(): { "stamp": m["stamp"], "value": m["value"] } });
        let current = data.prefs.get(&key).map(|p| json!({ key.clone(): p.0 }));
        let merged = match &current {
            Some(cur) => merge_settings(cur, &incoming),
            None => incoming,
        };
        let held = merged[&key]["stamp"].clone();
        if current
            .as_ref()
            .is_some_and(|c| canonical_json(c) == canonical_json(&merged))
        {
            return json!({ "id": key, "status": "superseded", "stamps": { "row": held } });
        }
        let rev = data.bump("preferences");
        data.prefs.insert(key.clone(), (merged[&key].clone(), rev));
        json!({ "id": key, "status": "applied", "stamps": { "row": held } })
    }

    fn pull_prefs(&self, data: &UserData, body: &Value) -> Value {
        let since = body["since"].as_i64().unwrap_or(0);
        let rows: Vec<Value> = data
            .prefs
            .iter()
            .filter(|(_, p)| p.1 > since)
            .map(|(k, p)| json!({ "key": k, "value": p.0["value"], "stamp": p.0["stamp"], "rev": p.1 }))
            .collect();
        let latest = data.rev("preferences");
        json!({ "preferences": rows, "rev": latest, "latestRev": latest, "more": false })
    }
}

impl SyncPost for Server {
    fn post_json(&self, path: &str, body: Value) -> NetFuture<'_, Value> {
        let result = self.handle(path, body);
        Box::pin(async move { result })
    }
}

// --- devices ---

struct Device {
    store: RefCell<AppStore>,
    engine: RefCell<SyncEngine>,
    clock: TestClock,
}

impl Device {
    fn new(seed: u64) -> Self {
        let clock = TestClock::new(T0);
        let store =
            AppStore::open_in_memory(Box::new(clock.clone()), Box::new(SeededRandom::new(seed)))
                .unwrap();
        Self {
            store: RefCell::new(store),
            engine: RefCell::new(SyncEngine::new(ClientInfo::default())),
            clock,
        }
    }

    fn signed_in(seed: u64, user: i64, device: &str) -> Self {
        let d = Self::new(seed);
        d.store.borrow_mut().sign_in(user, device).unwrap();
        d
    }

    fn sync(&self, server: &Server) -> FlushReport {
        self.sync_with(server, true, None)
    }

    fn sync_with(&self, server: &Server, online: bool, only: Option<SyncDomain>) -> FlushReport {
        let clock = self.clock.clone();
        let now = move || crate::clock::Clock::unix_ms(&clock);
        block_on(flush(
            &self.engine,
            &self.store,
            server,
            FlushOptions {
                now: &now,
                online,
                jitter: 0.0,
                only,
            },
        ))
    }

    fn status(&self) -> RoundStatus {
        let counts = store_counts(&self.store.borrow()).unwrap();
        self.engine.borrow().status(
            counts,
            self.clock.0.load(std::sync::atomic::Ordering::SeqCst),
        )
    }

    fn pending(&self) -> usize {
        self.status().pending
    }

    fn marks(&self) -> Vec<(i64, i64, String)> {
        let mut out: Vec<_> = bookmarks::bookmarks(&self.store.borrow())
            .unwrap()
            .into_iter()
            .map(|b| (b.surah, b.ayah, b.folder_id))
            .collect();
        out.sort();
        out
    }

    fn threads(&self) -> Vec<(String, i64, i64, String)> {
        let mut out: Vec<_> = sessions::all(&self.store.borrow())
            .unwrap()
            .into_iter()
            .map(|t| (t.id, t.surah, t.ayah, t.state))
            .collect();
        out.sort();
        out
    }
}

fn target(scope: &str, surah: i64, ayah: i64) -> ReaderTarget {
    ReaderTarget {
        scope: scope.into(),
        surah,
        ayah,
        exact: false,
        bound_end: None,
        source_id: None,
    }
}

fn read(device: &Device, surah: i64, ayah: i64) {
    let mut store = device.store.borrow_mut();
    sessions::enter(
        &mut store,
        &target(&format!("surah:{surah}"), surah, 1),
        None,
        true,
    )
    .unwrap();
    sessions::note_read(&mut store, surah, ayah).unwrap();
}

// --- scenarios ---

#[test]
fn signed_out_flush_is_a_no_op() {
    let server = Server::new();
    let device = Device::new(1);
    bookmarks::toggle(&mut device.store.borrow_mut(), 1, 1, None).unwrap();
    let report = device.sync(&server);
    assert_eq!(report, FlushReport::default());
    assert!(server.log.borrow().is_empty());
    assert_eq!(device.status().phase, Phase::Idle);
}

#[test]
fn offline_edits_on_two_devices_converge() {
    let server = Server::new();
    let a = Device::signed_in(1, 7, "dev-a");
    let b = Device::signed_in(2, 7, "dev-b");
    a.sync(&server);
    b.sync(&server);

    // Offline: both edit bookmarks, folders, threads and a setting.
    let folder_a = bookmarks::create_folder(&mut a.store.borrow_mut(), "Hifz")
        .unwrap()
        .unwrap();
    // Toggle before "Save to…": that remembers Hifz as the target folder
    // (web `#rememberFolder`), and 1:1 must land in the default folder on
    // both devices for the one-key rule below.
    bookmarks::toggle(&mut a.store.borrow_mut(), 1, 1, None).unwrap();
    bookmarks::set_in_folder(&mut a.store.borrow_mut(), 2, 255, &folder_a.id, true, None).unwrap();
    b.clock.advance(5);
    bookmarks::toggle(&mut b.store.borrow_mut(), 1, 1, None).unwrap();
    bookmarks::toggle(&mut b.store.borrow_mut(), 18, 10, None).unwrap();
    read(&a, 2, 10);
    read(&b, 3, 20);
    preferences::capture(
        &mut a.store.borrow_mut(),
        Some(&[("settings.appearance.theme".into(), json!("dark"))]),
    )
    .unwrap();
    preferences::capture(
        &mut b.store.borrow_mut(),
        Some(&[("settings.reading.fontSize".into(), json!(30))]),
    )
    .unwrap();
    assert!(a.sync_with(&server, false, None) == FlushReport::default());
    assert_eq!(a.status().phase, Phase::Error);
    assert!(a.pending() > 0);

    // Back online: A, B, A.
    a.engine.borrow_mut().note_online();
    a.sync(&server);
    let report_b = b.sync(&server);
    a.sync(&server);

    assert_eq!(a.marks(), b.marks());
    // One verse bookmarked on two devices in one folder: one row survives.
    assert_eq!(a.marks().iter().filter(|m| m.0 == 1 && m.1 == 1).count(), 1);
    assert_eq!(a.threads(), b.threads());
    assert_eq!(a.threads().len(), 2);
    assert_eq!(a.pending(), 0);
    assert_eq!(b.pending(), 0);
    // Two settings changed on two devices both survive.
    assert!(
        report_b
            .settings
            .iter()
            .any(|(k, v)| k == "settings.appearance.theme" && v == "dark")
    );
    let s = |d: &Device, k: &str| preferences::get(&d.store.borrow(), k).unwrap();
    assert_eq!(s(&a, "settings.reading.fontSize"), json!(30));
    assert_eq!(s(&b, "settings.appearance.theme"), json!("dark"));
    // Progress keeps the MAX on both.
    assert_eq!(
        sessions::furthest_in(&b.store.borrow(), 2).unwrap(),
        Some(10)
    );
}

#[test]
fn a_lost_response_re_push_is_idempotent() {
    let server = Server::new();
    let a = Device::signed_in(3, 7, "dev-a");
    bookmarks::toggle(&mut a.store.borrow_mut(), 2, 255, None).unwrap();
    server.push_script(Script::LoseResponse);
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    assert_eq!(a.pending(), 1, "no ack: the row stays dirty");
    let rev_after_first = server.users.borrow()[&7].rev("bookmarks");
    a.engine.borrow_mut().note_online();
    a.sync(&server);
    assert_eq!(a.pending(), 0);
    let last = server.requests("/bookmark/v1/sync");
    let replay = &last[1]["mutations"];
    assert_eq!(
        replay.as_array().unwrap().len(),
        1,
        "the same row went again"
    );
    assert_eq!(
        server.users.borrow()[&7].rev("bookmarks"),
        rev_after_first,
        "replay changed nothing"
    );
    assert_eq!(a.marks().len(), 1);
}

#[test]
fn kill_between_push_and_ack_loses_nothing() {
    let dir = std::env::temp_dir().join(format!("eq-core-kill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let server = Server::new();
    let clock = TestClock::new(T0);
    let open = |seed| {
        AppStore::open(
            &dir,
            Box::new(clock.clone()),
            Box::new(SeededRandom::new(seed)),
        )
        .unwrap()
    };
    {
        let mut store = open(1);
        store.sign_in(7, "dev").unwrap();
        bookmarks::toggle(&mut store, 2, 255, None).unwrap();
        sessions::enter(&mut store, &target("surah:2", 2, 1), None, false).unwrap();
        sessions::note_read(&mut store, 2, 3).unwrap();
        let device = Device {
            store: RefCell::new(store),
            engine: RefCell::new(SyncEngine::new(ClientInfo::default())),
            clock: clock.clone(),
        };
        server.push_script(Script::LoseResponse);
        server.push_script(Script::LoseResponse);
        device.sync(&server);
        // The process dies here: server committed, no ack ever landed.
    }
    let mut store = open(2);
    store.sign_in(7, "dev").unwrap();
    let device = Device {
        store: RefCell::new(store),
        engine: RefCell::new(SyncEngine::new(ClientInfo::default())),
        clock,
    };
    assert_eq!(
        device.pending(),
        3,
        "bookmark, thread and progress still dirty on disk"
    );
    device.sync(&server);
    assert_eq!(device.pending(), 0);
    assert_eq!(server.users.borrow()[&7].marks.len(), 1);
    assert_eq!(server.users.borrow()[&7].threads.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_while_the_request_flies_stays_dirty() {
    let server = Server::new();
    let a = Device::signed_in(4, 7, "dev-a");
    read(&a, 2, 5);
    let prepared = prepare_round(
        &a.store.borrow(),
        SyncDomain::Sessions,
        false,
        &ClientInfo::default(),
    )
    .unwrap()
    .unwrap();
    let response = server
        .handle("/reading-session/v1/sync", prepared.body.clone())
        .unwrap();
    // The reader moves on before the response is applied.
    a.clock.advance(10);
    sessions::cursor(&mut a.store.borrow_mut(), 2, 9, None).unwrap();
    apply_round(
        &mut a.store.borrow_mut(),
        SyncDomain::Sessions,
        7,
        &response,
        &prepared.sent,
    )
    .unwrap();
    let thread = sessions::all(&a.store.borrow()).unwrap().remove(0);
    assert!(thread.dirty, "the newer cursor still waits");
    assert_eq!(thread.ayah, 9);
    a.sync(&server);
    assert_eq!(a.pending(), 0);
}

#[test]
fn unknown_fields_and_registers_survive_a_round_trip() {
    let server = Server::new();
    *server.future_fields.borrow_mut() = Some(json!({ "futureField": { "x": 1 }, "mood": "calm" }));
    let a = Device::signed_in(5, 7, "dev-a");
    let b = Device::signed_in(6, 7, "dev-b");
    read(&a, 4, 2);
    a.sync(&server);
    b.sync(&server);
    let pulled = sessions::all(&b.store.borrow()).unwrap().remove(0);
    assert_eq!(pulled.extra.get("futureField"), Some(&json!({ "x": 1 })));
    // B edits the thread: the unknown fields go back with it and stay local.
    let id = pulled.id.clone();
    {
        let mut store = b.store.borrow_mut();
        sessions::enter(
            &mut store,
            &target("surah:4", 4, 1),
            Some(sessions::EntryIntent::resume(&id)),
            false,
        )
        .unwrap();
        sessions::cursor(&mut store, 4, 7, None).unwrap();
    }
    b.sync(&server);
    let sent = server.requests("/reading-session/v1/sync");
    let last_push = sent
        .iter()
        .rev()
        .find(|r| !r["mutations"].as_array().unwrap().is_empty())
        .unwrap();
    assert_eq!(last_push["mutations"][0]["mood"], "calm");
    let after = sessions::all(&b.store.borrow()).unwrap().remove(0);
    assert_eq!(after.extra.get("mood"), Some(&json!("calm")));
    assert_eq!(after.ayah, 7);
    // An unknown enum reads neutrally and the next round keeps working.
    *server.future_fields.borrow_mut() = Some(json!({ "origin": "voice-assistant" }));
    sessions::cursor(&mut b.store.borrow_mut(), 4, 8, None).unwrap();
    b.sync(&server);
    assert_eq!(b.status().phase, Phase::Idle);
}

#[test]
fn account_switch_isolation() {
    let server = Server::new();
    let device = Device::signed_in(7, 1, "dev-1");
    bookmarks::toggle(&mut device.store.borrow_mut(), 1, 1, None).unwrap();
    read(&device, 1, 3);
    // Session expires before a sync: rows wait for user 1.
    device.store.borrow_mut().sign_out();
    device.engine.borrow_mut().reset_for_account();
    device.store.borrow_mut().sign_in(2, "dev-2").unwrap();
    assert!(device.marks().is_empty());
    assert!(device.threads().is_empty());
    device.sync(&server);
    for (path, body) in server.log.borrow().iter() {
        assert_eq!(body["expectedUserId"], 2);
        let mutations = body["mutations"].as_array().unwrap();
        if path == preferences::SYNC_PATH {
            // User 2's own first capture after the baseline pull (web
            // `capture()`): the device's home layout and an empty goals
            // document, stamped by dev-2. Nothing of user 1's.
            for m in mutations {
                assert!(
                    m["key"] == preferences::LAYOUT_KEY || m["key"] == preferences::GOALS_KEY,
                    "user 2 pushed {m}"
                );
                assert!(m["stamp"].as_str().unwrap().ends_with("-dev-2"));
            }
            continue;
        }
        assert!(
            mutations.is_empty(),
            "user 1's rows leaked to user 2: {mutations:?}"
        );
    }
    assert!(
        !server
            .users
            .borrow()
            .get(&1)
            .is_some_and(|u| !u.marks.is_empty())
    );
    // User 1 comes back: the waiting rows drain to user 1 only.
    device.store.borrow_mut().sign_out();
    device.store.borrow_mut().sign_in(1, "dev-1").unwrap();
    assert_eq!(device.pending(), 3);
    device.sync(&server);
    assert_eq!(server.users.borrow()[&1].marks.len(), 1);
    assert!(server.users.borrow()[&2].marks.is_empty());
}

#[test]
fn a_401_keeps_rows_dirty_and_pauses_without_error() {
    let server = Server::new();
    let a = Device::signed_in(8, 7, "dev");
    bookmarks::toggle(&mut a.store.borrow_mut(), 1, 1, None).unwrap();
    for _ in 0..3 {
        server.push_script(Script::Fail(NetError::http(401, None, None)));
    }
    a.sync(&server);
    let status = a.status();
    assert_eq!(status.pending, 1);
    assert_eq!(status.phase, Phase::Idle);
    assert_eq!(a.engine.borrow().retry_at(), None, "no backoff for a pause");
}

#[test]
fn an_account_mismatch_pauses_and_reports_session_lost() {
    let server = Server::new();
    server.session_user.set(Some(99));
    let a = Device::signed_in(9, 7, "dev");
    bookmarks::toggle(&mut a.store.borrow_mut(), 1, 1, None).unwrap();
    a.sync(&server);
    let status = a.status();
    assert!(status.session_lost);
    assert_eq!(status.pending, 1);
}

#[test]
fn server_errors_back_off_and_rows_stay_dirty() {
    let server = Server::new();
    let a = Device::signed_in(10, 7, "dev");
    bookmarks::toggle(&mut a.store.borrow_mut(), 1, 1, None).unwrap();
    server.push_script(Script::Fail(NetError::http(503, None, None)));
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    let status = a.status();
    assert_eq!(status.phase, Phase::Error);
    assert_eq!(status.pending, 1);
    assert_eq!(a.engine.borrow().retry_at(), Some(T0 + 2_000));
    let poll = |now| PollInput {
        now,
        focused: false,
        online: true,
        signed_in: true,
    };
    assert_eq!(should_run(&a.engine.borrow(), poll(T0 + 1_999)), None);
    assert_eq!(
        should_run(&a.engine.borrow(), poll(T0 + 2_000)),
        Some(Trigger::Retry)
    );
    a.clock.advance(2_000);
    a.sync(&server);
    assert_eq!(a.status().phase, Phase::Idle);
    assert_eq!(a.pending(), 0);
}

#[test]
fn a_429_holds_pushes_and_still_pulls() {
    let server = Server::new();
    let a = Device::signed_in(11, 7, "dev-a");
    let b = Device::signed_in(12, 7, "dev-b");
    bookmarks::toggle(&mut b.store.borrow_mut(), 9, 9, None).unwrap();
    b.sync(&server);
    bookmarks::toggle(&mut a.store.borrow_mut(), 1, 1, None).unwrap();
    server.ceiling.set(Some(0));
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    // The push was held; the pull in the same flush brought B's row.
    assert!(a.marks().iter().any(|m| m.0 == 9));
    assert_eq!(a.pending(), 1);
    assert!(a.status().held_until.is_some());
    let pulls = server.requests("/bookmark/v1/sync");
    assert!(
        pulls.last().unwrap()["mutations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // During the hold every flush pulls only; after it, pushes resume.
    server.ceiling.set(None);
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    assert_eq!(a.pending(), 1);
    a.clock.advance(3_600_000);
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    assert_eq!(a.pending(), 0);
}

#[test]
fn capped_rows_stay_dirty_and_retry_when_room_appears() {
    let server = Server::new();
    let a = Device::signed_in(13, 7, "dev");
    for ayah in 1..=3 {
        bookmarks::toggle(&mut a.store.borrow_mut(), 1, ayah, None).unwrap();
    }
    server.ceiling.set(Some(2));
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    let status = a.status();
    assert_eq!(status.pending, 1);
    assert_eq!(status.capped, 1);
    server.ceiling.set(None);
    a.sync_with(&server, true, Some(SyncDomain::Bookmarks));
    assert_eq!(a.status().capped, 0);
    assert_eq!(a.pending(), 0);
}

#[test]
fn rejected_rows_are_kept_and_not_resent_until_edited() {
    let server = Server::new();
    let a = Device::signed_in(14, 7, "dev");
    read(&a, 2, 5);
    // Lose the identity register locally so the server refuses the new id.
    {
        let store = a.store.borrow();
        let mut thread = sessions::all(&store).unwrap().remove(0);
        thread.identity_stamp = None;
        crate::store::rows::save(store.owner_conn(), &thread).unwrap();
    }
    a.sync_with(&server, true, Some(SyncDomain::Sessions));
    assert_eq!(a.status().rejected, 1);
    let sent_before = server.requests("/reading-session/v1/sync").len();
    a.sync_with(&server, true, Some(SyncDomain::Sessions));
    let last = server.requests("/reading-session/v1/sync");
    assert_eq!(last.len(), sent_before + 1);
    assert!(
        last.last().unwrap()["mutations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        sessions::all(&a.store.borrow()).unwrap().len(),
        1,
        "kept locally"
    );
}

#[test]
fn a_restored_server_forces_a_full_pull_and_re_push() {
    let server = Server::new();
    let a = Device::signed_in(15, 7, "dev");
    bookmarks::toggle(&mut a.store.borrow_mut(), 1, 1, None).unwrap();
    a.sync(&server);
    // Restore from backup: the server lost the row and the epoch changed.
    server.users.borrow_mut().clear();
    *server.epoch.borrow_mut() = "e2".into();
    a.sync(&server);
    assert_eq!(
        server.users.borrow()[&7].marks.len(),
        1,
        "the device healed the restore"
    );
    assert_eq!(a.pending(), 0);
}

#[test]
fn a_thread_ended_on_one_device_stays_ended_when_a_stale_cursor_lands() {
    let server = Server::new();
    let a = Device::signed_in(16, 7, "dev-a");
    read(&a, 2, 5);
    a.sync(&server);
    let id = a.threads()[0].0.clone();
    a.clock.advance(1000);
    sessions::dismiss(&mut a.store.borrow_mut(), &id).unwrap();
    a.sync(&server);
    // A stale copy that never saw the end moves its cursor.
    let stale = Device::signed_in(18, 7, "dev-c");
    stale.sync(&server);
    {
        let store = stale.store.borrow();
        let mut thread = sessions::all(&store).unwrap().remove(0);
        // Pretend C never saw the end: old lifecycle, newer cursor.
        thread.state = "open".into();
        thread.ended_at = None;
        thread.end_reason = None;
        thread.lifecycle_stamp = Some(crate::merge::format_stamp(T0 as u64, 0, "dev-a"));
        thread.ayah = 40;
        thread.cursor_stamp = Some(crate::merge::format_stamp((T0 + 5_000) as u64, 0, "dev-c"));
        thread.dirty = true;
        crate::store::rows::save(store.owner_conn(), &thread).unwrap();
    }
    stale.sync(&server);
    a.sync(&server);
    let after = sessions::all(&a.store.borrow()).unwrap().remove(0);
    assert_eq!(
        after.state, "ended",
        "a stale reader never reopens a thread"
    );
    assert_eq!(after.ayah, 40, "its cursor still lands");
}

#[test]
fn the_accounts_preferences_win_on_a_new_device_and_capture_runs_after_baseline() {
    let server = Server::new();
    let a = Device::signed_in(19, 7, "dev-a");
    a.sync(&server);
    preferences::capture(
        &mut a.store.borrow_mut(),
        Some(&[("settings.appearance.theme".into(), json!("dark"))]),
    )
    .unwrap();
    a.sync(&server);
    // B has a local "light" before its first pull: not captured yet.
    let b = Device::signed_in(20, 7, "dev-b");
    preferences::capture(
        &mut b.store.borrow_mut(),
        Some(&[("settings.appearance.theme".into(), json!("light"))]),
    )
    .unwrap();
    assert_eq!(b.pending(), 0, "nothing captured before the baseline");
    let report = b.sync(&server);
    assert!(report.capture_settings);
    assert!(
        report
            .settings
            .contains(&("settings.appearance.theme".into(), json!("dark")))
    );
    assert_eq!(
        preferences::get(&b.store.borrow(), "settings.appearance.theme").unwrap(),
        json!("dark")
    );
}

#[test]
fn goals_and_layout_sync_whole_and_owners_stay_apart() {
    use crate::store::{goals, home};
    let server = Server::new();
    let a = Device::signed_in(21, 7, "dev-a");
    let b = Device::signed_in(22, 7, "dev-b");
    a.sync(&server);
    b.sync(&server);
    let draft = goals::preset_draft(goals::GoalPreset::Finish30, "2026-11-18");
    assert_eq!(goals::add(&mut a.store.borrow_mut(), &draft).unwrap(), None);
    home::toggle_widget(&mut a.store.borrow_mut(), "notes").unwrap();
    a.sync(&server);
    b.sync(&server);
    assert_eq!(goals::entries(&b.store.borrow()).unwrap().len(), 1);
    assert!(
        !home::layout(&b.store.borrow())
            .unwrap()
            .iter()
            .find(|w| w.id == "notes")
            .unwrap()
            .visible
    );
    // Another account on B sees none of user 7's goals.
    b.store.borrow_mut().sign_out();
    b.store.borrow_mut().sign_in(8, "dev-b8").unwrap();
    assert!(goals::entries(&b.store.borrow()).unwrap().is_empty());
}

#[test]
fn folder_delete_kills_older_bookmarks_and_moves_newer_ones() {
    let server = Server::new();
    let a = Device::signed_in(23, 7, "dev-a");
    let b = Device::signed_in(24, 7, "dev-b");
    let folder = bookmarks::create_folder(&mut a.store.borrow_mut(), "Trip")
        .unwrap()
        .unwrap();
    bookmarks::set_in_folder(&mut a.store.borrow_mut(), 1, 1, &folder.id, true, None).unwrap();
    a.sync(&server);
    b.sync(&server);
    a.clock.advance(1_000);
    bookmarks::delete_folder(&mut a.store.borrow_mut(), &folder.id).unwrap();
    b.clock.advance(5_000);
    bookmarks::set_in_folder(&mut b.store.borrow_mut(), 2, 2, &folder.id, true, None).unwrap();
    a.sync(&server);
    b.sync(&server);
    a.sync(&server);
    let default_id = bookmarks::default_folder_id_for(Some(7));
    for device in [&a, &b] {
        let marks = device.marks();
        assert!(
            !marks.iter().any(|m| m.0 == 1 && m.1 == 1),
            "older bookmark died with the folder"
        );
        assert!(
            marks.contains(&(2, 2, default_id.clone())),
            "newer one moved to the default"
        );
    }
}

#[test]
fn ten_thousand_offline_scrolls_are_one_dirty_row() {
    let a = Device::signed_in(25, 7, "dev");
    read(&a, 2, 1);
    for i in 0..500 {
        sessions::cursor(&mut a.store.borrow_mut(), 2, 1 + (i % 286), None).unwrap();
    }
    let rows = sessions::dirty_rows(&a.store.borrow()).unwrap();
    assert_eq!(
        rows.iter()
            .filter(|r| r["kind"] == "session.upsert")
            .count(),
        1
    );
}

#[test]
fn a_clock_years_ahead_is_rewritten_and_converges() {
    let server = Server::new();
    let fast = Device::signed_in(26, 7, "dev-fast");
    fast.clock.set(T0 + 365 * 86_400_000);
    bookmarks::toggle(&mut fast.store.borrow_mut(), 1, 1, None).unwrap();
    // The server rewrites the stamp to server + 60 s; the device adopts it and
    // its clock is pulled back.
    fast.sync(&server);
    let stored = bookmarks::bookmarks(&fast.store.borrow())
        .unwrap()
        .remove(0);
    let adopted = crate::merge::parse_stamp(stored.stamp.as_deref().unwrap())
        .unwrap()
        .ms as i64;
    assert_eq!(adopted, T0 + 60_000);
    let stamp = {
        let store = fast.store.borrow();
        let mut c = store.account.as_ref().unwrap().clock.clone();
        c.stamp(crate::clock::Clock::unix_ms(&fast.clock))
    };
    let ms = crate::merge::parse_stamp(&stamp).unwrap().ms as i64;
    assert!(
        ms <= T0 + 60_001,
        "stamps follow server time, not the device clock"
    );
    assert_eq!(
        fast.store.borrow().now(),
        T0,
        "now() is device time corrected by the offset"
    );
    // An honest device writing a minute later wins.
    let honest = Device::signed_in(27, 7, "dev-honest");
    honest.sync(&server);
    honest.clock.advance(61_000);
    bookmarks::toggle(&mut honest.store.borrow_mut(), 1, 1, None).unwrap();
    honest.sync(&server);
    fast.sync(&server);
    assert!(fast.marks().is_empty(), "the honest delete converged");
}

#[test]
fn retry_delays_double_and_cap() {
    let delays: Vec<i64> = (1..=8).map(retry_delay_ms).collect();
    assert_eq!(
        delays,
        vec![2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000, 60_000]
    );
}

#[test]
fn periodic_poll_only_while_focused_signed_in_and_online() {
    let engine = SyncEngine::default();
    let input = PollInput {
        now: T0,
        focused: true,
        online: true,
        signed_in: true,
    };
    assert_eq!(should_run(&engine, input), Some(Trigger::Periodic));
    assert_eq!(
        should_run(
            &engine,
            PollInput {
                focused: false,
                ..input
            }
        ),
        None
    );
    assert_eq!(
        should_run(
            &engine,
            PollInput {
                signed_in: false,
                ..input
            }
        ),
        None
    );
    assert_eq!(
        should_run(
            &engine,
            PollInput {
                online: false,
                ..input
            }
        ),
        None
    );
}
