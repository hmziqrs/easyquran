//! Home (PARITY G6; web `lib/home/{layout,cards}.svelte.ts`). Both are
//! device-wide documents in the local file:
//!
//! - the widget layout (`easyquran.home`), synced whole per account as the
//!   `home.layout` preference when signed in;
//! - the custom cards (pinned surah, ayah, range, zikr, snippet, collection)
//!   with tallies, the recite-event ring and schedules. Local only, never
//!   synced, as on the web; ids are random so a later sync maps 1:1.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use super::{
    AppStore, StoreError, preferences,
    rows::{int_in, kv_json, kv_set_json, str_of},
};
use crate::sync::clock::iso_from_ms;

const LAYOUT_KEY: &str = "home.layout";
const CARDS_KEY: &str = "home.cards";
pub const LAYOUT_SCHEMA_VERSION: i64 = 1;
pub const CARDS_SCHEMA_VERSION: i64 = 1;

/// Widget ids with their defaults: (id, visible, span, default recents count).
pub const HOME_WIDGETS: &[(&str, bool, i64, Option<i64>)] = &[
    ("continue-reading", true, 1, None),
    ("bookmarks", true, 1, None),
    ("notes", true, 1, None),
    ("recents", true, 1, Some(5)),
    ("most-read", false, 1, None),
    ("daily-ayah", false, 1, None),
    ("cards", true, 1, None),
];
pub const RECENTS_COUNT_MIN: i64 = 1;
pub const RECENTS_COUNT_MAX: i64 = 8;
pub const RECENTS_COUNT_CHOICES: [i64; 3] = [3, 5, 8];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeWidget {
    pub id: String,
    pub visible: bool,
    pub span: i64,
    /// The recents row count; `None` for widgets without one.
    pub count: Option<i64>,
}

impl HomeWidget {
    fn to_json(&self) -> Value {
        let settings = match self.count {
            Some(count) => json!({ "count": count }),
            None => json!({}),
        };
        json!({ "id": self.id, "visible": self.visible, "span": self.span, "settings": settings })
    }
}

fn default_widget(spec: &(&str, bool, i64, Option<i64>)) -> HomeWidget {
    HomeWidget {
        id: spec.0.into(),
        visible: spec.1,
        span: spec.2,
        count: spec.3,
    }
}

/// Forward-compatible decode: unknown ids drop, missing ids append, every
/// field re-validated; bad rows degrade to defaults.
pub fn decode_layout(raw: &Value) -> Vec<HomeWidget> {
    let defaults = || HOME_WIDGETS.iter().map(default_widget).collect();
    if raw.get("v").and_then(Value::as_i64) != Some(LAYOUT_SCHEMA_VERSION) {
        return defaults();
    }
    let mut out: Vec<HomeWidget> = Vec::new();
    for row in raw
        .get("widgets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = str_of(row, "id") else {
            continue;
        };
        let Some(spec) = HOME_WIDGETS.iter().find(|s| s.0 == id) else {
            continue;
        };
        if out.iter().any(|w| w.id == id) {
            continue;
        }
        let count = row
            .get("settings")
            .and_then(|s| int_in(s, "count", RECENTS_COUNT_MIN, RECENTS_COUNT_MAX))
            .filter(|_| spec.3.is_some());
        out.push(HomeWidget {
            id: id.into(),
            visible: row.get("visible").and_then(Value::as_bool) != Some(false),
            span: int_in(row, "span", 1, 2).unwrap_or(spec.2),
            count,
        });
    }
    for spec in HOME_WIDGETS {
        if !out.iter().any(|w| w.id == spec.0) {
            out.push(default_widget(spec));
        }
    }
    out
}

/// The widget layout, in display order.
pub fn layout(store: &AppStore) -> Result<Vec<HomeWidget>, StoreError> {
    Ok(decode_layout(
        &kv_json(store.local_conn(), LAYOUT_KEY)?.unwrap_or(Value::Null),
    ))
}

/// The layout as the `home.layout` preference carries it (`widgets` array).
pub(crate) fn layout_value(store: &AppStore) -> Result<Value, StoreError> {
    Ok(Value::Array(
        layout(store)?.iter().map(HomeWidget::to_json).collect(),
    ))
}

