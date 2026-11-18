//! Synced preferences (PARITY J9, F3, G4, G6; web `lib/preferences/*`): one
//! register per key, so two settings changed on two devices both survive.
//! Keys: `settings.*` (one per setting), `home.layout`, `reading.bookmark`,
//! `goals.reading`. No tombstones: clearing writes `null` with a newer stamp.
//!
//! Sources own their live values; this module owns the registers. The home
//! layout and goals are sources inside the store; settings belong to the
//! app's settings store, which hands its entries to [`capture`] and shows the
//! values a round returns. A first full pull lands before local values are
//! captured, so the account's values win on a new device and keys the
//! account lacks are pushed. Signed out nothing syncs, and only the reading
//! bookmark is held here. Consent flags and notes never sync.

use serde_json::{Value, json};

use super::{
    AppStore, StoreError, goals, home,
    position::{PageMeta, Position},
    rows::{
        RowModel, RowRecord, count_dirty, count_rejected, delete_domain, in_tx, int_in, kv_get,
        kv_set, load_all, opt_str, save, save_diff, str_of,
    },
};
use crate::{
    merge::{canonical_json, compare_stamps},
    sync::{RoundApplied, SyncDomain},
};

pub const SYNC_PATH: &str = "/preference/v1/sync";
pub const MISMATCH_MESSAGE: &str = "Preference sync account changed";
pub const READING_BOOKMARK_KEY: &str = "reading.bookmark";
pub const LAYOUT_KEY: &str = "home.layout";
pub const GOALS_KEY: &str = "goals.reading";
pub const SETTINGS_PREFIX: &str = "settings.";
const SYNC_PAGE: usize = 200;
const BASELINED_KEY: &str = "sync.preferences.baselined";

#[derive(Debug, Clone, PartialEq)]
pub struct PreferenceRow {
    pub key: String,
    pub value: Value,
    pub stamp: Option<String>,
    pub dirty: bool,
    pub rejected: bool,
}

impl RowModel for PreferenceRow {
    const DOMAIN: &'static str = "pref";
    fn id(&self) -> &str {
        &self.key
    }
    fn decode(record: &RowRecord) -> Option<Self> {
        Some(PreferenceRow {
            key: str_of(&record.row, "key")?.to_owned(),
            value: record.row.get("value").cloned().unwrap_or(Value::Null),
            stamp: opt_str(&record.row, "stamp"),
            dirty: record.dirty,
            rejected: record.rejected,
        })
    }
    fn encode(&self) -> RowRecord {
        RowRecord {
            id: self.key.clone(),
            row: json!({ "key": self.key, "value": self.value, "stamp": self.stamp }),
            stamps: vec![self.stamp.clone()],
            dirty: self.dirty,
            rejected: self.rejected,
        }
    }
}

fn same_value(a: &Value, b: &Value) -> bool {
    canonical_json(a) == canonical_json(b)
}

fn row(store: &AppStore, key: &str) -> Result<Option<PreferenceRow>, StoreError> {
    Ok(
        super::rows::get(store.owner_conn(), PreferenceRow::DOMAIN, key)?
            .and_then(|r| PreferenceRow::decode(&r)),
    )
}

/// The register's value, `null` when unset or cleared.
pub fn get(store: &AppStore, key: &str) -> Result<Value, StoreError> {
    Ok(row(store, key)?.map_or(Value::Null, |r| r.value))
}

fn put(store: &mut AppStore, key: &str, value: Value) -> Result<(), StoreError> {
    store.writable()?;
    let account = store.account_id().is_some();
    let stamp = store.stamp()?;
    let row = PreferenceRow {
        key: key.to_owned(),
        value,
        stamp,
        dirty: account,
        rejected: false,
    };
    let result = save(store.owner_conn(), &row);
    store.track(result)
}

/// An explicit write of a key this module keeps (the reading bookmark).
pub fn set(store: &mut AppStore, key: &str, value: Value) -> Result<(), StoreError> {
    put(store, key, value)
}

fn baselined(store: &AppStore) -> Result<bool, StoreError> {
    Ok(kv_get(store.owner_conn(), BASELINED_KEY)?.as_deref() == Some("1"))
}

