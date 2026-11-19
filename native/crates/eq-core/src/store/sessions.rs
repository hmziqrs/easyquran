//! Reading threads (PARITY G1, G2; web `lib/sessions/*`). A thread is one
//! continuous run of reading with a UUID v7 id: entering from somewhere else
//! starts (or resumes) one, continuing keeps it. The cursor lives on the
//! thread row; progress (furthest ayah per surah) is per owner and never
//! regresses. A thread the reader merely opened stays an in-memory draft
//! until something is read, so bounces never litter Continue.
//!
//! Signed in, the row is the queue. Each register (identity, cursor,
//! lifecycle) is stamped only when its fields change, so a cursor move can
//! never reopen a thread and a lifecycle write can never move a cursor.
//! Native writes every change through at once (no debounce), so a kill loses
//! nothing that was written.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
};

use serde_json::{Map, Value, json};

use super::{
    AppStore, StoreError,
    position::{PageMeta, PageVerdict, Position},
    rows::{
        RowModel, RowRecord, bool_true, count_dirty, count_rejected, delete_domain, extras_of,
        in_tx, int_in, kv_json, kv_remove, kv_set_json, load_all, num_in, opt_str, save, save_diff,
        str_of,
    },
};
use crate::{
    merge::{compare_stamps, merge_thread, parse_stamp, round_ratio},
    sync::{
        RoundApplied, SyncDomain,
        clock::ms_from_iso,
        ids::{is_uuid_v7, uuid_v7_from_legacy},
        round_status::merged_dirty,
    },
};

/// Open threads go stale after this long without a touch (derived, never stored).
pub const STALE_AFTER_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Newest threads kept on device (open and dirty first); the server keeps the rest.
pub const LOCAL_THREAD_CAP: usize = 60;
pub const ORIGINS: &[&str] = &["index", "search", "bookmark", "card", "snippet", "link"];
pub const END_REASONS: &[&str] = &["completed", "restarted", "dismissed"];
pub const DEVICES: &[&str] = &["phone", "tablet", "desktop"];
pub const STATES: &[&str] = &["open", "ended", "deleted"];
pub const SYNC_PATH: &str = "/reading-session/v1/sync";
pub const ARCHIVE_PATH: &str = "/reading-session/v1/archive";
pub const MISMATCH_MESSAGE: &str = "Reading session sync account changed";
pub const ARCHIVE_PAGE: usize = 20;
const SYNC_PAGE: usize = 200;
const TAB_KEY: &str = "session.tab";
const PROGRESS_PREFIX: &str = "progress:";

/// The coarse device class on a cursor (three buckets, never a fingerprint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceKind {
    Phone,
    Tablet,
    #[default]
    Desktop,
}

impl DeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::Phone => "phone",
            DeviceKind::Tablet => "tablet",
            DeviceKind::Desktop => "desktop",
        }
    }

    /// Unknown values read as desktop, as the web decoder does.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "phone" => DeviceKind::Phone,
            "tablet" => DeviceKind::Tablet,
            _ => DeviceKind::Desktop,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadingThread {
    pub id: String,
    /// `index | search | bookmark | card | snippet | link`.
    pub origin: String,
    /// Origin detail, e.g. the home card that opened the thread.
    pub origin_ref: Option<String>,
    /// `surah:N | page:N | juz:N | hizb:N | snippet:slug | range:S:A-S:A`.
    pub scope: String,
    /// `S:A` end of a bounded scope; reading it completes the thread.
    pub bound_end: Option<String>,
    pub surah: i64,
    pub ayah: i64,
    pub local_page: Option<i64>,
    pub ratio: Option<f64>,
    pub source_id: Option<String>,
    pub device: DeviceKind,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
    pub end_reason: Option<String>,
    /// `open | ended | deleted`.
    pub state: String,
    pub device_id: Option<String>,
    /// The server's name for `device_id` (devices.md D5). Set by a pull only, never sent.
    pub device_name: Option<String>,
    pub identity_stamp: Option<String>,
    pub cursor_stamp: Option<String>,
    pub lifecycle_stamp: Option<String>,
    pub dirty: bool,
    pub rejected: bool,
    /// Fields this build does not know, kept for whoever does.
    pub extra: Map<String, Value>,
}

const THREAD_FIELDS: &[&str] = &[
    "id",
    "kind",
    "origin",
    "originRef",
    "scope",
    "boundEnd",
    "surah",
    "ayah",
    "localPage",
    "ratio",
    "sourceId",
    "device",
    "deviceId",
    "deviceName",
    "startedAt",
    "updatedAt",
    "endedAt",
    "endReason",
    "state",
    "identityStamp",
    "cursorStamp",
    "lifecycleStamp",
    "dirty",
    "rejected",
    "rev",
    "extras",
];

fn one_of(raw: Option<&str>, allowed: &[&str], fallback: &str) -> String {
    raw.filter(|v| allowed.contains(v))
        .unwrap_or(fallback)
        .to_owned()
}

/// `S:A` → coordinates within the Quran's bounds.
pub fn parse_verse_key(key: &str) -> Option<(i64, i64)> {
    let (s, a) = key.split_once(':')?;
    let ok = |p: &str| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit());
    if !ok(s) || !ok(a) {
        return None;
    }
    let (surah, ayah) = (s.parse().ok()?, a.parse().ok()?);
    ((1..=114).contains(&surah) && (1..=286).contains(&ayah)).then_some((surah, ayah))
}

fn compare_verse(a: (i64, i64), b: (i64, i64)) -> Ordering {
    a.cmp(&b)
}

/// A thread row (wire or storage). Values this build has never seen read as a
/// neutral default; the stored register is never touched by reading.
pub fn decode_thread(v: &Value) -> Option<ReadingThread> {
    let id = str_of(v, "id").filter(|id| id.len() == 36)?;
    let scope = str_of(v, "scope")?;
    let surah = int_in(v, "surah", 1, 114)?;
    let ayah = int_in(v, "ayah", 1, 286)?;
    let started_at = str_of(v, "startedAt")?;
    let updated_at = str_of(v, "updatedAt")?;
    let ended_at = opt_str(v, "endedAt");
    let state = match str_of(v, "state").filter(|s| STATES.contains(s)) {
        Some(state) => state.to_owned(),
        None if ended_at.is_some() => "ended".to_owned(),
        None => "open".to_owned(),
    };
    let mut extra = v
        .get("extras")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    extra.extend(extras_of(v, THREAD_FIELDS));
    Some(ReadingThread {
        id: id.to_owned(),
        origin: one_of(str_of(v, "origin"), ORIGINS, "link"),
        origin_ref: opt_str(v, "originRef"),
        scope: scope.to_owned(),
        bound_end: opt_str(v, "boundEnd").filter(|k| parse_verse_key(k).is_some()),
        surah,
        ayah,
        local_page: int_in(v, "localPage", 1, 10_000),
        ratio: num_in(v, "ratio", 0.0, 1.0),
        source_id: opt_str(v, "sourceId"),
        device: DeviceKind::parse(str_of(v, "device").unwrap_or_default()),
        started_at: started_at.to_owned(),
        updated_at: updated_at.to_owned(),
        end_reason: ended_at
            .as_ref()
            .map(|_| one_of(str_of(v, "endReason"), END_REASONS, "dismissed")),
        ended_at,
        state,
        device_id: opt_str(v, "deviceId"),
        device_name: opt_str(v, "deviceName"),
        identity_stamp: opt_str(v, "identityStamp"),
        cursor_stamp: opt_str(v, "cursorStamp"),
        lifecycle_stamp: opt_str(v, "lifecycleStamp"),
        dirty: bool_true(v, "dirty"),
        rejected: bool_true(v, "rejected"),
        extra,
    })
}

