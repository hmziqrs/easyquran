//! Pinned verses (web `stores/pins.svelte.ts`): device-local, never synced,
//! nothing sent. Stored under `easyquran.reader.pins` as
//! `{ "v": 1, "pins": [{ "key": "S:A", "sourceId": id|null, "text": "...",
//! "lanes": [{ "sourceId", "name", "text" }] }] }`; stored order is pin
//! order. `text` is the verse line as the row showed it when pinned (the
//! Arabic on the Arabic reader, else the route translation's text), kept
//! verbatim: it is shown as stored, never trimmed.

use eq_core::kv::KeyValue;
use nsh_core::prelude::*;
use serde_json::{Value, json};

use crate::{services::storage::NshKv, stores::notes::parse_verse_key};

pub const PINS_STORAGE_KEY: &str = "easyquran.reader.pins";
const PINS_SCHEMA_VERSION: i64 = 1;
/// Most verses one device keeps pinned at once.
pub const PINS_MAX: usize = 20;
/// A longer string is malformed storage, not a verse.
const PIN_TEXT_MAX: usize = 8000;
const PIN_LANES_MAX: usize = 5;
/// Compare-view selection id for the Arabic text.
pub const ARABIC_SELECTION: &str = "arabic";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedLane {
    pub source_id: String,
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedVerse {
    /// `S:A`.
    pub key: String,
    /// `None`: pinned on the Arabic reader.
    pub source_id: Option<String>,
    pub text: String,
    /// Stacked translations shown when pinned.
    pub lanes: Vec<PinnedLane>,
}

/// What [`PinsStore::toggle`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinToggle {
    Added,
    Removed,
    /// [`PINS_MAX`] reached: nothing pinned.
    Full,
}

fn text_of(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|t| t.chars().count() <= PIN_TEXT_MAX)
        .map(str::to_owned)
}

fn decode_lane(v: &Value) -> Option<PinnedLane> {
    let source_id = v.get("sourceId")?.as_str()?.trim().to_owned();
    if source_id.is_empty() {
        return None;
    }
    Some(PinnedLane {
        source_id,
        name: v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        text: text_of(v.get("text")?)?,
    })
}

fn decode_pin(v: &Value) -> Option<PinnedVerse> {
    let key = v.get("key")?.as_str()?.to_owned();
    parse_verse_key(&key)?;
    let source_id = v
        .get("sourceId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let lanes = v
        .get("lanes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(decode_lane)
                .take(PIN_LANES_MAX)
                .collect()
        })
        .unwrap_or_default();
    Some(PinnedVerse {
        key,
        source_id,
        text: text_of(v.get("text")?)?,
        lanes,
    })
}