fn write_layout(store: &mut AppStore, widgets: &[HomeWidget]) -> Result<(), StoreError> {
    let doc = json!({
        "v": LAYOUT_SCHEMA_VERSION,
        "widgets": widgets.iter().map(HomeWidget::to_json).collect::<Vec<_>>(),
    });
    let result = kv_set_json(store.local_writable()?, LAYOUT_KEY, &doc);
    store.track(result)
}

/// A layout that arrived from another device (no capture: it came from the register).
pub(crate) fn replace_layout(store: &mut AppStore, raw: &Value) -> Result<(), StoreError> {
    let mut doc = json!({ "v": LAYOUT_SCHEMA_VERSION });
    if let (Some(out), Some(fields)) = (doc.as_object_mut(), raw.as_object()) {
        out.extend(fields.clone());
    }
    write_layout(store, &decode_layout(&doc))
}

fn edit_layout(
    store: &mut AppStore,
    change: impl FnOnce(&mut Vec<HomeWidget>),
) -> Result<(), StoreError> {
    let mut widgets = layout(store)?;
    change(&mut widgets);
    write_layout(store, &widgets)?;
    preferences::capture_documents(store)
}

pub fn toggle_widget(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    edit_layout(store, |w| {
        w.iter_mut()
            .filter(|w| w.id == id)
            .for_each(|w| w.visible = !w.visible);
    })
}

/// Moves a widget one place up (`-1`) or down (`1`).
pub fn move_widget(store: &mut AppStore, id: &str, direction: i64) -> Result<(), StoreError> {
    edit_layout(store, |w| {
        let Some(index) = w.iter().position(|e| e.id == id) else {
            return;
        };
        let target = index as i64 + direction.signum();
        if target >= 0 && (target as usize) < w.len() {
            let moved = w.remove(index);
            w.insert(target as usize, moved);
        }
    })
}

pub fn set_widget_count(store: &mut AppStore, id: &str, count: i64) -> Result<(), StoreError> {
    if !(RECENTS_COUNT_MIN..=RECENTS_COUNT_MAX).contains(&count) {
        return Ok(());
    }
    edit_layout(store, |w| {
        w.iter_mut()
            .filter(|w| w.id == id)
            .for_each(|w| w.count = Some(count));
    })
}

// --- cards ---

pub const CARD_KINDS: &[&str] = &["surah", "ayah", "range", "zikr", "snippet", "collection"];
pub const MAX_CARDS: usize = 24;
pub const MAX_COLLECTION_ITEMS: usize = 12;
pub const EVENTS_CAP: usize = 500;
pub const MAX_SCHEDULE_SLOTS: usize = 8;
const RANGE_SPAN_MAX: i64 = 50;
const TARGET_MAX: i64 = 10_000;
const COUNT_MAX: i64 = 1_000_000;
const ZIKR_TEXT_MAX: usize = 120;
const ID_MAX: usize = 64;
const CREATED_AT_MAX: usize = 32;
const SLOT_LABEL_MAX: usize = 40;
const EVENT_MS_MAX: i64 = 4_102_444_800_000;

/// What the app's catalogues know: card validation needs the translation
/// catalogue and the snippet list, which live outside this crate's store.
pub trait CardCatalog {
    fn known_source(&self, source_id: &str) -> bool;
    fn known_snippet(&self, slug: &str) -> bool;
}

fn source_of(catalog: &dyn CardCatalog, v: &Value) -> Value {
    match str_of(v, "sourceId") {
        Some(id) if catalog.known_source(id) => json!(id),
        _ => Value::Null,
    }
}

fn valid_time(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && matches!(
            (b[0], b[1]),
            (b'0' | b'1', b'0'..=b'9') | (b'2', b'0'..=b'3')
        )
        && (b'0'..=b'5').contains(&b[3])
        && b[4].is_ascii_digit()
}