fn thread_fields(t: &ReadingThread) -> Value {
    json!({
        "id": t.id, "origin": t.origin, "originRef": t.origin_ref, "scope": t.scope,
        "boundEnd": t.bound_end, "surah": t.surah, "ayah": t.ayah, "localPage": t.local_page,
        "ratio": t.ratio, "sourceId": t.source_id, "device": t.device.as_str(),
        "startedAt": t.started_at, "updatedAt": t.updated_at, "endedAt": t.ended_at,
        "endReason": t.end_reason, "state": t.state, "deviceId": t.device_id,
        "identityStamp": t.identity_stamp, "cursorStamp": t.cursor_stamp,
        "lifecycleStamp": t.lifecycle_stamp,
    })
}

fn stamps_of(t: &ReadingThread) -> Vec<Option<String>> {
    vec![
        t.identity_stamp.clone(),
        t.cursor_stamp.clone(),
        t.lifecycle_stamp.clone(),
    ]
}

impl RowModel for ReadingThread {
    const DOMAIN: &'static str = "thread";
    fn id(&self) -> &str {
        &self.id
    }
    fn decode(record: &RowRecord) -> Option<Self> {
        decode_thread(&record.row).map(|t| ReadingThread {
            dirty: record.dirty,
            rejected: record.rejected,
            ..t
        })
    }
    fn encode(&self) -> RowRecord {
        let mut row = Value::Object(self.extra.clone());
        if let (Some(o), Some(fields)) = (row.as_object_mut(), thread_fields(self).as_object()) {
            o.extend(fields.clone());
            o.insert("deviceName".into(), json!(self.device_name));
        }
        RowRecord {
            id: self.id.clone(),
            row,
            stamps: stamps_of(self),
            dirty: self.dirty,
            rejected: self.rejected,
        }
    }
}

/// Furthest ayah reached in one surah (account-level, MAX-merged).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurahProgress {
    pub surah: i64,
    pub furthest_ayah: i64,
    pub updated_at: String,
    pub dirty: bool,
}

fn decode_progress(v: &Value) -> Option<SurahProgress> {
    Some(SurahProgress {
        surah: int_in(v, "surah", 1, 114)?,
        furthest_ayah: int_in(v, "furthestAyah", 1, 286)?,
        updated_at: str_of(v, "updatedAt")?.to_owned(),
        dirty: bool_true(v, "dirty"),
    })
}

/// Progress rows are keyed by surah; `id()` needs a borrowed str, so the
/// model wraps the row with its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProgressRow {
    key: String,
    row: SurahProgress,
}

impl ProgressRow {
    fn of(row: SurahProgress) -> Self {
        Self {
            key: row.surah.to_string(),
            row,
        }
    }
}

impl RowModel for ProgressRow {
    const DOMAIN: &'static str = "progress";
    fn id(&self) -> &str {
        &self.key
    }
    fn decode(record: &RowRecord) -> Option<Self> {
        decode_progress(&record.row).map(|row| {
            ProgressRow::of(SurahProgress {
                dirty: record.dirty,
                ..row
            })
        })
    }
    fn encode(&self) -> RowRecord {
        RowRecord {
            id: self.key.clone(),
            row: json!({
                "surah": self.row.surah, "furthestAyah": self.row.furthest_ayah,
                "updatedAt": self.row.updated_at,
            }),
            stamps: Vec::new(),
            dirty: self.row.dirty,
            rejected: false,
        }
    }
}

/// When the cursor last moved. Signed-out rows have no stamp: their own time.
pub fn touched_at(t: &ReadingThread) -> Option<i64> {
    match t.cursor_stamp.as_deref().and_then(parse_stamp) {
        Some(stamp) => Some(stamp.ms as i64),
        None => ms_from_iso(&t.updated_at),
    }
}

/// Open = not ended and touched within [`STALE_AFTER_MS`].
pub fn is_open(t: &ReadingThread, now_ms: i64) -> bool {
    t.state == "open"
        && t.ended_at.is_none()
        && touched_at(t).is_some_and(|touched| now_ms - touched < STALE_AFTER_MS)
}

/// Newest first, ties by id: the server's order, so every device converges.
pub fn by_newest(a: &ReadingThread, b: &ReadingThread) -> Ordering {
    let (left, right) = (
        touched_at(a).unwrap_or(i64::MIN),
        touched_at(b).unwrap_or(i64::MIN),
    );
    right.cmp(&left).then_with(|| b.id.cmp(&a.id))
}

fn registers_of(t: &ReadingThread) -> Value {
    json!({
        "id": t.id,
        "identity": { "stamp": t.identity_stamp, "value": {
            "origin": t.origin, "originRef": t.origin_ref, "scope": t.scope,
            "boundEnd": t.bound_end, "startedAt": t.started_at,
        }},
        "cursor": { "stamp": t.cursor_stamp, "value": {
            "surah": t.surah, "ayah": t.ayah, "localPage": t.local_page, "ratio": t.ratio,
            "sourceId": t.source_id, "device": t.device.as_str(), "deviceId": t.device_id,
        }},
        "lifecycle": { "stamp": t.lifecycle_stamp, "value": {
            "state": t.state, "endedAt": t.ended_at, "endReason": t.end_reason,
        }},
    })
}

/// The register merge on the local row shape; `merged_dirty` holds the flag rule.
pub(crate) fn merge_row(a: &ReadingThread, b: &ReadingThread) -> ReadingThread {
    // Signed-out rows carry no stamps yet: their own time decides.
    if a.cursor_stamp.is_none() && b.cursor_stamp.is_none() {
        return if b.updated_at > a.updated_at {
            b.clone()
        } else {
            a.clone()
        };
    }
    let merged = merge_thread(&registers_of(a), &registers_of(b));
    let group = |k: &str| merged[k].get("value").cloned().unwrap_or(Value::Null);
    let stamp = |k: &str| opt_str(&merged[k], "stamp");
    let mut flat = json!({ "id": a.id, "updatedAt": "1970-01-01T00:00:00.000Z" });
    for k in ["identity", "cursor", "lifecycle"] {
        if let (Some(out), Some(fields)) = (flat.as_object_mut(), group(k).as_object()) {
            out.extend(fields.clone());
        }
    }
    let Some(decoded) = decode_thread(&flat) else {
        return a.clone();
    };
    let merged_stamps = [stamp("identity"), stamp("cursor"), stamp("lifecycle")];
    let mut extra = a.extra.clone();
    extra.extend(b.extra.clone());
    ReadingThread {
        updated_at: a.updated_at.clone().max(b.updated_at.clone()),
        dirty: merged_dirty(
            (a.dirty, &stamps_of(a)),
            (b.dirty, &stamps_of(b)),
            &merged_stamps,
        ),
        rejected: a.rejected && b.rejected,
        identity_stamp: merged_stamps[0].clone(),
        cursor_stamp: merged_stamps[1].clone(),
        lifecycle_stamp: merged_stamps[2].clone(),
        device_name: a.device_name.clone(),
        extra,
        ..decoded
    }
}

/// Union by id through the merge core. Open threads are kept before the cap
/// trims history, and a dirty row is never evicted: it is the queue.
pub(crate) fn merge_threads(
    a: &[ReadingThread],
    b: &[ReadingThread],
    now: i64,
) -> Vec<ReadingThread> {
    let mut by_id: BTreeMap<String, ReadingThread> = BTreeMap::new();
    for row in a.iter().chain(b) {
        let merged = match by_id.get(&row.id) {
            Some(prev) => merge_row(prev, row),
            None => row.clone(),
        };
        by_id.insert(row.id.clone(), merged);
    }
    let mut all: Vec<ReadingThread> = by_id.into_values().collect();
    all.sort_by(by_newest);
    let (pinned, rest): (Vec<_>, Vec<_>) =
        all.into_iter().partition(|t| t.dirty || is_open(t, now));
    let keep = LOCAL_THREAD_CAP.max(pinned.len());
    let mut out: Vec<ReadingThread> = pinned.into_iter().chain(rest).take(keep).collect();
    out.sort_by(by_newest);
    out
}