/// Stamp every source value that differs from its register. Settings entries
/// come from the app (`None` leaves settings keys alone); the home layout and
/// goals are read here. No-op signed out or before the first full pull.
pub fn capture(
    store: &mut AppStore,
    settings: Option<&[(String, Value)]>,
) -> Result<(), StoreError> {
    if store.account_id().is_none() || store.future_schema() || !baselined(store)? {
        return Ok(());
    }
    let mut entries: Vec<(String, Value)> = settings.map(<[_]>::to_vec).unwrap_or_default();
    entries.retain(|(k, _)| k.starts_with(SETTINGS_PREFIX));
    entries.push((
        LAYOUT_KEY.into(),
        json!({ "widgets": home::layout_value(store)? }),
    ));
    entries.push((GOALS_KEY.into(), goals::document(store)?));
    for (key, value) in entries {
        let unchanged = row(store, &key)?.is_some_and(|r| same_value(&r.value, &value));
        if !unchanged {
            put(store, &key, value)?;
        }
    }
    Ok(())
}

/// Captures the store-owned sources (layout, goals) after a local change.
pub(crate) fn capture_documents(store: &mut AppStore) -> Result<(), StoreError> {
    capture(store, None)
}

// --- reading bookmark (F3) ---

/// The page the reader chose to come back to. Nothing in the reader writes
/// it except an explicit set, replace, clear or undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingBookmark {
    pub surah: i64,
    pub ayah: i64,
    pub source_id: Option<String>,
    /// Epoch ms.
    pub set_at: i64,
}

impl ReadingBookmark {
    fn to_json(&self) -> Value {
        json!({ "surah": self.surah, "ayah": self.ayah, "sourceId": self.source_id, "setAt": self.set_at })
    }
}

pub fn reading_bookmark(store: &AppStore) -> Result<Option<ReadingBookmark>, StoreError> {
    let value = get(store, READING_BOOKMARK_KEY)?;
    let (Some(surah), Some(ayah), Some(set_at)) = (
        int_in(&value, "surah", 1, 114),
        int_in(&value, "ayah", 1, 286),
        int_in(&value, "setAt", 0, i64::MAX >> 11),
    ) else {
        return Ok(None);
    };
    Ok(Some(ReadingBookmark {
        surah,
        ayah,
        source_id: opt_str(&value, "sourceId"),
        set_at,
    }))
}

/// Set or replace. Returns what it replaced, for Undo.
pub fn set_reading_bookmark(
    store: &mut AppStore,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
) -> Result<Option<ReadingBookmark>, StoreError> {
    let previous = reading_bookmark(store)?;
    let next = ReadingBookmark {
        surah,
        ayah,
        source_id,
        set_at: store.local_now(),
    };
    put(store, READING_BOOKMARK_KEY, next.to_json())?;
    Ok(previous)
}

/// Clear. Returns what it cleared, for Undo.
pub fn clear_reading_bookmark(store: &mut AppStore) -> Result<Option<ReadingBookmark>, StoreError> {
    let previous = reading_bookmark(store)?;
    put(store, READING_BOOKMARK_KEY, Value::Null)?;
    Ok(previous)
}

/// Undo: the earlier value back with a newer stamp, or a clear when there was none.
pub fn restore_reading_bookmark(
    store: &mut AppStore,
    previous: Option<&ReadingBookmark>,
) -> Result<(), StoreError> {
    put(
        store,
        READING_BOOKMARK_KEY,
        previous.map_or(Value::Null, ReadingBookmark::to_json),
    )
}

// --- settings document (J9) ---

/// One key per setting: `settings.appearance.*` and `settings.reading.*` of a
/// settings document (web `settings-document.ts`). Privacy (consent) never syncs.
pub fn settings_entries(doc: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for section in ["appearance", "reading"] {
        if let Some(fields) = doc.get(section).and_then(Value::as_object) {
            for (field, value) in fields {
                out.push((format!("{SETTINGS_PREFIX}{section}.{field}"), value.clone()));
            }
        }
    }
    out
}