fn decode_slot(v: &Value) -> Option<Value> {
    let id = str_of(v, "id").filter(|s| !s.is_empty() && s.len() <= ID_MAX)?;
    let label =
        str_of(v, "label").filter(|s| !s.is_empty() && s.chars().count() <= SLOT_LABEL_MAX)?;
    let time = str_of(v, "time").filter(|t| valid_time(t))?;
    Some(json!({ "id": id, "label": label, "time": time }))
}

fn span_ok(start: i64, end: i64) -> bool {
    start <= end && end - start < RANGE_SPAN_MAX
}

fn decode_item(catalog: &dyn CardCatalog, v: &Value) -> Option<Value> {
    let source = source_of(catalog, v);
    if let Some(slug) = str_of(v, "slug") {
        return catalog
            .known_snippet(slug)
            .then(|| json!({ "slug": slug, "sourceId": source }));
    }
    let surah = int_in(v, "surah", 1, 114)?;
    match (int_in(v, "ayahStart", 1, 286), int_in(v, "ayahEnd", 1, 286)) {
        (None, None) => Some(json!({ "surah": surah, "sourceId": source })),
        (Some(a), Some(b)) if span_ok(a, b) => {
            Some(json!({ "surah": surah, "ayahStart": a, "ayahEnd": b, "sourceId": source }))
        }
        _ => None,
    }
}

fn kind_fields(catalog: &dyn CardCatalog, kind: &str, v: &Value) -> Option<Map<String, Value>> {
    let surah = int_in(v, "surah", 1, 114);
    let fields = match kind {
        "surah" => json!({ "surah": surah?, "sourceId": source_of(catalog, v) }),
        "ayah" => json!({
            "surah": surah?, "ayah": int_in(v, "ayah", 1, 286)?, "sourceId": source_of(catalog, v),
        }),
        "range" => {
            let (a, b) = (
                int_in(v, "ayahStart", 1, 286)?,
                int_in(v, "ayahEnd", 1, 286)?,
            );
            if !span_ok(a, b) {
                return None;
            }
            json!({ "surah": surah?, "ayahStart": a, "ayahEnd": b, "sourceId": source_of(catalog, v) })
        }
        "zikr" => {
            let text = str_of(v, "text")
                .filter(|t| !t.trim().is_empty() && t.chars().count() <= ZIKR_TEXT_MAX)?;
            json!({ "text": text })
        }
        "snippet" => {
            let slug = str_of(v, "slug").filter(|s| catalog.known_snippet(s))?;
            json!({ "slug": slug, "sourceId": source_of(catalog, v) })
        }
        "collection" => {
            let items: Vec<Value> = v
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|i| decode_item(catalog, i))
                .take(MAX_COLLECTION_ITEMS)
                .collect();
            if items.is_empty() {
                return None;
            }
            json!({ "items": items })
        }
        _ => return None,
    };
    fields.as_object().cloned()
}

/// One card, validated; bad cards drop, caps clamp.
pub fn decode_card(catalog: &dyn CardCatalog, v: &Value, now_ms: i64) -> Option<Value> {
    let kind = str_of(v, "kind").filter(|k| CARD_KINDS.contains(k))?;
    let id = str_of(v, "id").filter(|id| !id.is_empty() && id.len() <= ID_MAX)?;
    let mut card = kind_fields(catalog, kind, v)?;
    let created = str_of(v, "createdAt")
        .filter(|c| !c.is_empty() && c.len() <= CREATED_AT_MAX)
        .map_or_else(|| iso_from_ms(now_ms), str::to_owned);
    card.insert("id".into(), json!(id));
    card.insert("kind".into(), json!(kind));
    card.insert(
        "count".into(),
        json!(int_in(v, "count", 0, COUNT_MAX).unwrap_or(0)),
    );
    card.insert("createdAt".into(), json!(created));
    if let Some(target) = int_in(v, "target", 1, TARGET_MAX) {
        card.insert("target".into(), json!(target));
    }
    let events: Vec<i64> = v
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .filter(|ms| (0..=EVENT_MS_MAX).contains(ms))
        .collect();
    if !events.is_empty() {
        let skip = events.len().saturating_sub(EVENTS_CAP);
        card.insert("events".into(), json!(events[skip..]));
    }
    let slots: Vec<Value> = v
        .get("schedule")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(decode_slot)
        .take(MAX_SCHEDULE_SLOTS)
        .collect();
    if !slots.is_empty() {
        card.insert("schedule".into(), json!(slots));
    }
    Some(Value::Object(card))
}