// --- the reader tab ---

/// What a reader route shows: the reader hands this to [`enter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderTarget {
    /// The route's natural scope: `surah:73`, `page:574`, `juz:30`, `hizb:60`, `snippet:slug`.
    pub scope: String,
    /// First verse on screen, or the explicit ayah the route names.
    pub surah: i64,
    pub ayah: i64,
    /// The route names an exact ayah.
    pub exact: bool,
    pub bound_end: Option<String>,
    pub source_id: Option<String>,
}

/// The intent a navigation into the reader carries (web `entry.ts`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryIntent {
    /// An origin of [`ORIGINS`], or `resume`.
    pub origin: String,
    pub reference: Option<String>,
    /// Overrides the reader's natural scope (card ranges, snippet sets).
    pub scope: Option<String>,
    pub bound_end: Option<String>,
    /// The target names an exact ayah (search hit, bookmark): always a new thread.
    pub exact: bool,
    /// `resume` only: the thread to continue.
    pub thread_id: Option<String>,
}

impl EntryIntent {
    /// An explicit resume of `id` (Continue, History).
    pub fn resume(id: impl Into<String>) -> Self {
        Self {
            origin: "resume".into(),
            thread_id: Some(id.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnterResult {
    pub thread: ReadingThread,
    /// Picked up an existing thread whose cursor is elsewhere: move the reader there.
    pub resumed: bool,
}

/// The app's reader tab: its thread (a draft until something is read) and
/// the "Start over" offer. In memory, with the tab's id and scope persisted
/// per owner so a restart continues the thread.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tab {
    id: Option<String>,
    scope: Option<String>,
    draft: Option<ReadingThread>,
    offer: Option<String>,
    device: DeviceKind,
}

impl Tab {
    pub(crate) fn load(store: &AppStore) -> Tab {
        let saved = kv_json(store.owner_conn(), TAB_KEY).ok().flatten();
        let field = |k: &str| saved.as_ref().and_then(|v| opt_str(v, k));
        Tab {
            id: field("id"),
            scope: field("scope"),
            draft: None,
            offer: None,
            device: store.tab.device,
        }
    }
}

/// This install's device class, set once by the app from its platform.
pub fn set_device_kind(store: &mut AppStore, kind: DeviceKind) {
    store.tab.device = kind;
}

fn set_tab(store: &mut AppStore, id: &str, scope: &str) -> Result<(), StoreError> {
    store.tab.id = Some(id.to_owned());
    store.tab.scope = Some(scope.to_owned());
    let result = kv_set_json(
        store.writable()?,
        TAB_KEY,
        &json!({ "id": id, "scope": scope }),
    );
    store.track(result)
}

fn clear_tab(store: &mut AppStore) -> Result<(), StoreError> {
    store.tab.id = None;
    store.tab.scope = None;
    store.tab.draft = None;
    let result = kv_remove(store.writable()?, TAB_KEY);
    store.track(result)
}

// --- reads ---

fn all_threads(store: &AppStore) -> Result<Vec<ReadingThread>, StoreError> {
    let mut threads: Vec<ReadingThread> = load_all(store.owner_conn())?;
    threads.sort_by(by_newest);
    Ok(threads)
}

/// Every thread this device holds for the current owner, ended and deleted
/// ones included, newest first. Export reads these.
pub fn all(store: &AppStore) -> Result<Vec<ReadingThread>, StoreError> {
    all_threads(store)
}

/// Open threads (not ended, touched within 30 days), newest first.
pub fn open(store: &AppStore) -> Result<Vec<ReadingThread>, StoreError> {
    let now = store.now();
    Ok(all_threads(store)?
        .into_iter()
        .filter(|t| is_open(t, now))
        .collect())
}

/// Newest open thread: what "Continue reading" resumes.
pub fn latest(store: &AppStore) -> Result<Option<ReadingThread>, StoreError> {
    Ok(open(store)?.into_iter().next())
}

/// This tab's thread (a draft until something is read).
pub fn current(store: &AppStore) -> Result<Option<ReadingThread>, StoreError> {
    let Some(id) = store.tab.id.clone() else {
        return Ok(None);
    };
    if let Some(draft) = store.tab.draft.as_ref().filter(|d| d.id == id) {
        return Ok(Some(draft.clone()));
    }
    find_stored(store, &id)
}

fn find_stored(store: &AppStore, id: &str) -> Result<Option<ReadingThread>, StoreError> {
    Ok(
        super::rows::get(store.owner_conn(), ReadingThread::DOMAIN, id)?
            .and_then(|r| ReadingThread::decode(&r)),
    )
}

/// Thread id the reader may offer "Start over" for.
pub fn offer(store: &AppStore) -> Option<String> {
    store.tab.offer.clone()
}

/// Furthest ayah reached in a surah (account-level, never regresses).
pub fn furthest_in(store: &AppStore, surah: i64) -> Result<Option<i64>, StoreError> {
    Ok(
        super::rows::get(store.owner_conn(), ProgressRow::DOMAIN, &surah.to_string())?
            .and_then(|r| ProgressRow::decode(&r))
            .map(|p| p.row.furthest_ayah),
    )
}

/// Every surah's progress.
pub fn progress(store: &AppStore) -> Result<Vec<SurahProgress>, StoreError> {
    Ok(load_all::<ProgressRow>(store.owner_conn())?
        .into_iter()
        .map(|p| p.row)
        .collect())
}

pub fn pending(store: &AppStore) -> Result<usize, StoreError> {
    if store.account_id().is_none() {
        return Ok(0);
    }
    let conn = store.owner_conn();
    Ok(count_dirty(conn, ReadingThread::DOMAIN)? + count_dirty(conn, ProgressRow::DOMAIN)?)
}

pub fn rejected_count(store: &AppStore) -> Result<usize, StoreError> {
    count_rejected(store.owner_conn(), ReadingThread::DOMAIN)
}

pub fn capped_count(store: &AppStore) -> usize {
    store
        .sync_memory
        .capped
        .get(&SyncDomain::Sessions)
        .map_or(0, HashSet::len)
}

// --- handoff (devices.md D5) ---

/// This install, as the handoff label compares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HereDevice {
    pub device_id: Option<String>,
    pub kind: DeviceKind,
}

/// Where a thread was last moved, from this install's point of view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffPlace {
    Here,
    /// The device's own name, as the server resolved it.
    Named(String),
    /// "on your {kind}".
    Device(DeviceKind),
}

/// Two phones are two devices: compare ids. A row from before device ids compares the kind.
pub fn is_thread_here(thread: &ReadingThread, here: &HereDevice) -> bool {
    match (&thread.device_id, &here.device_id) {
        (Some(a), Some(b)) => a == b,
        _ => thread.device == here.kind,
    }
}

/// The label for a thread: this device (no label), its name, else its kind.
pub fn handoff_place(thread: &ReadingThread, here: &HereDevice) -> HandoffPlace {
    if is_thread_here(thread, here) {
        return HandoffPlace::Here;
    }
    match &thread.device_name {
        Some(name) => HandoffPlace::Named(name.clone()),
        None => HandoffPlace::Device(thread.device),
    }
}

/// [`HereDevice`] for the current owner.
pub fn here(store: &AppStore) -> HereDevice {
    HereDevice {
        device_id: store.device_id().map(str::to_owned),
        kind: store.tab.device,
    }
}

// --- writes ---

/// Commit a thread row: drafts materialize here, the first time anything is read.
fn write(store: &mut AppStore, thread: ReadingThread) -> Result<ReadingThread, StoreError> {
    store.writable()?;
    if store.tab.draft.as_ref().is_some_and(|d| d.id == thread.id) {
        store.tab.draft = None;
    }
    let before = all_threads(store)?;
    let prev = before.iter().find(|t| t.id == thread.id);
    let lifecycle_moved = prev.is_none_or(|p| {
        p.state != thread.state
            || p.ended_at != thread.ended_at
            || p.end_reason != thread.end_reason
    });
    let row = stamped(store, thread, prev.cloned(), lifecycle_moved)?;
    let others: Vec<ReadingThread> = before.iter().filter(|t| t.id != row.id).cloned().collect();
    let after = merge_threads(&others, std::slice::from_ref(&row), store.now());
    let result = in_tx(store.owner_conn(), |tx| save_diff(tx, &before, &after));
    store.track(result)?;
    Ok(row)
}

fn stamped(
    store: &mut AppStore,
    thread: ReadingThread,
    prev: Option<ReadingThread>,
    lifecycle_moved: bool,
) -> Result<ReadingThread, StoreError> {
    if store.account_id().is_none() {
        return Ok(thread);
    }
    let cursor_moved = prev.as_ref().is_none_or(|p| {
        p.surah != thread.surah
            || p.ayah != thread.ayah
            || p.local_page != thread.local_page
            || p.ratio != thread.ratio
            || p.source_id != thread.source_id
            || p.device != thread.device
    });
    let here = store.device_id().map(str::to_owned);
    // A cursor move names the install that made it; lifecycle-only writes keep the last mover.
    let device_id = match cursor_moved {
        true => here.clone().or(thread.device_id.clone()),
        false => thread.device_id.clone().or(here),
    };
    let identity_stamp = match thread.identity_stamp.clone() {
        Some(stamp) => Some(stamp),
        None => store.stamp()?,
    };
    let cursor_stamp = match (cursor_moved, thread.cursor_stamp.clone()) {
        (false, Some(stamp)) => Some(stamp),
        _ => store.stamp()?,
    };
    let lifecycle_stamp = match (lifecycle_moved, thread.lifecycle_stamp.clone()) {
        (false, Some(stamp)) => Some(stamp),
        _ => store.stamp()?,
    };
    Ok(ReadingThread {
        device_id,
        identity_stamp,
        cursor_stamp,
        lifecycle_stamp,
        dirty: true,
        rejected: false,
        ..thread
    })
}

fn bump_progress(store: &mut AppStore, surah: i64, ayah: i64) -> Result<(), StoreError> {
    if furthest_in(store, surah)?.is_some_and(|f| f >= ayah) {
        return Ok(());
    }
    let row = ProgressRow::of(SurahProgress {
        surah,
        furthest_ayah: ayah,
        updated_at: store.now_iso(),
        dirty: store.account_id().is_some(),
    });
    let result = save(store.writable()?, &row);
    store.track(result)
}

struct NewThread {
    origin: String,
    origin_ref: Option<String>,
    scope: String,
    bound_end: Option<String>,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
}

fn new_thread(store: &mut AppStore, fields: NewThread) -> ReadingThread {
    let now = store.now_iso();
    ReadingThread {
        id: store.new_id(),
        origin: fields.origin,
        origin_ref: fields.origin_ref,
        scope: fields.scope,
        bound_end: fields.bound_end,
        surah: fields.surah,
        ayah: fields.ayah,
        local_page: None,
        ratio: None,
        source_id: fields.source_id,
        device: store.tab.device,
        started_at: now.clone(),
        updated_at: now,
        ended_at: None,
        end_reason: None,
        state: "open".into(),
        device_id: None,
        device_name: None,
        identity_stamp: None,
        cursor_stamp: None,
        lifecycle_stamp: None,
        dirty: false,
        rejected: false,
        extra: Map::new(),
    }
}

/// A `range:S:A-S:B` scope bounds itself at `S:B`.
fn bound_from_scope(scope: &str) -> Option<String> {
    let (_, end) = scope.strip_prefix("range:")?.split_once('-')?;
    parse_verse_key(end).map(|_| end.to_owned())
}

/// Start verse of a `range:` scope.
fn range_start(scope: &str) -> Option<(i64, i64)> {
    let (start, _) = scope.strip_prefix("range:")?.split_once('-')?;
    parse_verse_key(start)
}

fn open_in_scope(
    store: &AppStore,
    scope: &str,
    surah: i64,
) -> Result<Option<ReadingThread>, StoreError> {
    let surah_scope = scope.starts_with("surah:");
    Ok(open(store)?.into_iter().find(|t| {
        t.scope == scope || (surah_scope && t.scope.starts_with("surah:") && t.surah == surah)
    }))
}

fn reopen(store: &mut AppStore, thread: ReadingThread) -> Result<ReadingThread, StoreError> {
    let updated_at = store.now_iso();
    write(
        store,
        ReadingThread {
            state: "open".into(),
            ended_at: None,
            end_reason: None,
            updated_at,
            ..thread
        },
    )
}

fn end(store: &mut AppStore, thread: ReadingThread, reason: &str) -> Result<(), StoreError> {
    let now = store.now_iso();
    let id = thread.id.clone();
    write(
        store,
        ReadingThread {
            state: "ended".into(),
            ended_at: Some(now.clone()),
            end_reason: Some(reason.into()),
            updated_at: now,
            ..thread
        },
    )?;
    if store.tab.offer.as_deref() == Some(id.as_str()) {
        store.tab.offer = None;
    }
    Ok(())
}

/// Resolve the tab's thread for a reader route: explicit resume → that
/// thread; untagged continuation → keep; exact-ayah, card and snippet entries
/// → new; whole-scope entries → resume the open thread in that scope (with a
/// "Start over" offer), else new. `came_from_reader`: the page being left was
/// a reader page.
pub fn enter(
    store: &mut AppStore,
    target: &ReaderTarget,
    intent: Option<EntryIntent>,
    came_from_reader: bool,
) -> Result<EnterResult, StoreError> {
    store.tab.offer = None;
    if let Some(intent) = intent.as_ref().filter(|i| i.origin == "resume") {
        let found = match intent.thread_id.as_deref() {
            Some(id) => find_stored(store, id)?,
            None => None,
        };
        if let Some(thread) = found {
            let resumed = match thread.ended_at {
                None => thread,
                Some(_) => reopen(store, thread)?,
            };
            set_tab(store, &resumed.id, &target.scope)?;
            return Ok(EnterResult {
                thread: resumed,
                resumed: false,
            });
        }
    }
    if let Some(tab) = current(store)? {
        let is_draft = store.tab.draft.as_ref().is_some_and(|d| d.id == tab.id);
        let alive = is_draft || is_open(&tab, store.now());
        let continuing =
            store.tab.scope.as_deref() == Some(target.scope.as_str()) || came_from_reader;
        if intent.is_none() && alive && continuing {
            set_tab(store, &tab.id, &target.scope)?;
            return Ok(EnterResult {
                thread: tab,
                resumed: false,
            });
        }
    }
    let origin = intent
        .as_ref()
        .map(|i| i.origin.as_str())
        .filter(|o| ORIGINS.contains(o))
        .unwrap_or("link")
        .to_owned();
    let scope = intent
        .as_ref()
        .and_then(|i| i.scope.clone())
        .unwrap_or_else(|| target.scope.clone());
    let exact = target.exact || intent.as_ref().is_some_and(|i| i.exact);
    if !exact
        && origin != "card"
        && origin != "snippet"
        && let Some(existing) = open_in_scope(store, &scope, target.surah)?
    {
        set_tab(store, &existing.id, &target.scope)?;
        let at_start = existing.surah == target.surah && existing.ayah == target.ayah;
        if !at_start {
            store.tab.offer = Some(existing.id.clone());
        }
        return Ok(EnterResult {
            thread: existing,
            resumed: !at_start,
        });
    }
    let start = if exact { None } else { range_start(&scope) };
    let bound_end = intent
        .as_ref()
        .and_then(|i| i.bound_end.clone())
        .or_else(|| target.bound_end.clone())
        .or_else(|| bound_from_scope(&scope));
    let draft = new_thread(
        store,
        NewThread {
            origin,
            origin_ref: intent.as_ref().and_then(|i| i.reference.clone()),
            bound_end,
            surah: start.map_or(target.surah, |s| s.0),
            ayah: start.map_or(target.ayah, |s| s.1),
            scope,
            source_id: target.source_id.clone(),
        },
    );
    store.tab.draft = Some(draft.clone());
    set_tab(store, &draft.id, &target.scope)?;
    Ok(EnterResult {
        thread: draft,
        resumed: false,
    })
}

fn implicit_thread(
    store: &mut AppStore,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
) -> Result<ReadingThread, StoreError> {
    let draft = new_thread(
        store,
        NewThread {
            origin: "link".into(),
            origin_ref: None,
            scope: format!("surah:{surah}"),
            bound_end: None,
            surah,
            ayah,
            source_id,
        },
    );
    store.tab.draft = Some(draft.clone());
    set_tab(store, &draft.id, &draft.scope)?;
    Ok(draft)
}

/// The reader's position moved (topmost verse on screen). An unmoved cursor
/// is no evidence of reading: a draft stays a draft.
pub fn cursor(
    store: &mut AppStore,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
) -> Result<(), StoreError> {
    let thread = match current(store)? {
        Some(thread) => thread,
        None => implicit_thread(store, surah, ayah, source_id.clone())?,
    };
    let moved = thread.surah != surah || thread.ayah != ayah || thread.source_id != source_id;
    if !moved {
        return Ok(());
    }
    let updated_at = store.now_iso();
    let device = store.tab.device;
    write(
        store,
        ReadingThread {
            surah,
            ayah,
            source_id,
            local_page: None,
            ratio: None,
            device,
            updated_at,
            ..thread
        },
    )?;
    bump_progress(store, surah, ayah)
}

/// Fine position inside the cursor's local page (precise handoff restore).
pub fn anchor(store: &mut AppStore, local_page: i64, ratio: f64) -> Result<(), StoreError> {
    let Some(thread) = current(store)? else {
        return Ok(());
    };
    if store.tab.draft.as_ref().is_some_and(|d| d.id == thread.id) {
        return Ok(());
    }
    let rounded = round_ratio(ratio);
    if thread.local_page == Some(local_page) && thread.ratio == Some(rounded) {
        return Ok(());
    }
    let updated_at = store.now_iso();
    let device = store.tab.device;
    write(
        store,
        ReadingThread {
            local_page: Some(local_page),
            ratio: Some(rounded),
            device,
            updated_at,
            ..thread
        },
    )?;
    Ok(())
}

/// A verse was actually read: a draft becomes real, a read past the cursor
/// moves it there, and a bounded thread completes at its end.
pub fn note_read(store: &mut AppStore, surah: i64, ayah: i64) -> Result<(), StoreError> {
    let Some(mut thread) = current(store)? else {
        return Ok(());
    };
    if thread.ended_at.is_some() {
        return Ok(());
    }
    let is_draft = store.tab.draft.as_ref().is_some_and(|d| d.id == thread.id);
    let ahead = compare_verse((surah, ayah), (thread.surah, thread.ayah)).is_gt();
    if is_draft || ahead {
        let updated_at = store.now_iso();
        let device = store.tab.device;
        let moved = ReadingThread {
            surah: if ahead { surah } else { thread.surah },
            ayah: if ahead { ayah } else { thread.ayah },
            local_page: if ahead { None } else { thread.local_page },
            ratio: if ahead { None } else { thread.ratio },
            device,
            updated_at,
            ..thread
        };
        thread = write(store, moved)?;
    }
    bump_progress(store, surah, ayah)?;
    let Some(end_at) = thread.bound_end.as_deref().and_then(parse_verse_key) else {
        return Ok(());
    };
    if compare_verse((surah, ayah), end_at).is_lt() {
        return Ok(());
    }
    end(store, thread, "completed")
}

/// "Start over": retire the resumed thread and begin a fresh draft at the scope start.
pub fn start_over(store: &mut AppStore) -> Result<Option<ReadingThread>, StoreError> {
    let Some(thread) = current(store)? else {
        return Ok(None);
    };
    store.tab.offer = None;
    let is_draft = store.tab.draft.as_ref().is_some_and(|d| d.id == thread.id);
    if !is_draft {
        end(store, thread.clone(), "restarted")?;
    }
    let start = range_start(&thread.scope);
    let draft = new_thread(
        store,
        NewThread {
            origin: thread.origin.clone(),
            origin_ref: thread.origin_ref.clone(),
            scope: thread.scope.clone(),
            bound_end: thread.bound_end.clone(),
            surah: start.map_or(thread.surah, |s| s.0),
            ayah: start.map_or(1, |s| s.1),
            source_id: thread.source_id.clone(),
        },
    );
    store.tab.draft = Some(draft.clone());
    let scope = store.tab.scope.clone().unwrap_or(thread.scope);
    set_tab(store, &draft.id, &scope)?;
    Ok(Some(draft))
}

pub fn dismiss_offer(store: &mut AppStore) {
    store.tab.offer = None;
}

/// Remove from Continue (kept in history).
pub fn dismiss(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    match find_stored(store, id)? {
        Some(thread) if thread.ended_at.is_none() => end(store, thread, "dismissed"),
        _ => Ok(()),
    }
}

/// Delete from history everywhere. Signed in, the row stays as a tombstone.
/// `archived` is the thread when it came from a server history page.
pub fn remove(
    store: &mut AppStore,
    id: &str,
    archived: Option<&ReadingThread>,
) -> Result<(), StoreError> {
    store.writable()?;
    if store.tab.id.as_deref() == Some(id) {
        clear_tab(store)?;
    }
    if store.account_id().is_none() {
        let result = super::rows::delete(store.owner_conn(), ReadingThread::DOMAIN, id);
        return store.track(result);
    }
    let thread = match find_stored(store, id)? {
        Some(thread) => Some(thread),
        None => archived.filter(|t| t.id == id).cloned(),
    };
    let Some(thread) = thread else {
        return Ok(());
    };
    let updated_at = store.now_iso();
    write(
        store,
        ReadingThread {
            state: "deleted".into(),
            updated_at,
            ..thread
        },
    )?;
    Ok(())
}

/// Threads cleared by [`clear_history`], kept so Undo can put them back.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryClear {
    pub owner: Option<i64>,
    pub threads: Vec<ReadingThread>,
}

