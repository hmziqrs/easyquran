//! Translation choices (PARITY C6, C7): the stacked extra translations
//! (`stacked-translations.svelte.ts`, `easyquran.reader.stacked`) and the
//! picker's most-recently-used list (`translation-recents.svelte.ts`,
//! `easyquran.translations.recent`). Same persisted blobs, schema versions,
//! caps and decode rules as the web; device-local.
//!
//! One owner each: a surface-wide signal loaded from storage on the first
//! `get`, so the picker, the lanes and the reader write the same value.

use std::collections::BTreeMap;

use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use serde_json::{Map, Value, json};

/// `STACKED_STORAGE_KEY`.
pub const STACKED_KEY: &str = "easyquran.reader.stacked";
const STACKED_VERSION: u64 = 1;
/// `STACKED_MAX_EXTRAS`: how many translations the stack holds.
pub const STACKED_MAX_EXTRAS: usize = 5;
/// Per-lane text size steps (`LANE_STEP_MIN` / `LANE_STEP_MAX`).
pub const LANE_STEP_MIN: i8 = -2;
pub const LANE_STEP_MAX: i8 = 2;
/// Font scale per step, indexed by `step - LANE_STEP_MIN`.
const LANE_SCALES: [f32; 5] = [0.75, 0.875, 1.0, 1.125, 1.25];

/// `TRANSLATION_RECENTS_STORAGE_KEY`.
pub const RECENTS_KEY: &str = "easyquran.translations.recent";
const RECENTS_VERSION: u64 = 1;
/// `TRANSLATION_RECENTS_MAX`.
pub const RECENTS_MAX: usize = 20;

/// How the extra translations sit under a verse (`LaneLayout`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LaneLayout {
    #[default]
    Stacked,
    Parallel,
}

/// `isFutureSchema`: a blob written by a newer schema reads as defaults.
fn is_future(raw: &Value, version: u64) -> bool {
    raw.get("v")
        .and_then(Value::as_u64)
        .is_some_and(|v| v > version)
}

fn read_blob(key: &str) -> Option<Value> {
    Storage::get_item(key).and_then(|s| serde_json::from_str::<Value>(&s).ok())
}