/// Writes arrived `settings.*` values into a settings document. The app's
/// settings store validates and applies the result.
pub fn apply_settings_entries(doc: &mut Value, values: &[(String, Value)]) {
    for (key, value) in values {
        let Some((section, field)) = key
            .strip_prefix(SETTINGS_PREFIX)
            .and_then(|rest| rest.split_once('.'))
        else {
            continue;
        };
        if section != "appearance" && section != "reading" {
            continue;
        }
        if !doc.is_object() {
            *doc = json!({});
        }
        let slot = &mut doc[section];
        if !slot.is_object() {
            *slot = json!({});
        }
        slot[field] = value.clone();
    }
}

// --- sync ---

pub fn pending(store: &AppStore) -> Result<usize, StoreError> {
    if store.account_id().is_none() {
        return Ok(0);
    }
    count_dirty(store.owner_conn(), PreferenceRow::DOMAIN)
}

pub fn rejected_count(store: &AppStore) -> Result<usize, StoreError> {
    count_rejected(store.owner_conn(), PreferenceRow::DOMAIN)
}

/// Rows the next round sends.
pub fn dirty_rows(store: &AppStore) -> Result<Vec<Value>, StoreError> {
    Ok(load_all::<PreferenceRow>(store.owner_conn())?
        .into_iter()
        .filter(|r| r.dirty && !r.rejected)
        .take(SYNC_PAGE)
        .map(|r| json!({ "key": r.key, "value": r.value, "stamp": r.stamp }))
        .collect())
}

/// Apply one round (web `PreferencesStore.applyRound`).
pub(crate) fn apply_round(
    store: &mut AppStore,
    round: &Value,
    sent: &[Value],
) -> Result<RoundApplied, StoreError> {
    let Some(incoming) = round.get("preferences").and_then(Value::as_array) else {
        return Err(StoreError::Unreadable(
            "preference sync returned an unreadable round".into(),
        ));
    };
    store.writable()?;
    let meta = PageMeta::decode(round);
    if let Some(server) = meta.server_time {
        store.on_server_time(server);
    }
    let position = Position::load(store.owner_conn(), SyncDomain::Preferences)?;
    let before: Vec<PreferenceRow> = load_all(store.owner_conn())?;
    let mut rows: std::collections::BTreeMap<String, PreferenceRow> =
        before.iter().map(|r| (r.key.clone(), r.clone())).collect();

    for result in round
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = str_of(result, "id") else {
            continue;
        };
        let sent_row = sent.iter().find(|s| str_of(s, "key") == Some(id));
        let Some(mine) = rows.get_mut(id) else {
            continue;
        };
        if !sent_row.is_some_and(|s| mine.stamp.as_deref() == str_of(s, "stamp")) {
            continue;
        }
        match str_of(result, "status") {
            Some("rejected") => mine.rejected = true,
            Some("capped") => {}
            _ => {
                let returned = result.get("stamps").and_then(|s| str_of(s, "row"));
                if let (Some(r), Some(l)) = (returned, mine.stamp.as_deref())
                    && r < l
                {
                    mine.stamp = Some(r.to_owned());
                }
                mine.dirty = false;
            }
        }
    }
    let mut arrived: Vec<(String, Value)> = Vec::new();
    for raw in incoming {
        let Some(key) = str_of(raw, "key") else {
            continue;
        };
        let stamp = opt_str(raw, "stamp");
        store.observe(stamp.as_deref());
        let mine_wins = rows
            .get(key)
            .is_some_and(|mine| compare_stamps(mine.stamp.as_deref(), stamp.as_deref()).is_ge());
        if mine_wins {
            continue;
        }
        let value = raw.get("value").cloned().unwrap_or(Value::Null);
        rows.insert(
            key.to_owned(),
            PreferenceRow {
                key: key.to_owned(),
                value: value.clone(),
                stamp,
                dirty: false,
                rejected: false,
            },
        );
        arrived.push((key.to_owned(), value));
    }

    let restored = (position.epoch.is_some() && meta.sync_epoch != position.epoch)
        || position.since > meta.latest_rev;
    if restored {
        rows.values_mut().for_each(|r| r.dirty = r.stamp.is_some());
    }
    let next = Position {
        since: if restored { 0 } else { meta.rev.max(0) },
        after: -1,
        epoch: meta.sync_epoch.clone().or(position.epoch.clone()),
    };
    let first_baseline = !meta.more && !restored && !baselined(store)?;
    let after: Vec<PreferenceRow> = rows.into_values().collect();
    let result = in_tx(store.owner_conn(), |tx| {
        save_diff(tx, &before, &after)?;
        next.save(tx, SyncDomain::Preferences)?;
        if first_baseline {
            kv_set(tx, BASELINED_KEY, "1")?;
        }
        Ok(())
    });
    store.track(result)?;
    store.persist_clock()?;
    store.mark_seen()?;

    // Show what arrived: goals and the layout here, settings through the app.
    let mut settings = Vec::new();
    for (key, value) in arrived {
        match key.as_str() {
            GOALS_KEY => goals::replace(store, &value)?,
            LAYOUT_KEY => home::replace_layout(store, &value)?,
            k if k.starts_with(SETTINGS_PREFIX) => settings.push((key, value)),
            _ => {}
        }
    }
    if first_baseline {
        capture_documents(store)?;
        store.sync_memory.settings_capture_due = true;
    }
    let dirty_left = !dirty_rows(store)?.is_empty();
    // The baseline capture's rows go in this flush (web: capture → flush).
    let progressed = dirty_left && (sent.len() >= SYNC_PAGE || first_baseline);
    Ok(RoundApplied {
        more: meta.more || restored || progressed,
        settings,
    })
}

