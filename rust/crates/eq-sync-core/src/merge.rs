//! Row merges. Each register is `{ "stamp": string|null, "value": any }` and
//! merges alone, so every function here is commutative, associative and
//! idempotent. A register absent from an input means no change, never a clear.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::canonical::canonical_json;
use crate::stamp::compare_stamps;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// Highest stamp wins.
    Highest,
    /// Set once: lowest stamp wins.
    Lowest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Thread,
    Progress,
    Bookmark,
    Folder,
    Event,
}

fn stamp_of(register: &Value) -> Option<&str> {
    register.get("stamp").and_then(Value::as_str)
}

fn tie_break<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    let (ca, cb) = (canonical_json(a), canonical_json(b));
    if ca.as_bytes() >= cb.as_bytes() {
        a
    } else {
        b
    }
}

pub fn merge_register(a: &Value, b: &Value, rule: Rule) -> Value {
    let order = compare_stamps(stamp_of(a), stamp_of(b));
    let winner = match (order, rule) {
        (Ordering::Equal, _) => tie_break(a, b),
        (Ordering::Greater, Rule::Highest) | (Ordering::Less, Rule::Lowest) => a,
        _ => b,
    };
    winner.clone()
}

fn is_register(value: &Value) -> bool {
    value.as_object().is_some_and(|o| o.contains_key("stamp"))
}

/// Field by field. Unknown fields keep: a register by the highest-stamp rule,
/// anything else by the higher canonical bytes.
fn merge_fields(a: &Value, b: &Value, rule_for: impl Fn(&str) -> Rule) -> Value {
    let empty = Map::new();
    let left = a.as_object().unwrap_or(&empty);
    let right = b.as_object().unwrap_or(&empty);
    let mut out = Map::new();
    for key in left.keys().chain(right.keys()) {
        if out.contains_key(key) {
            continue;
        }
        let merged = match (left.get(key), right.get(key)) {
            (Some(x), Some(y)) if is_register(x) && is_register(y) => {
                merge_register(x, y, rule_for(key))
            }
            (Some(x), Some(y)) => tie_break(x, y).clone(),
            (Some(x), None) | (None, Some(x)) => x.clone(),
            (None, None) => continue,
        };
        out.insert(key.clone(), merged);
    }
    Value::Object(out)
}

/// Identity is set once; cursor, lifecycle and anything newer take the highest stamp.
pub fn merge_thread(a: &Value, b: &Value) -> Value {
    merge_fields(a, b, |key| {
        if key == "identity" {
            Rule::Lowest
        } else {
            Rule::Highest
        }
    })
}

/// MAX of `furthestAyah`: progress never regresses.
pub fn merge_progress(a: &Value, b: &Value) -> Value {
    let ayah = |row: &Value| row.get("furthestAyah").and_then(Value::as_i64).unwrap_or(0);
    match ayah(a).cmp(&ayah(b)) {
        Ordering::Greater => a.clone(),
        Ordering::Less => b.clone(),
        Ordering::Equal => tie_break(a, b).clone(),
    }
}

/// Whole-row register: folder, key, context and deleted move together.
pub fn merge_bookmark(a: &Value, b: &Value) -> Value {
    merge_fields(a, b, |_| Rule::Highest)
}

fn is_default_folder(row: &Value) -> bool {
    row.get("isDefault").and_then(Value::as_bool) == Some(true)
}

fn force_alive(row: &Value) -> Value {
    let mut out = row.clone();
    if let Some(value) = out.get_mut("deleted").and_then(|d| d.get_mut("value")) {
        *value = Value::Bool(false);
    }
    out
}

/// Name, position and deleted each merge alone. The default folder cannot be
/// deleted, and either side knowing a folder is the default is enough.
pub fn merge_folder(a: &Value, b: &Value) -> Value {
    if is_default_folder(a) || is_default_folder(b) {
        return merge_fields(&force_alive(a), &force_alive(b), |_| Rule::Highest);
    }
    merge_fields(a, b, |_| Rule::Highest)
}

/// One register per setting key, so two settings changed apart both survive.
pub fn merge_settings(a: &Value, b: &Value) -> Value {
    merge_fields(a, b, |_| Rule::Highest)
}

/// The home layout is one document.
pub fn merge_layout(a: &Value, b: &Value) -> Value {
    merge_register(a, b, Rule::Highest)
}

/// Reading events are insert-once by id, so both inputs are the same event.
pub fn merge_event(a: &Value, b: &Value) -> Value {
    tie_break(a, b).clone()
}

