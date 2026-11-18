//! Local search history and the search page's translation selection
//! (`stores/search-recents.svelte.ts`, `stores/search-selection.svelte.ts`,
//! `search/page/{selection,query-url}.ts`). Device-local: never synced,
//! never sent. The blobs keep the web's keys and shapes.

use serde_json::{Value, json};

use crate::quran::catalogue::{self, Direction};

/// `SEARCH_RECENTS_STORAGE_KEY`.
pub const RECENTS_KEY: &str = "easyquran.search.recent";
/// `SEARCH_SELECTION_STORAGE_KEY`.
pub const SELECTION_KEY: &str = "easyquran.search.selection";
const SCHEMA_VERSION: u64 = 1;
/// Kept queries per device (`SEARCH_RECENTS_MAX`).
pub const RECENTS_MAX: usize = 10;
const QUERY_MAX: usize = 128;

/// One recent query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEntry {
    pub q: String,
    pub at: i64,
}

/// The recents list, newest first, plus the pause switch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recents {
    pub entries: Vec<RecentEntry>,
    pub paused: bool,
}

fn is_future(raw: &Value) -> bool {
    raw.get("v")
        .and_then(Value::as_u64)
        .is_some_and(|v| v > SCHEMA_VERSION)
}

impl Recents {
    /// `decodeSearchRecents`: malformed entries and duplicates dropped,
    /// capped; a future schema reads as empty.
    pub fn decode(raw: Option<&str>) -> Self {
        let Some(raw) = raw.and_then(|r| serde_json::from_str::<Value>(r).ok()) else {
            return Self::default();
        };
        if is_future(&raw) || !raw.is_object() {
            return Self::default();
        }
        let mut entries: Vec<RecentEntry> = Vec::new();
        for e in raw
            .get("entries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let q = e.get("q").and_then(Value::as_str).unwrap_or("").trim();
            let at = e.get("at").and_then(Value::as_f64);
            let Some(at) = at.filter(|a| *a >= 0.0 && a.fract() == 0.0) else {
                continue;
            };
            if q.is_empty() || q.chars().count() > QUERY_MAX || entries.iter().any(|x| x.q == q) {
                continue;
            }
            entries.push(RecentEntry {
                q: q.to_owned(),
                at: at as i64,
            });
            if entries.len() >= RECENTS_MAX {
                break;
            }
        }
        Self {
            entries,
            paused: raw.get("paused").and_then(Value::as_bool) == Some(true),
        }
    }

    /// The persisted blob.
    pub fn encode(&self) -> String {
        json!({
            "v": SCHEMA_VERSION,
            "entries": self.entries.iter().map(|e| json!({"q": e.q, "at": e.at})).collect::<Vec<_>>(),
            "paused": self.paused,
        })
        .to_string()
    }

    /// Queries, newest first.
    pub fn queries(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.q.clone()).collect()
    }

    /// `record`: moves `q` to the front with a fresh stamp, capped; no-op
    /// while paused. Returns whether anything changed.
    pub fn record(&mut self, q: &str, at: i64) -> bool {
        let q = q.trim();
        if self.paused || q.is_empty() || q.chars().count() > QUERY_MAX {
            return false;
        }
        self.entries.retain(|e| e.q != q);
        self.entries.insert(
            0,
            RecentEntry {
                q: q.to_owned(),
                at,
            },
        );
        self.entries.truncate(RECENTS_MAX);
        true
    }

    /// `clear`. Returns whether anything changed.
    pub fn clear(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.entries.clear();
        true
    }
}

/// `decodeSearchSelection`: the stored id list (strings only).
pub fn decode_selection(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.and_then(|r| serde_json::from_str::<Value>(r).ok()) else {
        return Vec::new();
    };
    if is_future(&raw) {
        return Vec::new();
    }
    raw.get("ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// The persisted selection blob.
pub fn encode_selection(ids: &[String]) -> String {
    json!({ "v": SCHEMA_VERSION, "ids": ids }).to_string()
}

/// Deduplicated, order kept (`uniq`).
pub fn uniq(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// A catalogue translation that reads left to right (`isSearchable`).
pub fn is_searchable(id: &str) -> bool {
    catalogue::translation(id).is_some_and(|t| t.direction == Direction::Ltr)
}

/// `partitionSearchable`: LTR ids, and known RTL ids; unknown ids land in
/// neither.
pub fn partition_searchable(ids: &[String]) -> (Vec<String>, Vec<String>) {
    let mut searchable = Vec::new();
    let mut disabled = Vec::new();
    for id in ids {
        if is_searchable(id) {
            searchable.push(id.clone());
        } else if catalogue::translation(id).is_some() {
            disabled.push(id.clone());
        }
    }
    (searchable, disabled)
}

/// Translation searches in flight per batch (`SEARCH_BATCH_SIZE`).
pub const SEARCH_BATCH_SIZE: usize = 3;

/// `resolveDefaultSelection`: the deep link's ids, else the persisted
/// selection, else the last-read source, else the first stacked
/// translation; each re-validated against the catalogue.
pub fn resolve_default_selection(
    url_t: &[String],
    persisted: &[String],
    last_read: Option<&str>,
    stacked: &[String],
) -> Vec<String> {
    if !url_t.is_empty() {
        return url_t.to_vec();
    }
    let kept: Vec<String> = persisted
        .iter()
        .filter(|id| is_searchable(id))
        .cloned()
        .collect();
    if !kept.is_empty() {
        return kept;
    }
    if let Some(id) = last_read.filter(|id| is_searchable(id)) {
        return vec![id.to_owned()];
    }
    stacked
        .iter()
        .find(|id| is_searchable(id))
        .map(|id| vec![id.clone()])
        .unwrap_or_default()
}

/// `parseSearchUrl`'s `t`: comma list filtered to known LTR ids.
pub fn searchable_ids(raw: &str) -> Vec<String> {
    let ids: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|id| is_searchable(id))
        .map(str::to_owned)
        .collect();
    uniq(&ids)
}