/// Forward-compatible decode of `{v, cards}`: unique ids, at most [`MAX_CARDS`].
pub fn decode_cards(catalog: &dyn CardCatalog, raw: &Value, now_ms: i64) -> Vec<Value> {
    if raw.get("v").and_then(Value::as_i64) != Some(CARDS_SCHEMA_VERSION) {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    raw.get("cards")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| decode_card(catalog, c, now_ms))
        .filter(|c| seen.insert(str_of(c, "id").unwrap_or_default().to_owned()))
        .take(MAX_CARDS)
        .collect()
}

fn stored_cards(store: &AppStore) -> Result<Vec<Value>, StoreError> {
    Ok(kv_json(store.local_conn(), CARDS_KEY)?
        .and_then(|doc| doc.get("cards").and_then(Value::as_array).cloned())
        .unwrap_or_default())
}

/// The cards, validated against the app's catalogues.
pub fn cards(store: &AppStore, catalog: &dyn CardCatalog) -> Result<Vec<Value>, StoreError> {
    let raw = json!({ "v": CARDS_SCHEMA_VERSION, "cards": stored_cards(store)? });
    Ok(decode_cards(catalog, &raw, store.local_now()))
}

fn write_cards(store: &mut AppStore, cards: Vec<Value>) -> Result<(), StoreError> {
    let doc = json!({ "v": CARDS_SCHEMA_VERSION, "cards": cards });
    let result = kv_set_json(store.local_writable()?, CARDS_KEY, &doc);
    store.track(result)
}

fn edit_card(
    store: &mut AppStore,
    id: &str,
    change: impl Fn(&mut Map<String, Value>),
) -> Result<(), StoreError> {
    let mut list = stored_cards(store)?;
    for card in list.iter_mut().filter(|c| str_of(c, "id") == Some(id)) {
        if let Some(fields) = card.as_object_mut() {
            change(fields);
        }
    }
    write_cards(store, list)
}

/// A fresh card id (random v4, as the web's `crypto.randomUUID`).
pub fn new_card_id(store: &mut AppStore) -> String {
    store.new_v4()
}

/// `false` when the store is full or the card does not validate.
pub fn add_card(
    store: &mut AppStore,
    catalog: &dyn CardCatalog,
    card: &Value,
) -> Result<bool, StoreError> {
    let mut list = stored_cards(store)?;
    let Some(card) = decode_card(catalog, card, store.local_now()) else {
        return Ok(false);
    };
    if list.len() >= MAX_CARDS {
        return Ok(false);
    }
    list.push(card);
    write_cards(store, list)?;
    Ok(true)
}

pub fn remove_card(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    let list: Vec<Value> = stored_cards(store)?
        .into_iter()
        .filter(|c| str_of(c, "id") != Some(id))
        .collect();
    write_cards(store, list)
}

/// One completed recite pass. The tally pins at its cap; the event ring keeps accruing.
pub fn increment_card(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    let now = store.local_now();
    edit_card(store, id, |c| {
        let mut events: Vec<Value> = c
            .get("events")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        events.push(json!(now));
        let skip = events.len().saturating_sub(EVENTS_CAP);
        c.insert("events".into(), Value::Array(events.split_off(skip)));
        let count = c.get("count").and_then(Value::as_i64).unwrap_or(0);
        if count < COUNT_MAX {
            c.insert("count".into(), json!(count + 1));
        }
    })
}

/// Tasbih-style reset: the tally zeroes, the event ring survives.
pub fn reset_card(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    edit_card(store, id, |c| {
        c.insert("count".into(), json!(0));
    })
}

/// Id-keyed upsert of an edited card; never re-adds a removed one.
pub fn replace_card(
    store: &mut AppStore,
    catalog: &dyn CardCatalog,
    next: &Value,
) -> Result<(), StoreError> {
    let Some(next) = decode_card(catalog, next, store.local_now()) else {
        return Ok(());
    };
    let id = str_of(&next, "id").unwrap_or_default().to_owned();
    let mut list = stored_cards(store)?;
    let Some(slot) = list
        .iter_mut()
        .find(|c| str_of(c, "id") == Some(id.as_str()))
    else {
        return Ok(());
    };
    *slot = next;
    write_cards(store, list)
}