fn row_id(domain: Domain, row: &Value) -> String {
    let field = if domain == Domain::Progress {
        "surah"
    } else {
        "id"
    };
    match row.get(field) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => format!("{:0>8}", other),
        None => String::new(),
    }
}

/// Union by id, sorted by id. Rows with different ids are never merged, so two
/// threads on one scope and two folders with one name both stay.
pub fn merge_rows(domain: Domain, a: &[Value], b: &[Value]) -> Vec<Value> {
    let mut by_id: BTreeMap<String, Value> = BTreeMap::new();
    for row in a.iter().chain(b.iter()) {
        let id = row_id(domain, row);
        let merged = match by_id.get(&id) {
            Some(existing) => match domain {
                Domain::Thread => merge_thread(existing, row),
                Domain::Progress => merge_progress(existing, row),
                Domain::Bookmark => merge_bookmark(existing, row),
                Domain::Folder => merge_folder(existing, row),
                Domain::Event => merge_event(existing, row),
            },
            None => row.clone(),
        };
        by_id.insert(id, merged);
    }
    by_id.into_values().collect()
}

fn bookmark_field<'a>(row: &'a Value, field: &str) -> Option<&'a Value> {
    row.get("register")?.get("value")?.get(field)
}

fn set_bookmark_field(row: &mut Value, field: &str, value: Value) {
    if let Some(slot) = row
        .get_mut("register")
        .and_then(|r| r.get_mut("value"))
        .and_then(Value::as_object_mut)
    {
        slot.insert(field.to_string(), value);
    }
}

fn bookmark_stamp(row: &Value) -> Option<&str> {
    row.get("register").and_then(stamp_of)
}

fn is_live(row: &Value) -> bool {
    bookmark_field(row, "deleted").and_then(Value::as_bool) != Some(true)
}

fn outranks(a: &Value, b: &Value) -> bool {
    match compare_stamps(bookmark_stamp(a), bookmark_stamp(b)) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => canonical_json(a).as_bytes() > canonical_json(b).as_bytes(),
    }
}

/// Set-level rules, run after merging rows. Writes keep their stamp: at equal
/// stamps `deleted: true` has the higher canonical bytes, so a tombstone made
/// here beats the live copy on every replica. Older bookmarks die with a deleted
/// folder and newer ones move to the default folder; then two live ids on one
/// key leave only the highest stamp.
pub fn converge_bookmarks(
    rows: &[Value],
    folders: &[Value],
    default_folder_id: &str,
) -> Vec<Value> {
    let delete_stamp = |folder_id: &str| -> Option<Option<&str>> {
        let folder = folders
            .iter()
            .find(|f| f.get("id").and_then(Value::as_str) == Some(folder_id))?;
        let deleted = folder.get("deleted")?;
        let is_deleted = deleted.get("value").and_then(Value::as_bool) == Some(true);
        is_deleted.then(|| stamp_of(deleted))
    };

    let mut out: Vec<Value> = rows.to_vec();
    for row in &mut out {
        if !is_live(row) {
            continue;
        }
        let folder_id = bookmark_field(row, "folderId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some(deleted_at) = delete_stamp(&folder_id) else {
            continue;
        };
        if compare_stamps(bookmark_stamp(row), deleted_at) == Ordering::Greater {
            set_bookmark_field(
                row,
                "folderId",
                Value::String(default_folder_id.to_string()),
            );
        } else {
            set_bookmark_field(row, "deleted", Value::Bool(true));
        }
    }

    let mut winners: BTreeMap<String, usize> = BTreeMap::new();
    let mut losers: Vec<usize> = Vec::new();
    for (index, row) in out.iter().enumerate() {
        if !is_live(row) {
            continue;
        }
        let slot = format!(
            "{}|{}",
            bookmark_field(row, "folderId")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            bookmark_field(row, "key")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        match winners.get(&slot).copied() {
            Some(current) if outranks(&out[current], row) => losers.push(index),
            Some(current) => {
                losers.push(current);
                winners.insert(slot, index);
            }
            None => {
                winners.insert(slot, index);
            }
        }
    }
    for index in losers {
        set_bookmark_field(&mut out[index], "deleted", Value::Bool(true));
    }
    out
}

/// Applied before a ratio is stamped so every replica stores the same value.
pub fn round_ratio(ratio: f64) -> f64 {
    (ratio * 10_000.0).round() / 10_000.0
}