/// Clear-all history: tombstone every ended (or stale) thread. `whole_archive`
/// is every server history page (`None` when it could not be read whole:
/// then nothing is cleared). Signed out, pass `Some(&[])`.
pub fn clear_history(
    store: &mut AppStore,
    whole_archive: Option<&[ReadingThread]>,
) -> Result<Option<HistoryClear>, StoreError> {
    let Some(archive) = whole_archive else {
        return Ok(None);
    };
    let now = store.now();
    let mut ended: Vec<ReadingThread> = Vec::new();
    // Local rows first: they carry this device's newest copy.
    for thread in all_threads(store)?
        .into_iter()
        .chain(archive.iter().cloned())
    {
        if thread.state == "deleted"
            || is_open(&thread, now)
            || ended.iter().any(|t| t.id == thread.id)
        {
            continue;
        }
        ended.push(thread);
    }
    for thread in &ended {
        if store.account_id().is_none() {
            remove(store, &thread.id, None)?;
            continue;
        }
        if store.tab.id.as_deref() == Some(thread.id.as_str()) {
            clear_tab(store)?;
        }
        let updated_at = store.now_iso();
        write(
            store,
            ReadingThread {
                state: "deleted".into(),
                updated_at,
                ..thread.clone()
            },
        )?;
    }
    Ok(Some(HistoryClear {
        owner: store.account_id(),
        threads: ended,
    }))
}