fn write_blob(key: &str, value: &Value) {
    if let Err(e) = Storage::set_item(key, value.to_string()) {
        log::warn!("{key} not saved: {e:?}");
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// The persisted stack (`PersistedStacked`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Stacked {
    pub ids: Vec<String>,
    pub enabled: bool,
    pub layout: LaneLayout,
    /// Non-zero steps of selected lanes only.
    pub steps: BTreeMap<String, i8>,
}

impl Stacked {
    /// `decodeStacked`.
    pub fn from_value(raw: &Value) -> Self {
        let fallback = Stacked {
            enabled: true,
            ..Stacked::default()
        };
        if is_future(raw, STACKED_VERSION) {
            return fallback;
        }
        let Some(o) = raw.as_object() else {
            return fallback;
        };
        let ids: Vec<String> = o
            .get("ids")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let mut steps = BTreeMap::new();
        if let Some(stored) = o.get("steps").and_then(Value::as_object) {
            for id in &ids {
                let step = stored
                    .get(id)
                    .and_then(Value::as_f64)
                    .filter(|n| *n >= f64::from(LANE_STEP_MIN) && *n <= f64::from(LANE_STEP_MAX));
                #[allow(clippy::cast_possible_truncation)]
                if let Some(n) = step.map(|n| n as i8).filter(|n| *n != 0) {
                    steps.insert(id.clone(), n);
                }
            }
        }
        Stacked {
            enabled: o.get("enabled") != Some(&Value::Bool(false)),
            layout: if o.get("layout").and_then(Value::as_str) == Some("parallel") {
                LaneLayout::Parallel
            } else {
                LaneLayout::Stacked
            },
            ids,
            steps,
        }
    }

    /// The record `#commit` writes: `layout` only when parallel, `steps`
    /// only when some lane is stepped.
    pub fn to_value(&self) -> Value {
        let mut record = json!({ "v": STACKED_VERSION, "ids": self.ids, "enabled": self.enabled });
        if self.layout == LaneLayout::Parallel {
            record["layout"] = json!("parallel");
        }
        if !self.steps.is_empty() {
            let steps: Map<String, Value> = self
                .steps
                .iter()
                .map(|(k, v)| (k.clone(), json!(v)))
                .collect();
            record["steps"] = Value::Object(steps);
        }
        record
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
struct HeldStack {
    loaded: bool,
    value: Stacked,
}

/// The stacked translations store (`StackedTranslationsStore`).
#[derive(Clone, Copy)]
pub struct StackStore {
    held: Signal<HeldStack>,
}

impl StackStore {
    /// The surface's store; the first call loads the saved blob.
    pub fn get() -> StackStore {
        let held = global::<HeldStack>();
        if !held.with_untracked(|h| h.loaded) {
            let value = read_blob(STACKED_KEY)
                .map(|v| Stacked::from_value(&v))
                .unwrap_or(Stacked {
                    enabled: true,
                    ..Stacked::default()
                });
            held.set(HeldStack {
                loaded: true,
                value,
            });
        }
        StackStore { held }
    }

    fn read<T>(self, f: impl FnOnce(&Stacked) -> T) -> T {
        self.held.with(|h| f(&h.value))
    }

    /// `#commit`: drops steps of lanes no longer selected, persists.
    fn commit(self, f: impl FnOnce(&mut Stacked)) {
        let mut next = self.held.with_untracked(|h| h.value.clone());
        f(&mut next);
        let ids = next.ids.clone();
        next.steps.retain(|id, step| *step != 0 && ids.contains(id));
        if self.held.with_untracked(|h| h.value == next) {
            return;
        }
        write_blob(STACKED_KEY, &next.to_value());
        self.held.set(HeldStack {
            loaded: true,
            value: next,
        });
    }

    /// The selection, in order. Tracked.
    pub fn ids(self) -> Vec<String> {
        self.read(|s| s.ids.clone())
    }
    /// Whether the reader flows the selection at all.
    pub fn enabled(self) -> bool {
        self.read(|s| s.enabled)
    }
    /// What the reader renders: the selection when on, nothing when paused.
    pub fn active_ids(self) -> Vec<String> {
        self.read(|s| if s.enabled { s.ids.clone() } else { Vec::new() })
    }
    pub fn layout(self) -> LaneLayout {
        self.read(|s| s.layout)
    }
    pub fn lane_step(self, id: &str) -> i8 {
        self.read(|s| s.steps.get(id).copied().unwrap_or(0))
    }
    /// The font scale for one lane (1 at step 0).
    pub fn lane_scale(self, id: &str) -> f32 {
        let i = usize::try_from(self.lane_step(id) - LANE_STEP_MIN).unwrap_or(2);
        LANE_SCALES.get(i).copied().unwrap_or(1.0)
    }
    pub fn is_full(self) -> bool {
        self.read(|s| s.ids.len() >= STACKED_MAX_EXTRAS)
    }

    /// `setIds`: deduped, capped.
    pub fn set_ids(self, ids: &[String]) {
        let mut next: Vec<String> = Vec::new();
        for id in ids {
            if !next.contains(id) {
                next.push(id.clone());
            }
        }
        next.truncate(STACKED_MAX_EXTRAS);
        self.commit(|s| s.ids = next);
    }
    pub fn set_enabled(self, enabled: bool) {
        self.commit(|s| s.enabled = enabled);
    }
    pub fn set_layout(self, layout: LaneLayout) {
        self.commit(|s| s.layout = layout);
    }
    /// One selected lane a step larger (+1) or smaller (-1), clamped.
    pub fn step_lane(self, id: &str, delta: i8) {
        if !self
            .held
            .with_untracked(|h| h.value.ids.iter().any(|i| i == id))
        {
            return;
        }
        let id = id.to_owned();
        self.commit(|s| {
            let current = s.steps.get(&id).copied().unwrap_or(0);
            let next = (current + delta).clamp(LANE_STEP_MIN, LANE_STEP_MAX);
            s.steps.insert(id, next);
        });
    }
    pub fn reset_lane(self, id: &str) {
        let id = id.to_owned();
        self.commit(|s| {
            s.steps.remove(&id);
        });
    }
    /// Adds or removes `id`.
    pub fn toggle(self, id: &str) {
        let mut ids = self.held.with_untracked(|h| h.value.ids.clone());
        if ids.iter().any(|i| i == id) {
            ids.retain(|i| i != id);
        } else {
            ids.push(id.to_owned());
        }
        self.set_ids(&ids);
    }
    /// Swaps `id` with its neighbour (`delta` -1 up, +1 down).
    pub fn reorder(self, id: &str, delta: isize) {
        let id = id.to_owned();
        self.commit(|s| {
            let Some(i) = s.ids.iter().position(|x| *x == id) else {
                return;
            };
            let Some(j) = i.checked_add_signed(delta).filter(|j| *j < s.ids.len()) else {
                return;
            };
            s.ids.swap(i, j);
        });
    }
    pub fn remove(self, id: &str) {
        let id = id.to_owned();
        self.commit(|s| s.ids.retain(|x| *x != id));
    }
    pub fn clear(self) {
        self.commit(|s| s.ids.clear());
    }
}

/// One recently used translation (`TranslationRecentEntry`).
#[derive(Clone, Debug, PartialEq)]
pub struct RecentEntry {
    pub id: String,
    pub last_used_at: i64,
    pub use_count: i64,
}

/// The decoded recents blob (`PersistedTranslationRecents`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Recents {
    pub entries: Vec<RecentEntry>,
    /// Author key -> last-used variant id (Feature 1.4).
    pub authors: BTreeMap<String, String>,
}

impl Recents {
    /// `decodeTranslationRecents`: stored order is the MRU order;
    /// duplicates and malformed entries drop, capped at [`RECENTS_MAX`].
    pub fn from_value(raw: &Value) -> Self {
        let mut out = Recents::default();
        if is_future(raw, RECENTS_VERSION) {
            return out;
        }
        let Some(o) = raw.as_object() else {
            return out;
        };
        for e in o
            .get("entries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = e.get("id").and_then(Value::as_str).unwrap_or("");
            #[allow(clippy::cast_possible_truncation)]
            let last = e
                .get("lastUsedAt")
                .and_then(Value::as_f64)
                .filter(|n| *n >= 0.0)
                .map(|n| n as i64);
            #[allow(clippy::cast_possible_truncation)]
            let count = e
                .get("useCount")
                .and_then(Value::as_f64)
                .filter(|n| *n >= 1.0)
                .map(|n| n as i64);
            let (Some(last), Some(count)) = (last, count) else {
                continue;
            };
            if id.is_empty() || out.entries.iter().any(|x| x.id == id) {
                continue;
            }
            out.entries.push(RecentEntry {
                id: id.to_owned(),
                last_used_at: last,
                use_count: count,
            });
            if out.entries.len() >= RECENTS_MAX {
                break;
            }
        }
        if let Some(authors) = o.get("authors").and_then(Value::as_object) {
            for (k, v) in authors {
                if let Some(id) = v.as_str().filter(|s| !s.is_empty() && !k.is_empty()) {
                    out.authors.insert(k.clone(), id.to_owned());
                }
            }
        }
        out
    }

    /// The envelope: `authors` only once a variant memory exists.
    pub fn to_value(&self) -> Value {
        let entries: Vec<Value> = self
            .entries
            .iter()
            .map(|e| json!({ "id": e.id, "lastUsedAt": e.last_used_at, "useCount": e.use_count }))
            .collect();
        let mut record = json!({ "v": RECENTS_VERSION, "entries": entries });
        if !self.authors.is_empty() {
            record["authors"] = json!(self.authors);
        }
        record
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
struct HeldRecents {
    loaded: bool,
    value: Recents,
}

/// Translations the reader explicitly chose, most recent first
/// (`TranslationRecentsStore`).
#[derive(Clone, Copy)]
pub struct RecentsStore {
    held: Signal<HeldRecents>,
}

impl RecentsStore {
    pub fn get() -> RecentsStore {
        let held = global::<HeldRecents>();
        if !held.with_untracked(|h| h.loaded) {
            let value = read_blob(RECENTS_KEY)
                .map(|v| Recents::from_value(&v))
                .unwrap_or_default();
            held.set(HeldRecents {
                loaded: true,
                value,
            });
        }
        RecentsStore { held }
    }

    /// Ids, most recently used first. Tracked.
    pub fn ids(self) -> Vec<String> {
        self.held
            .with(|h| h.value.entries.iter().map(|e| e.id.clone()).collect())
    }

    /// The MRU rank of `id` (0 = most recent), `None` when never used.
    pub fn rank_of(self, id: &str) -> Option<usize> {
        self.held
            .with(|h| h.value.entries.iter().position(|e| e.id == id))
    }

    /// The author's last-used variant id (Feature 1.4).
    pub fn author_variant(self, author_key: &str) -> Option<String> {
        self.held.with(|h| h.value.authors.get(author_key).cloned())
    }

    /// Records one explicit selection: `id` to the front, its use counted;
    /// `author_key` also notes the author's last-used variant.
    pub fn record(self, id: &str, author_key: Option<&str>) {
        if id.is_empty() {
            return;
        }
        let mut next = self.held.with_untracked(|h| h.value.clone());
        let previous = next
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.use_count);
        next.entries.retain(|e| e.id != id);
        next.entries.insert(
            0,
            RecentEntry {
                id: id.to_owned(),
                last_used_at: now_ms(),
                use_count: previous.map_or(1, |n| n + 1),
            },
        );
        next.entries.truncate(RECENTS_MAX);
        if let Some(key) = author_key.filter(|k| !k.is_empty()) {
            next.authors.insert(key.to_owned(), id.to_owned());
        }
        write_blob(RECENTS_KEY, &next.to_value());
        self.held.set(HeldRecents {
            loaded: true,
            value: next,
        });
    }
}