/// Sign-in: the signed-out reading bookmark joins the account with its own time.
pub(crate) fn absorb_local(store: &mut AppStore) -> Result<(), StoreError> {
    let local: Vec<PreferenceRow> = load_all(store.local_conn())?;
    let Some(bookmark) = local
        .into_iter()
        .find(|r| r.key == READING_BOOKMARK_KEY && !r.value.is_null())
    else {
        return clear_local(store);
    };
    let set_at = int_in(&bookmark.value, "setAt", 0, i64::MAX >> 11).unwrap_or(0);
    let stamp = store.stamp_at(set_at)?;
    let row = PreferenceRow {
        stamp,
        dirty: true,
        ..bookmark
    };
    let mine = super::rows::get(
        store.owner_conn(),
        PreferenceRow::DOMAIN,
        READING_BOOKMARK_KEY,
    )?
    .and_then(|r| PreferenceRow::decode(&r));
    if mine.is_none_or(|m| compare_stamps(m.stamp.as_deref(), row.stamp.as_deref()).is_lt()) {
        let result = save(store.owner_conn(), &row);
        store.track(result)?;
    }
    clear_local(store)
}

fn clear_local(store: &mut AppStore) -> Result<(), StoreError> {
    let result = delete_domain(store.local_conn(), PreferenceRow::DOMAIN);
    store.track(result)
}

/// Whether a stored flag is set (helper for tests).
#[cfg(test)]
pub(crate) fn is_baselined(store: &AppStore) -> bool {
    baselined(store).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::memory_store;

    #[test]
    fn settings_entries_round_trip_and_skip_privacy() {
        let doc = json!({
            "v": 1,
            "appearance": {"theme": "dark"},
            "reading": {"fontSize": 32},
            "privacy": {"analytics": true},
        });
        let entries = settings_entries(&doc);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|(k, _)| !k.contains("privacy")));
        let mut other = json!({ "appearance": {"theme": "light"} });
        apply_settings_entries(&mut other, &entries);
        assert_eq!(other["appearance"]["theme"], "dark");
        assert_eq!(other["reading"]["fontSize"], 32);
    }

    #[test]
    fn reading_bookmark_set_undo_and_sign_in_fold() {
        let (mut store, clock) = memory_store(1);
        assert_eq!(
            set_reading_bookmark(&mut store, 18, 10, None).unwrap(),
            None
        );
        let first = reading_bookmark(&store).unwrap().unwrap();
        clock.advance(10);
        let replaced = set_reading_bookmark(&mut store, 36, 1, None).unwrap();
        assert_eq!(replaced.as_ref(), Some(&first));
        restore_reading_bookmark(&mut store, replaced.as_ref()).unwrap();
        assert_eq!(reading_bookmark(&store).unwrap().unwrap().surah, 18);
        store.sign_in(3, "dev").unwrap();
        let row = row(&store, READING_BOOKMARK_KEY).unwrap().unwrap();
        assert!(row.dirty);
        assert!(row.stamp.is_some());
        assert!(!is_baselined(&store));
    }
}