/// Undo for [`clear_history`]: each thread back with a fresh lifecycle stamp.
/// Ignored once another owner holds the store.
pub fn restore_history(store: &mut AppStore, clear: &HistoryClear) -> Result<(), StoreError> {
    if clear.owner != store.account_id() {
        return Ok(());
    }
    for thread in &clear.threads {
        // A signed-out row merges by `updatedAt`: newer than the tombstone it replaces.
        let tombstone_at = find_stored(store, &thread.id)?
            .and_then(|t| ms_from_iso(&t.updated_at))
            .map_or(0, |ms| ms + 1);
        let updated_at = crate::sync::clock::iso_from_ms(store.local_now().max(tombstone_at));
        write(
            store,
            ReadingThread {
                updated_at,
                ..thread.clone()
            },
        )?;
    }
    Ok(())
}

/// Import (F6). Threads whose ids this device lacks are added through the
/// normal write (fresh stamps, `dirty`, this device's id). A non-v7 id is
/// re-keyed from `startedAt` so the server accepts it. Returns how many were added.
pub fn import_threads(
    store: &mut AppStore,
    threads: &[ReadingThread],
) -> Result<usize, StoreError> {
    let mut known: HashSet<String> = all_threads(store)?.into_iter().map(|t| t.id).collect();
    let mut added = 0;
    for thread in threads {
        if thread.state == "deleted" {
            continue;
        }
        let id = if is_uuid_v7(&thread.id) {
            thread.id.clone()
        } else {
            uuid_v7_from_legacy(&thread.id, ms_from_iso(&thread.started_at).unwrap_or(0))
        };
        if !known.insert(id.clone()) {
            continue;
        }
        write(
            store,
            ReadingThread {
                id,
                device_id: None,
                device_name: None,
                identity_stamp: None,
                cursor_stamp: None,
                lifecycle_stamp: None,
                dirty: false,
                rejected: false,
                ..thread.clone()
            },
        )?;
        added += 1;
    }
    Ok(added)
}