/// `decodePins`: malformed rows and duplicate keys drop; a newer schema
/// reads as empty.
pub fn decode_pins(raw: &str) -> Vec<PinnedVerse> {
    let Ok(stored) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    if stored
        .get("v")
        .and_then(Value::as_i64)
        .is_some_and(|v| v > PINS_SCHEMA_VERSION)
    {
        return Vec::new();
    }
    let mut out: Vec<PinnedVerse> = Vec::new();
    for pin in stored
        .get("pins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(decode_pin)
    {
        if out.iter().any(|p| p.key == pin.key) {
            continue;
        }
        out.push(pin);
        if out.len() >= PINS_MAX {
            break;
        }
    }
    out
}

pub fn encode_pins(pins: &[PinnedVerse]) -> String {
    let rows: Vec<Value> = pins
        .iter()
        .map(|p| {
            json!({
                "key": p.key,
                "sourceId": p.source_id,
                "text": p.text,
                "lanes": p.lanes.iter().map(|l| json!({
                    "sourceId": l.source_id, "name": l.name, "text": l.text,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "v": PINS_SCHEMA_VERSION, "pins": rows }).to_string()
}

/// Selection ids the compare view offers: Arabic first, then each
/// translation the pins carry, first-seen order.
pub fn compare_selections(pins: &[PinnedVerse]) -> Vec<String> {
    let mut ids = vec![ARABIC_SELECTION.to_owned()];
    for pin in pins {
        let lanes = pin.lanes.iter().map(|l| l.source_id.clone());
        for id in pin.source_id.clone().into_iter().chain(lanes) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// One compare cell: what a pin shows under the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareCell {
    pub key: String,
    pub arabic: Option<String>,
    pub translation: Option<String>,
}

/// The pin's cell for `selection`; texts are the pinned texts, unchanged.
pub fn compare_cell(pin: &PinnedVerse, selection: &str) -> CompareCell {
    let arabic = pin.source_id.is_none().then(|| pin.text.clone());
    let translation = match selection {
        ARABIC_SELECTION => None,
        s if pin.source_id.as_deref() == Some(s) => Some(pin.text.clone()),
        s => pin
            .lanes
            .iter()
            .find(|l| l.source_id == s)
            .map(|l| l.text.clone()),
    };
    CompareCell {
        key: pin.key.clone(),
        arabic,
        translation,
    }
}

#[derive(Default, Clone)]
struct PinsState {
    loaded: bool,
    pins: Vec<PinnedVerse>,
}

fn state() -> Signal<PinsState> {
    let s = global::<PinsState>();
    if !s.with_untracked(|st| st.loaded) {
        let pins = NshKv
            .get(PINS_STORAGE_KEY)
            .map(|raw| decode_pins(&raw))
            .unwrap_or_default();
        s.set(PinsState { loaded: true, pins });
    }
    s
}

/// The pins store handle (surface-wide; no context needed).
#[derive(Clone, Copy)]
pub struct PinsStore;

pub fn pins_store() -> PinsStore {
    PinsStore
}

impl PinsStore {
    /// Pins in pin order (tracked).
    pub fn list(self) -> Vec<PinnedVerse> {
        state().with(|s| s.pins.clone())
    }

    /// How many (tracked).
    pub fn count(self) -> usize {
        state().with(|s| s.pins.len())
    }

    /// Whether `key` is pinned (tracked).
    pub fn has(self, key: &str) -> bool {
        state().with(|s| s.pins.iter().any(|p| p.key == key))
    }

    fn commit(self, pins: Vec<PinnedVerse>) {
        // Full or blocked storage keeps the in-memory list.
        let _ = NshKv.set(PINS_STORAGE_KEY, &encode_pins(&pins));
        state().update(|s| s.pins = pins);
    }

    /// Pin, or unpin when pinned. A full set refuses new pins.
    pub fn toggle(self, verse: PinnedVerse) -> PinToggle {
        let pins = state().with_untracked(|s| s.pins.clone());
        if pins.iter().any(|p| p.key == verse.key) {
            self.remove(&verse.key);
            return PinToggle::Removed;
        }
        if pins.len() >= PINS_MAX {
            return PinToggle::Full;
        }
        let mut next = pins;
        next.push(verse);
        self.commit(next);
        PinToggle::Added
    }

    pub fn remove(self, key: &str) {
        let pins = state().with_untracked(|s| s.pins.clone());
        if !pins.iter().any(|p| p.key == key) {
            return;
        }
        self.commit(pins.into_iter().filter(|p| p.key != key).collect());
    }

    /// Moves one pin up (`-1`) or down (`1`); edges do nothing.
    pub fn move_by(self, key: &str, delta: i32) {
        let mut pins = state().with_untracked(|s| s.pins.clone());
        let Some(i) = pins.iter().position(|p| p.key == key) else {
            return;
        };
        let j = i as i64 + i64::from(delta);
        if j < 0 || j >= pins.len() as i64 {
            return;
        }
        pins.swap(i, j as usize);
        self.commit(pins);
    }

    pub fn clear(self) {
        if self.count_untracked() > 0 {
            self.commit(Vec::new());
        }
    }

    fn count_untracked(self) -> usize {
        state().with_untracked(|s| s.pins.len())
    }
}