/// Copy under a fresh id with a zeroed tally and no history.
pub fn duplicate_card(
    store: &mut AppStore,
    catalog: &dyn CardCatalog,
    id: &str,
) -> Result<bool, StoreError> {
    let Some(mut copy) = stored_cards(store)?
        .into_iter()
        .find(|c| str_of(c, "id") == Some(id))
    else {
        return Ok(false);
    };
    let new_id = new_card_id(store);
    let created = store.now_iso();
    if let Some(fields) = copy.as_object_mut() {
        fields.insert("id".into(), json!(new_id));
        fields.insert("count".into(), json!(0));
        fields.insert("createdAt".into(), json!(created));
        fields.remove("events");
    }
    add_card(store, catalog, &copy)
}

/// Replaces or clears a card's target without touching the tally.
pub fn set_card_target(
    store: &mut AppStore,
    id: &str,
    target: Option<i64>,
) -> Result<(), StoreError> {
    if target.is_some_and(|t| !(1..=TARGET_MAX).contains(&t)) {
        return Ok(());
    }
    edit_card(store, id, |c| match target {
        Some(t) => {
            c.insert("target".into(), json!(t));
        }
        None => {
            c.remove("target");
        }
    })
}

/// Re-pins a card's translation (`None` = Arabic). Zikr and collection cards carry none.
pub fn set_card_source(
    store: &mut AppStore,
    catalog: &dyn CardCatalog,
    id: &str,
    source_id: Option<&str>,
) -> Result<(), StoreError> {
    if source_id.is_some_and(|s| !catalog.known_source(s)) {
        return Ok(());
    }
    edit_card(store, id, |c| {
        if c.contains_key("sourceId") {
            c.insert("sourceId".into(), json!(source_id));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::memory_store;

    struct All;
    impl CardCatalog for All {
        fn known_source(&self, _: &str) -> bool {
            true
        }
        fn known_snippet(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn layout_defaults_moves_and_forward_compat() {
        let (mut store, _) = memory_store(1);
        assert_eq!(layout(&store).unwrap().len(), HOME_WIDGETS.len());
        move_widget(&mut store, "bookmarks", -1).unwrap();
        assert_eq!(layout(&store).unwrap()[0].id, "bookmarks");
        set_widget_count(&mut store, "recents", 8).unwrap();
        toggle_widget(&mut store, "notes").unwrap();
        let w = layout(&store).unwrap();
        assert_eq!(w.iter().find(|w| w.id == "recents").unwrap().count, Some(8));
        assert!(!w.iter().find(|w| w.id == "notes").unwrap().visible);
        let odd = json!({"v": 1, "widgets": [{"id": "future-widget"}, {"id": "cards", "visible": false}]});
        let decoded = decode_layout(&odd);
        assert_eq!(decoded[0].id, "cards");
        assert_eq!(decoded.len(), HOME_WIDGETS.len());
    }

    #[test]
    fn cards_tally_ring_and_caps() {
        let (mut store, _) = memory_store(2);
        let id = new_card_id(&mut store);
        let card = json!({"id": id, "kind": "zikr", "text": "subhanallah", "count": 0});
        assert!(add_card(&mut store, &All, &card).unwrap());
        increment_card(&mut store, &id).unwrap();
        increment_card(&mut store, &id).unwrap();
        reset_card(&mut store, &id).unwrap();
        let list = cards(&store, &All).unwrap();
        assert_eq!(list[0]["count"], 0);
        assert_eq!(list[0]["events"].as_array().unwrap().len(), 2);
        assert!(duplicate_card(&mut store, &All, &id).unwrap());
        assert_eq!(cards(&store, &All).unwrap().len(), 2);
        let bad = json!({"id": "x", "kind": "range", "surah": 2, "ayahStart": 1, "ayahEnd": 200});
        assert!(!add_card(&mut store, &All, &bad).unwrap());
    }
}