// --- history pages ---

/// One `GET /reading-session/v1/archive` page.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchivePage {
    pub sessions: Vec<ReadingThread>,
    pub next: Option<String>,
}

/// The archive path for a page before `cursor`.
pub fn archive_path(before: Option<&str>, limit: usize) -> String {
    let mut path = format!("{ARCHIVE_PATH}?limit={}", limit.clamp(1, 50));
    if let Some(cursor) = before {
        path.push_str("&before=");
        for b in cursor.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                path.push(char::from(b));
            } else {
                path.push_str(&format!("%{b:02X}"));
            }
        }
    }
    path
}

pub fn decode_archive_page(v: &Value) -> Option<ArchivePage> {
    let rows = v.get("sessions")?.as_array()?;
    Some(ArchivePage {
        sessions: rows.iter().filter_map(decode_thread).collect(),
        next: opt_str(v, "next"),
    })
}

/// History signed out: ended and stale threads held here, newest first.
pub fn local_history(store: &AppStore) -> Result<Vec<ReadingThread>, StoreError> {
    let now = store.now();
    Ok(all_threads(store)?
        .into_iter()
        .filter(|t| t.state != "deleted" && !is_open(t, now))
        .collect())
}

// --- sync ---

fn wire_of(t: &ReadingThread) -> Value {
    // deviceName is the server's label for a pulled row, never sent back.
    let mut wire = Value::Object(t.extra.clone());
    if let (Some(out), Some(fields)) = (wire.as_object_mut(), thread_fields(t).as_object()) {
        out.insert("kind".into(), json!("session.upsert"));
        out.extend(fields.clone());
    }
    wire
}

/// Rows the next round sends. A capped row waits behind the others.
pub fn dirty_rows(store: &AppStore) -> Result<Vec<Value>, StoreError> {
    let capped = store
        .sync_memory
        .capped
        .get(&SyncDomain::Sessions)
        .cloned()
        .unwrap_or_default();
    let dirty: Vec<ReadingThread> = all_threads(store)?
        .into_iter()
        .filter(|t| t.dirty && !t.rejected && t.id.len() == 36)
        .collect();
    let (waiting, ready): (Vec<_>, Vec<_>) =
        dirty.into_iter().partition(|t| capped.contains(&t.id));
    let mut out: Vec<Value> = ready.iter().chain(&waiting).map(wire_of).collect();
    out.extend(progress(store)?.into_iter().filter(|p| p.dirty).map(|p| {
        json!({
            "kind": "progress.upsert", "surah": p.surah,
            "furthestAyah": p.furthest_ayah, "updatedAt": p.updated_at,
        })
    }));
    out.truncate(SYNC_PAGE);
    Ok(out)
}

fn apply_result(
    result: &Value,
    sent: Option<&Value>,
    capped: &mut HashSet<String>,
    by_id: &mut BTreeMap<String, ReadingThread>,
    gone: &mut Vec<String>,
) {
    let (Some(id), Some(sent)) = (str_of(result, "id"), sent) else {
        return;
    };
    let Some(mine) = by_id.get_mut(id) else {
        return;
    };
    let unchanged = mine.identity_stamp.as_deref() == str_of(sent, "identityStamp")
        && mine.cursor_stamp.as_deref() == str_of(sent, "cursorStamp")
        && mine.lifecycle_stamp.as_deref() == str_of(sent, "lifecycleStamp");
    capped.remove(id);
    let status = str_of(result, "status").unwrap_or_default();
    if status == "capped" {
        capped.insert(id.to_owned());
        return;
    }
    if !unchanged {
        return;
    }
    if status == "rejected" {
        mine.rejected = true;
        return;
    }
    let Some(stamps) = result.get("stamps").filter(|s| s.is_object()) else {
        // Older than the tombstone floor: it may be long deleted. Let it go.
        gone.push(id.to_owned());
        return;
    };
    let adopt = |k: &str, local: &Option<String>| match (str_of(stamps, k), local) {
        (Some(r), Some(l)) if r < l.as_str() => Some(r.to_owned()),
        _ => local.clone(),
    };
    mine.identity_stamp = adopt("identity", &mine.identity_stamp);
    mine.cursor_stamp = adopt("cursor", &mine.cursor_stamp);
    mine.lifecycle_stamp = adopt("lifecycle", &mine.lifecycle_stamp);
    mine.dirty = false;
    mine.rejected = false;
}

/// Tombstones stay as clean local rows until the cap trims them, so a stale
/// copy can never bring a deleted thread back.
fn gone_locally(store: &mut AppStore, thread: &mut ReadingThread) -> Result<(), StoreError> {
    thread.state = "deleted".into();
    thread.lifecycle_stamp = store.stamp()?.or(thread.lifecycle_stamp.take());
    thread.dirty = false;
    Ok(())
}

/// Apply one round durably (contracts/sync.md; web `SessionsStore.applyRound`).
pub(crate) fn apply_round(
    store: &mut AppStore,
    round: &Value,
    sent: &[Value],
) -> Result<RoundApplied, StoreError> {
    let rows_raw = round.get("sessions").and_then(Value::as_array);
    let progress_raw = round.get("progress").and_then(Value::as_array);
    let (Some(rows_raw), Some(progress_raw)) = (rows_raw, progress_raw) else {
        return Err(StoreError::Unreadable(
            "reading-session sync returned an unreadable round".into(),
        ));
    };
    store.writable()?;
    let meta = PageMeta::decode(round);
    if let Some(server) = meta.server_time {
        store.on_server_time(server);
    }
    let position = Position::load(store.owner_conn(), SyncDomain::Sessions)?;
    let verdict = PageVerdict::of(&position, &meta);
    let before = all_threads(store)?;
    let before_progress: Vec<ProgressRow> = load_all(store.owner_conn())?;
    let mut by_id: BTreeMap<String, ReadingThread> =
        before.iter().map(|t| (t.id.clone(), t.clone())).collect();
    let mut progress_now: HashMap<i64, SurahProgress> = before_progress
        .iter()
        .map(|p| (p.row.surah, p.row.clone()))
        .collect();

    let sent_by_id: HashMap<&str, &Value> = sent
        .iter()
        .filter(|s| str_of(s, "kind") == Some("session.upsert"))
        .filter_map(|s| Some((str_of(s, "id")?, s)))
        .collect();
    let mut capped = store
        .sync_memory
        .capped
        .remove(&SyncDomain::Sessions)
        .unwrap_or_default();
    let mut gone = Vec::new();
    let results: Vec<&Value> = round
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    for result in &results {
        let sent_row = str_of(result, "id").and_then(|id| sent_by_id.get(id).copied());
        apply_result(result, sent_row, &mut capped, &mut by_id, &mut gone);
        // Progress acks: clear dirty when the furthest ayah is still the one sent.
        let status = str_of(result, "status").unwrap_or_default();
        let surah = str_of(result, "id")
            .and_then(|id| id.strip_prefix(PROGRESS_PREFIX))
            .and_then(|s| s.parse::<i64>().ok());
        if let Some(surah) = surah.filter(|_| status == "applied" || status == "superseded") {
            let sent_ayah = sent
                .iter()
                .find(|m| {
                    str_of(m, "kind") == Some("progress.upsert")
                        && m["surah"].as_i64() == Some(surah)
                })
                .and_then(|m| m["furthestAyah"].as_i64());
            if let Some(current) = progress_now.get_mut(&surah)
                && sent_ayah == Some(current.furthest_ayah)
            {
                current.dirty = false;
            }
        }
    }
    store
        .sync_memory
        .capped
        .insert(SyncDomain::Sessions, capped);
    for id in gone {
        if let Some(mut thread) = by_id.remove(&id) {
            gone_locally(store, &mut thread)?;
            by_id.insert(id, thread);
        }
    }

    let mut seen = store
        .sync_memory
        .full_pull_seen
        .remove(&SyncDomain::Sessions);
    for row in rows_raw.iter().filter_map(decode_thread) {
        for stamp in [&row.identity_stamp, &row.cursor_stamp, &row.lifecycle_stamp] {
            store.observe(stamp.as_deref());
        }
        if let Some(seen) = seen.as_mut() {
            seen.insert(row.id.clone());
        }
        // The server's device label is authoritative for the row it came with.
        let merged = match by_id.get(&row.id) {
            Some(mine) => ReadingThread {
                device_name: row.device_name.clone(),
                ..merge_row(mine, &row)
            },
            None => row,
        };
        by_id.insert(merged.id.clone(), merged);
    }
    let mut cleared_tab = false;
    for tombstone in rows_raw
        .iter()
        .filter(|r| str_of(r, "state") == Some("deleted"))
    {
        let Some(id) = str_of(tombstone, "id") else {
            continue;
        };
        let lifecycle = opt_str(tombstone, "lifecycleStamp");
        for k in ["identityStamp", "cursorStamp", "lifecycleStamp"] {
            store.observe(str_of(tombstone, k));
        }
        let Some(mine) = by_id.get_mut(id) else {
            continue;
        };
        let local_wins = mine.lifecycle_stamp.is_some()
            && compare_stamps(mine.lifecycle_stamp.as_deref(), lifecycle.as_deref()).is_gt();
        if local_wins {
            continue;
        }
        mine.state = "deleted".into();
        mine.lifecycle_stamp = lifecycle;
        cleared_tab |= !mine.dirty && store.tab.id.as_deref() == Some(id);
    }
    for row in progress_raw.iter().filter_map(decode_progress) {
        let newer = progress_now
            .get(&row.surah)
            .is_none_or(|prev| row.furthest_ayah >= prev.furthest_ayah);
        if newer {
            progress_now.insert(
                row.surah,
                SurahProgress {
                    dirty: false,
                    ..row
                },
            );
        }
    }

    if verdict.below_floor {
        seen = Some(HashSet::new());
    }
    if verdict.restored {
        by_id.values_mut().for_each(|t| t.dirty = true);
    }
    if !verdict.reset()
        && !meta.more
        && let Some(seen) = seen.take()
    {
        let missing: Vec<String> = by_id
            .values()
            .filter(|t| !t.dirty && !seen.contains(&t.id))
            .map(|t| t.id.clone())
            .collect();
        for id in missing {
            if let Some(mut thread) = by_id.remove(&id) {
                gone_locally(store, &mut thread)?;
                by_id.insert(id, thread);
            }
        }
    }
    if let Some(seen) = seen {
        store
            .sync_memory
            .full_pull_seen
            .insert(SyncDomain::Sessions, seen);
    }
    if cleared_tab {
        clear_tab(store)?;
    }

    let threads: Vec<ReadingThread> = by_id.into_values().collect();
    let after = merge_threads(&threads, &[], store.now());
    let after_progress: Vec<ProgressRow> =
        progress_now.into_values().map(ProgressRow::of).collect();
    let next = verdict.next(&position, &meta);
    let result = in_tx(store.owner_conn(), |tx| {
        save_diff(tx, &before, &after)?;
        save_diff(tx, &before_progress, &after_progress)?;
        next.save(tx, SyncDomain::Sessions)
    });
    store.track(result)?;
    store.persist_clock()?;
    store.mark_seen()?;

    let progressed = results
        .iter()
        .any(|r| matches!(str_of(r, "status"), Some("applied" | "superseded")));
    let dirty_left = !dirty_rows(store)?.is_empty();
    Ok(RoundApplied {
        more: verdict.reset() || meta.more || (sent.len() >= SYNC_PAGE && progressed && dirty_left),
        settings: Vec::new(),
    })
}

/// Sign-in: the device's signed-out threads and progress join the account
/// silently, each stamped from its own time. Returns how many threads joined.
pub(crate) fn absorb_local(store: &mut AppStore) -> Result<usize, StoreError> {
    let local: Vec<ReadingThread> = load_all(store.local_conn())?;
    let local_progress: Vec<ProgressRow> = load_all(store.local_conn())?;
    if local.is_empty() && local_progress.is_empty() {
        return Ok(0);
    }
    let device_id = store.device_id().map(str::to_owned);
    let mut incoming = Vec::new();
    for thread in &local {
        let stamp = store.stamp_at(ms_from_iso(&thread.updated_at).unwrap_or(0))?;
        incoming.push(ReadingThread {
            device_id: device_id.clone(),
            identity_stamp: stamp.clone(),
            cursor_stamp: stamp.clone(),
            lifecycle_stamp: stamp,
            dirty: true,
            rejected: false,
            ..thread.clone()
        });
    }
    let before = all_threads(store)?;
    let after = merge_threads(&before, &incoming, store.now());
    let before_progress: Vec<ProgressRow> = load_all(store.owner_conn())?;
    let mut merged: HashMap<i64, SurahProgress> = before_progress
        .iter()
        .map(|p| (p.row.surah, p.row.clone()))
        .collect();
    for p in &local_progress {
        let take = merged.get(&p.row.surah).is_none_or(|prev| {
            p.row.furthest_ayah > prev.furthest_ayah
                || (p.row.furthest_ayah == prev.furthest_ayah && !prev.dirty)
        });
        if take {
            merged.insert(
                p.row.surah,
                SurahProgress {
                    dirty: true,
                    ..p.row.clone()
                },
            );
        }
    }
    let after_progress: Vec<ProgressRow> = merged.into_values().map(ProgressRow::of).collect();
    let result = in_tx(store.owner_conn(), |tx| {
        save_diff(tx, &before, &after)?;
        save_diff(tx, &before_progress, &after_progress)
    });
    store.track(result)?;
    let result = in_tx(store.local_conn(), |tx| {
        delete_domain(tx, ReadingThread::DOMAIN)?;
        delete_domain(tx, ProgressRow::DOMAIN)?;
        kv_remove(tx, TAB_KEY)
    });
    store.track(result)?;
    Ok(local.len())
}

/// The export form of a thread: content and lifecycle only, no device id or stamps.
pub(crate) fn thread_export(t: &ReadingThread) -> Value {
    json!({
        "id": t.id, "origin": t.origin, "originRef": t.origin_ref, "scope": t.scope,
        "boundEnd": t.bound_end, "surah": t.surah, "ayah": t.ayah, "localPage": t.local_page,
        "ratio": t.ratio, "sourceId": t.source_id, "device": t.device.as_str(),
        "startedAt": t.started_at, "updatedAt": t.updated_at, "endedAt": t.ended_at,
        "endReason": t.end_reason, "state": t.state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::memory_store;

    fn target(scope: &str, surah: i64, ayah: i64, exact: bool) -> ReaderTarget {
        ReaderTarget {
            scope: scope.into(),
            surah,
            ayah,
            exact,
            bound_end: None,
            source_id: None,
        }
    }

    #[test]
    fn an_opened_scope_stays_a_draft_until_read() {
        let (mut store, _) = memory_store(1);
        let entered = enter(&mut store, &target("surah:2", 2, 1, false), None, false).unwrap();
        assert!(all(&store).unwrap().is_empty());
        cursor(&mut store, 2, 1, None).unwrap();
        assert!(
            all(&store).unwrap().is_empty(),
            "an unmoved cursor is no evidence"
        );
        note_read(&mut store, 2, 3).unwrap();
        let stored = all(&store).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, entered.thread.id);
        assert_eq!((stored[0].surah, stored[0].ayah), (2, 3));
        assert_eq!(furthest_in(&store, 2).unwrap(), Some(3));
    }

    #[test]
    fn whole_scope_entry_resumes_with_an_offer_and_exact_entry_starts_new() {
        let (mut store, clock) = memory_store(2);
        enter(&mut store, &target("surah:2", 2, 1, false), None, false).unwrap();
        cursor(&mut store, 2, 50, None).unwrap();
        clock.advance(1000);
        let search = EntryIntent {
            origin: "index".into(),
            ..EntryIntent::default()
        };
        let resumed = enter(
            &mut store,
            &target("surah:2", 2, 1, false),
            Some(search),
            false,
        )
        .unwrap();
        assert!(resumed.resumed);
        assert_eq!(offer(&store), Some(resumed.thread.id.clone()));
        let fresh = start_over(&mut store).unwrap().unwrap();
        assert_ne!(fresh.id, resumed.thread.id);
        let exact = EntryIntent {
            origin: "search".into(),
            exact: true,
            ..EntryIntent::default()
        };
        let new = enter(
            &mut store,
            &target("surah:2", 2, 7, true),
            Some(exact),
            false,
        )
        .unwrap();
        assert_ne!(new.thread.id, resumed.thread.id);
    }

    #[test]
    fn bounded_threads_complete_and_resume_reopens() {
        let (mut store, _) = memory_store(3);
        let card = EntryIntent {
            origin: "card".into(),
            scope: Some("range:1:1-1:3".into()),
            ..EntryIntent::default()
        };
        let entered = enter(
            &mut store,
            &target("surah:1", 1, 1, false),
            Some(card),
            false,
        )
        .unwrap();
        assert_eq!(entered.thread.bound_end.as_deref(), Some("1:3"));
        note_read(&mut store, 1, 3).unwrap();
        let done = find_stored(&store, &entered.thread.id).unwrap().unwrap();
        assert_eq!(done.end_reason.as_deref(), Some("completed"));
        let back = enter(
            &mut store,
            &target("surah:1", 1, 1, false),
            Some(EntryIntent::resume(&entered.thread.id)),
            false,
        )
        .unwrap();
        assert_eq!(back.thread.state, "open");
    }

    #[test]
    fn a_cursor_move_names_this_install_and_lifecycle_writes_keep_the_mover() {
        let (mut store, _) = memory_store(4);
        store.sign_in(7, "dev-here").unwrap();
        enter(&mut store, &target("surah:3", 3, 1, false), None, false).unwrap();
        note_read(&mut store, 3, 2).unwrap();
        let mut thread = all(&store).unwrap().remove(0);
        assert_eq!(thread.device_id.as_deref(), Some("dev-here"));
        assert!(thread.dirty);
        // Simulate a pulled row last moved elsewhere, then dismiss here.
        thread.device_id = Some("dev-other".into());
        thread.dirty = false;
        save(store.owner_conn(), &thread).unwrap();
        dismiss(&mut store, &thread.id).unwrap();
        let after = find_stored(&store, &thread.id).unwrap().unwrap();
        assert_eq!(after.device_id.as_deref(), Some("dev-other"));
        assert_eq!(after.cursor_stamp, thread.cursor_stamp);
        assert_ne!(after.lifecycle_stamp, thread.lifecycle_stamp);
    }

    #[test]
    fn handoff_labels() {
        let (mut store, _) = memory_store(5);
        let here = HereDevice {
            device_id: Some("a".into()),
            kind: DeviceKind::Phone,
        };
        let mut t = new_thread(
            &mut store,
            NewThread {
                origin: "link".into(),
                origin_ref: None,
                scope: "surah:1".into(),
                bound_end: None,
                surah: 1,
                ayah: 1,
                source_id: None,
            },
        );
        t.device = DeviceKind::Phone;
        t.device_id = Some("b".into());
        assert_eq!(
            handoff_place(&t, &here),
            HandoffPlace::Device(DeviceKind::Phone)
        );
        t.device_name = Some("Pixel".into());
        assert_eq!(
            handoff_place(&t, &here),
            HandoffPlace::Named("Pixel".into())
        );
        t.device_id = Some("a".into());
        assert_eq!(handoff_place(&t, &here), HandoffPlace::Here);
        t.device_id = None;
        assert!(is_thread_here(&t, &here));
    }

    #[test]
    fn signed_out_threads_fold_in_at_sign_in_with_their_own_time() {
        let (mut store, clock) = memory_store(6);
        enter(&mut store, &target("surah:5", 5, 1, false), None, false).unwrap();
        note_read(&mut store, 5, 4).unwrap();
        let local = all(&store).unwrap().remove(0);
        clock.advance(60_000);
        let report = store.sign_in(9, "dev").unwrap();
        assert_eq!(report.absorbed_threads, 1);
        let joined = all(&store).unwrap().remove(0);
        assert_eq!(joined.id, local.id);
        assert!(joined.dirty);
        let stamp_ms = parse_stamp(joined.cursor_stamp.as_deref().unwrap())
            .unwrap()
            .ms as i64;
        assert_eq!(Some(stamp_ms), ms_from_iso(&local.updated_at));
        assert_eq!(dirty_rows(&store).unwrap().len(), 2, "thread + progress");
        store.sign_out();
        assert!(
            all(&store).unwrap().is_empty(),
            "local rows moved, not copied"
        );
    }
}
