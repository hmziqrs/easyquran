//! Bookmarks with folders (PARITY F1; web `lib/bookmarks/{schema,store.svelte}.ts`).
//!
//! Signed out, the device owner's folders and bookmarks live in the local
//! file and every edit writes through. Signed in, the row is the queue: each
//! edit stamps its register and persists `dirty` with the row; a round sends
//! the dirty rows and merges what it pulls. Only a tombstone deletes.
//! Every owner has a default folder whose id is a UUID v5 the server derives
//! the same way.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
};

use serde_json::{Map, Value, json};

use super::{
    AppStore, StoreError,
    position::{PageMeta, PageVerdict, Position},
    rows::{
        RowModel, RowRecord, bool_true, count_dirty, count_rejected, delete, delete_domain,
        extras_of, in_tx, int_in, kv_get, kv_set, load_all, opt_json, opt_str, save, save_diff,
        str_of,
    },
};
use crate::{
    merge::{compare_stamps, merge_bookmark, merge_folder},
    sync::{
        RoundApplied, SyncDomain, clock::ms_from_iso, ids::uuid_v5, round_status::merged_dirty,
    },
};

pub const SURAH_MAX: i64 = 114;
/// Longest surah (Al-Baqarah).
pub const AYAH_MAX: i64 = 286;
pub const FOLDER_NAME_MAX: usize = 100;
/// Seed name of every default folder (the user may rename it). // i18n: bookmarks default folder
pub const DEFAULT_FOLDER_NAME: &str = "Bookmarks";
/// Byte-for-byte the server's `DEFAULT_FOLDER_NAMESPACE`.
const DEFAULT_FOLDER_NAMESPACE: &str = "6f0e4c1a-2b7d-4e59-9a83-5c1d2e3f4a5b";
const SYNC_PAGE: usize = 200;
/// A device may hold this many clean tombstones.
const TOMBSTONE_CAP: usize = 500;
const EPOCH_ISO: &str = "1970-01-01T00:00:00.000Z";
const LAST_FOLDER_KEY: &str = "bookmarks.lastFolder";
pub const SYNC_PATH: &str = "/bookmark/v1/sync";
pub const MISMATCH_MESSAGE: &str = "Bookmark sync account changed";

/// Default folder id: `user:{id}` for an account, `local` for the device owner.
pub fn default_folder_id_for(owner: Option<i64>) -> String {
    match owner {
        Some(id) => uuid_v5(DEFAULT_FOLDER_NAMESPACE, &format!("user:{id}")),
        None => uuid_v5(DEFAULT_FOLDER_NAMESPACE, "local"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BookmarkFolder {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    /// Manual order; `None` sorts after positioned folders.
    pub position: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub name_stamp: Option<String>,
    pub position_stamp: Option<String>,
    pub deleted_stamp: Option<String>,
    pub deleted: bool,
    pub dirty: bool,
    pub rejected: bool,
    /// Fields this build does not know, kept for whoever does.
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    pub id: String,
    pub folder_id: String,
    pub surah: i64,
    pub ayah: i64,
    /// Range end; `None` for a single verse.
    pub end_surah: Option<i64>,
    pub end_ayah: Option<i64>,
    /// Saved translation; `None` falls back to Arabic.
    pub source_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub stamp: Option<String>,
    pub deleted: bool,
    pub dirty: bool,
    pub rejected: bool,
    pub extra: Map<String, Value>,
}

const FOLDER_FIELDS: &[&str] = &[
    "id",
    "name",
    "isDefault",
    "position",
    "createdAt",
    "updatedAt",
    "nameStamp",
    "positionStamp",
    "deletedStamp",
    "deleted",
    "dirty",
    "rejected",
    "rev",
    "kind",
];
const BOOKMARK_FIELDS: &[&str] = &[
    "id",
    "folderId",
    "surah",
    "ayah",
    "endSurah",
    "endAyah",
    "sourceId",
    "createdAt",
    "updatedAt",
    "stamp",
    "deleted",
    "dirty",
    "rejected",
    "rev",
    "kind",
];

/// A folder row (wire or storage); `None` for anything malformed.
pub fn decode_folder(v: &Value) -> Option<BookmarkFolder> {
    let id = str_of(v, "id").filter(|id| id.len() == 36)?;
    let name =
        str_of(v, "name").filter(|n| !n.is_empty() && n.chars().count() <= FOLDER_NAME_MAX)?;
    let updated_at = opt_str(v, "updatedAt")?;
    Some(BookmarkFolder {
        id: id.to_owned(),
        name: name.to_owned(),
        is_default: bool_true(v, "isDefault"),
        position: int_in(v, "position", -1_000_000, 1_000_000),
        created_at: opt_str(v, "createdAt").unwrap_or_else(|| updated_at.clone()),
        updated_at,
        name_stamp: opt_str(v, "nameStamp"),
        position_stamp: opt_str(v, "positionStamp"),
        deleted_stamp: opt_str(v, "deletedStamp"),
        deleted: bool_true(v, "deleted"),
        dirty: bool_true(v, "dirty"),
        rejected: bool_true(v, "rejected"),
        extra: extras_of(v, FOLDER_FIELDS),
    })
}

/// A bookmark row (wire or storage). A tombstone carries its key and stamp only.
pub fn decode_bookmark(v: &Value) -> Option<Bookmark> {
    let id = str_of(v, "id").filter(|id| id.len() == 36)?;
    let folder_id = str_of(v, "folderId").filter(|id| id.len() == 36)?;
    let surah = int_in(v, "surah", 1, SURAH_MAX)?;
    let ayah = int_in(v, "ayah", 1, AYAH_MAX)?;
    let deleted = bool_true(v, "deleted");
    let updated_at = match opt_str(v, "updatedAt") {
        Some(at) => at,
        None if deleted => EPOCH_ISO.to_owned(),
        None => return None,
    };
    let end_surah = int_in(v, "endSurah", 1, SURAH_MAX);
    let end_ayah = int_in(v, "endAyah", 1, AYAH_MAX);
    if end_surah.is_some() != end_ayah.is_some() {
        return None;
    }
    let single = end_surah == Some(surah) && end_ayah == Some(ayah);
    let (end_surah, end_ayah) = if single {
        (None, None)
    } else {
        (end_surah, end_ayah)
    };
    Some(Bookmark {
        id: id.to_owned(),
        folder_id: folder_id.to_owned(),
        surah,
        ayah,
        end_surah,
        end_ayah,
        source_id: opt_str(v, "sourceId"),
        created_at: opt_str(v, "createdAt").unwrap_or_else(|| updated_at.clone()),
        updated_at,
        stamp: opt_str(v, "stamp"),
        deleted,
        dirty: bool_true(v, "dirty"),
        rejected: bool_true(v, "rejected"),
        extra: extras_of(v, BOOKMARK_FIELDS),
    })
}

impl RowModel for BookmarkFolder {
    const DOMAIN: &'static str = "folder";
    fn id(&self) -> &str {
        &self.id
    }
    fn decode(record: &RowRecord) -> Option<Self> {
        decode_folder(&record.row).map(|f| BookmarkFolder {
            dirty: record.dirty,
            rejected: record.rejected,
            ..f
        })
    }
    fn encode(&self) -> RowRecord {
        let mut row = Value::Object(self.extra.clone());
        if let Some(o) = row.as_object_mut() {
            o.extend(
                json!({
                    "id": self.id, "name": self.name, "isDefault": self.is_default,
                    "position": self.position, "createdAt": self.created_at,
                    "updatedAt": self.updated_at, "nameStamp": self.name_stamp,
                    "positionStamp": self.position_stamp, "deletedStamp": self.deleted_stamp,
                    "deleted": self.deleted,
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            );
        }
        RowRecord {
            id: self.id.clone(),
            row,
            stamps: folder_stamps(self),
            dirty: self.dirty,
            rejected: self.rejected,
        }
    }
}

impl RowModel for Bookmark {
    const DOMAIN: &'static str = "bookmark";
    fn id(&self) -> &str {
        &self.id
    }
    fn decode(record: &RowRecord) -> Option<Self> {
        decode_bookmark(&record.row).map(|b| Bookmark {
            dirty: record.dirty,
            rejected: record.rejected,
            ..b
        })
    }
    fn encode(&self) -> RowRecord {
        let mut row = Value::Object(self.extra.clone());
        if let Some(o) = row.as_object_mut() {
            o.extend(
                json!({
                    "id": self.id, "folderId": self.folder_id, "surah": self.surah,
                    "ayah": self.ayah, "endSurah": self.end_surah, "endAyah": self.end_ayah,
                    "sourceId": self.source_id, "createdAt": self.created_at,
                    "updatedAt": self.updated_at, "stamp": self.stamp, "deleted": self.deleted,
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            );
        }
        RowRecord {
            id: self.id.clone(),
            row,
            stamps: vec![self.stamp.clone()],
            dirty: self.dirty,
            rejected: self.rejected,
        }
    }
}

fn folder_stamps(f: &BookmarkFolder) -> Vec<Option<String>> {
    vec![
        f.name_stamp.clone(),
        f.position_stamp.clone(),
        f.deleted_stamp.clone(),
    ]
}

/// `S:A-S:A`: start verse plus range end.
pub fn range_key(b: &Bookmark) -> String {
    format!(
        "{}:{}-{}:{}",
        b.surah,
        b.ayah,
        b.end_surah.unwrap_or(b.surah),
        b.end_ayah.unwrap_or(b.ayah)
    )
}

/// Server-identical live key: folder + range.
pub fn live_key(b: &Bookmark) -> String {
    format!("{}/{}", b.folder_id, range_key(b))
}

/// True when the row covers exactly one ayah.
pub fn is_single_verse(b: &Bookmark) -> bool {
    b.end_surah.unwrap_or(b.surah) == b.surah && b.end_ayah.unwrap_or(b.ayah) == b.ayah
}

/// Default first, then manual position (unpositioned last), then name.
pub fn compare_folders(a: &BookmarkFolder, b: &BookmarkFolder) -> Ordering {
    b.is_default
        .cmp(&a.is_default)
        .then_with(|| {
            a.position
                .unwrap_or(i64::MAX)
                .cmp(&b.position.unwrap_or(i64::MAX))
        })
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.name.cmp(&b.name))
}

fn seed_default(id: String) -> BookmarkFolder {
    BookmarkFolder {
        id,
        name: DEFAULT_FOLDER_NAME.into(),
        is_default: true,
        position: None,
        created_at: EPOCH_ISO.into(),
        updated_at: EPOCH_ISO.into(),
        name_stamp: None,
        position_stamp: None,
        deleted_stamp: None,
        deleted: false,
        dirty: false,
        rejected: false,
        extra: Map::new(),
    }
}

fn valid_folder_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    (!trimmed.is_empty() && trimmed.chars().count() <= FOLDER_NAME_MAX).then(|| trimmed.to_owned())
}

/// A stamp the server rewrote is below ours: adopt it. One it beat arrives with the row.
fn lower_of(result: Option<&str>, local: &Option<String>) -> Option<String> {
    match (result, local) {
        (Some(r), Some(l)) if r < l.as_str() => Some(r.to_owned()),
        _ => local.clone(),
    }
}

fn bookmark_registers(b: &Bookmark) -> Value {
    json!({
        "id": b.id,
        "register": {
            "stamp": b.stamp,
            "value": {
                "context": { "sourceId": b.source_id },
                "deleted": b.deleted,
                "folderId": b.folder_id,
                "key": range_key(b),
            },
        },
    })
}

/// Whole-row register: the winning side's row, with flags merged.
pub(crate) fn merge_bookmark_row(a: &Bookmark, b: &Bookmark) -> Bookmark {
    let (ra, rb) = (bookmark_registers(a), bookmark_registers(b));
    let merged = merge_bookmark(&ra, &rb);
    let winner = if merged == ra { a } else { b };
    let dirty = merged_dirty(
        (a.dirty, &[a.stamp.clone()]),
        (b.dirty, &[b.stamp.clone()]),
        &[winner.stamp.clone()],
    );
    Bookmark {
        dirty,
        rejected: a.rejected && b.rejected,
        ..winner.clone()
    }
}

fn folder_registers(f: &BookmarkFolder) -> Value {
    json!({
        "id": f.id,
        "isDefault": f.is_default,
        "name": { "stamp": f.name_stamp, "value": f.name },
        "position": { "stamp": f.position_stamp, "value": f.position },
        "deleted": { "stamp": f.deleted_stamp, "value": f.deleted },
    })
}

/// Name, position and deleted each merge alone.
pub(crate) fn merge_folder_row(a: &BookmarkFolder, b: &BookmarkFolder) -> BookmarkFolder {
    let merged = merge_folder(&folder_registers(a), &folder_registers(b));
    let stamp = |k: &str| opt_str(&merged[k], "stamp");
    let value = |k: &str| merged[k].get("value").cloned().unwrap_or(Value::Null);
    let (name_stamp, position_stamp, deleted_stamp) =
        (stamp("name"), stamp("position"), stamp("deleted"));
    let mut extra = a.extra.clone();
    extra.extend(b.extra.clone());
    let merged_stamps = [
        name_stamp.clone(),
        position_stamp.clone(),
        deleted_stamp.clone(),
    ];
    BookmarkFolder {
        name: value("name")
            .as_str()
            .map_or_else(|| a.name.clone(), str::to_owned),
        position: value("position")
            .as_i64()
            .filter(|p| (-1_000_000..=1_000_000).contains(p)),
        deleted: value("deleted").as_bool() == Some(true),
        is_default: a.is_default || b.is_default,
        updated_at: a.updated_at.clone().max(b.updated_at.clone()),
        dirty: merged_dirty(
            (a.dirty, &folder_stamps(a)),
            (b.dirty, &folder_stamps(b)),
            &merged_stamps,
        ),
        rejected: a.rejected && b.rejected,
        name_stamp,
        position_stamp,
        deleted_stamp,
        extra,
        ..a.clone()
    }
}

// --- reads ---

fn all_folders(store: &AppStore) -> Result<Vec<BookmarkFolder>, StoreError> {
    load_all(store.owner_conn())
}

fn all_bookmarks(store: &AppStore) -> Result<Vec<Bookmark>, StoreError> {
    load_all(store.owner_conn())
}

pub fn default_folder_id(store: &AppStore) -> String {
    default_folder_id_for(store.account_id())
}

/// Live folders, default first then manual order. Always holds the default folder.
pub fn folders(store: &AppStore) -> Result<Vec<BookmarkFolder>, StoreError> {
    let default_id = default_folder_id(store);
    let mut live: Vec<BookmarkFolder> = all_folders(store)?
        .into_iter()
        .filter(|f| !f.deleted)
        .collect();
    if !live.iter().any(|f| f.id == default_id) {
        live.push(seed_default(default_id));
    }
    live.sort_by(compare_folders);
    Ok(live)
}

fn outranks(a: &Bookmark, b: &Bookmark) -> bool {
    match compare_stamps(a.stamp.as_deref(), b.stamp.as_deref()) {
        Ordering::Equal => a.updated_at > b.updated_at,
        order => order.is_gt(),
    }
}

/// Live bookmarks, newest first, with the set rules applied to the view: an
/// older bookmark dies with a deleted folder and a newer one shows in the
/// default folder; one key shows one row, the highest stamp.
pub fn bookmarks(store: &AppStore) -> Result<Vec<Bookmark>, StoreError> {
    let default_id = default_folder_id(store);
    let deleted_folders: HashMap<String, Option<String>> = all_folders(store)?
        .into_iter()
        .filter(|f| f.deleted)
        .map(|f| (f.id, f.deleted_stamp))
        .collect();
    let mut by_key: BTreeMap<String, Bookmark> = BTreeMap::new();
    for stored in all_bookmarks(store)? {
        if stored.deleted {
            continue;
        }
        let row = match deleted_folders.get(&stored.folder_id) {
            None => stored,
            Some(stamp) => {
                if compare_stamps(stored.stamp.as_deref(), stamp.as_deref()).is_le() {
                    continue;
                }
                Bookmark {
                    folder_id: default_id.clone(),
                    ..stored
                }
            }
        };
        let key = live_key(&row);
        let keep = by_key.get(&key).is_none_or(|prev| outranks(&row, prev));
        if keep {
            by_key.insert(key, row);
        }
    }
    let mut out: Vec<Bookmark> = by_key.into_values().collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

pub fn folder_by_id(store: &AppStore, id: &str) -> Result<Option<BookmarkFolder>, StoreError> {
    Ok(folders(store)?.into_iter().find(|f| f.id == id))
}

/// Where a new tap saves: the last folder used, else the default.
pub fn target_folder(store: &AppStore) -> Result<BookmarkFolder, StoreError> {
    let list = folders(store)?;
    let last = kv_get(store.owner_conn(), LAST_FOLDER_KEY)?;
    let found = last
        .and_then(|id| list.iter().find(|f| f.id == id).cloned())
        .or_else(|| list.iter().find(|f| f.is_default).cloned())
        .or_else(|| list.first().cloned());
    Ok(found.unwrap_or_else(|| seed_default(default_folder_id(store))))
}

pub fn bookmarks_in(store: &AppStore, folder_id: &str) -> Result<Vec<Bookmark>, StoreError> {
    Ok(bookmarks(store)?
        .into_iter()
        .filter(|b| b.folder_id == folder_id)
        .collect())
}

pub fn count_in(store: &AppStore, folder_id: &str) -> Result<usize, StoreError> {
    Ok(bookmarks_in(store, folder_id)?.len())
}

/// Single-verse rows for this verse, across every folder.
pub fn rows_for(store: &AppStore, surah: i64, ayah: i64) -> Result<Vec<Bookmark>, StoreError> {
    Ok(bookmarks(store)?
        .into_iter()
        .filter(|b| is_single_verse(b) && b.surah == surah && b.ayah == ayah)
        .collect())
}

pub fn is_bookmarked(store: &AppStore, surah: i64, ayah: i64) -> Result<bool, StoreError> {
    Ok(!rows_for(store, surah, ayah)?.is_empty())
}

/// Folder ids holding this verse.
pub fn folders_of(store: &AppStore, surah: i64, ayah: i64) -> Result<HashSet<String>, StoreError> {
    Ok(rows_for(store, surah, ayah)?
        .into_iter()
        .map(|b| b.folder_id)
        .collect())
}

fn same_range(b: &Bookmark, surah: i64, ayah: i64, end_surah: i64, end_ayah: i64) -> bool {
    b.surah == surah
        && b.ayah == ayah
        && b.end_surah.unwrap_or(b.surah) == end_surah
        && b.end_ayah.unwrap_or(b.ayah) == end_ayah
}

pub fn is_range_marked(
    store: &AppStore,
    surah: i64,
    ayah: i64,
    end_surah: i64,
    end_ayah: i64,
) -> Result<bool, StoreError> {
    Ok(bookmarks(store)?
        .iter()
        .any(|b| same_range(b, surah, ayah, end_surah, end_ayah)))
}

/// Rows waiting to reach the server (0 signed out).
pub fn pending(store: &AppStore) -> Result<usize, StoreError> {
    if store.account_id().is_none() {
        return Ok(0);
    }
    let conn = store.owner_conn();
    Ok(count_dirty(conn, BookmarkFolder::DOMAIN)? + count_dirty(conn, Bookmark::DOMAIN)?)
}

/// Rows the server refused, kept here and not resent until edited.
pub fn rejected_count(store: &AppStore) -> Result<usize, StoreError> {
    let conn = store.owner_conn();
    Ok(count_rejected(conn, BookmarkFolder::DOMAIN)? + count_rejected(conn, Bookmark::DOMAIN)?)
}

/// Rows waiting for room under the account's quota.
pub fn capped_count(store: &AppStore) -> usize {
    store
        .sync_memory
        .capped
        .get(&SyncDomain::Bookmarks)
        .map_or(0, HashSet::len)
}

// --- writes ---

/// What a verse toggle did: the UI shows "Saved to …" or an Undo for the removal.
#[derive(Debug, Clone, PartialEq)]
pub enum ToggleResult {
    Added {
        folder: BookmarkFolder,
        bookmark: Bookmark,
    },
    Removed {
        bookmarks: Vec<Bookmark>,
    },
}

/// Everything a folder delete took with it, for Undo.
#[derive(Debug, Clone, PartialEq)]
pub struct DeletedFolder {
    pub folder: BookmarkFolder,
    pub bookmarks: Vec<Bookmark>,
}

/// What an import added.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportCounts {
    pub folders: usize,
    pub bookmarks: usize,
}

fn put_bookmark(store: &mut AppStore, row: Bookmark) -> Result<Bookmark, StoreError> {
    store.writable()?;
    if store.account_id().is_none() {
        let conn = store.owner_conn();
        let others = load_all::<Bookmark>(conn)?;
        let key = live_key(&row);
        let result = in_tx(conn, |tx| {
            for other in others
                .iter()
                .filter(|b| b.id == row.id || live_key(b) == key)
            {
                delete(tx, Bookmark::DOMAIN, &other.id)?;
            }
            save(tx, &row)
        });
        store.track(result)?;
        return Ok(row);
    }
    let stamp = store.stamp()?;
    let stamped = Bookmark {
        stamp: stamp.or(row.stamp),
        dirty: true,
        rejected: false,
        ..row
    };
    let result = save(store.owner_conn(), &stamped);
    store.track(result)?;
    Ok(stamped)
}

fn delete_bookmark(store: &mut AppStore, id: &str) -> Result<(), StoreError> {
    store.writable()?;
    if store.account_id().is_none() {
        let result = delete(store.owner_conn(), Bookmark::DOMAIN, id);
        return store.track(result);
    }
    let Some(record) = super::rows::get(store.owner_conn(), Bookmark::DOMAIN, id)? else {
        return Ok(());
    };
    let Some(row) = Bookmark::decode(&record) else {
        return Ok(());
    };
    let updated_at = store.now_iso();
    put_bookmark(
        store,
        Bookmark {
            deleted: true,
            updated_at,
            ..row
        },
    )?;
    Ok(())
}

/// Only a register whose value changed gets a new stamp.
fn put_folder(store: &mut AppStore, folder: BookmarkFolder) -> Result<BookmarkFolder, StoreError> {
    store.writable()?;
    if store.account_id().is_none() {
        let result = save(store.owner_conn(), &folder);
        store.track(result)?;
        return Ok(folder);
    }
    let prev = super::rows::get(store.owner_conn(), BookmarkFolder::DOMAIN, &folder.id)?
        .and_then(|r| BookmarkFolder::decode(&r));
    let name_changed = prev.as_ref().is_none_or(|p| p.name != folder.name);
    let position_changed = prev.as_ref().is_some_and(|p| p.position != folder.position);
    let deleted_changed = prev.as_ref().is_some_and(|p| p.deleted != folder.deleted);
    let mut stamped = folder;
    if name_changed {
        stamped.name_stamp = store.stamp()?.or(stamped.name_stamp);
    }
    if position_changed {
        stamped.position_stamp = store.stamp()?.or(stamped.position_stamp);
    }
    if deleted_changed {
        stamped.deleted_stamp = store.stamp()?.or(stamped.deleted_stamp);
    }
    stamped.dirty = true;
    stamped.rejected = false;
    let result = save(store.owner_conn(), &stamped);
    store.track(result)?;
    Ok(stamped)
}

fn remember_folder(store: &mut AppStore, id: Option<&str>) -> Result<(), StoreError> {
    let result = kv_set(store.writable()?, LAST_FOLDER_KEY, id.unwrap_or(""));
    store.track(result)
}

fn add_bookmark(
    store: &mut AppStore,
    folder_id: &str,
    range: (i64, i64, Option<i64>, Option<i64>),
    source_id: Option<String>,
) -> Result<Bookmark, StoreError> {
    let now = store.now_iso();
    let id = store.new_id();
    let (surah, ayah, end_surah, end_ayah) = range;
    put_bookmark(
        store,
        Bookmark {
            id,
            folder_id: folder_id.to_owned(),
            surah,
            ayah,
            end_surah,
            end_ayah,
            source_id,
            created_at: now.clone(),
            updated_at: now,
            stamp: None,
            deleted: false,
            dirty: false,
            rejected: false,
            extra: Map::new(),
        },
    )
}

fn valid_verse(surah: i64, ayah: i64) -> bool {
    (1..=SURAH_MAX).contains(&surah) && (1..=AYAH_MAX).contains(&ayah)
}

/// Verse tap: unsaved → save to the target folder; saved anywhere → remove from every folder.
pub fn toggle(
    store: &mut AppStore,
    surah: i64,
    ayah: i64,
    source_id: Option<String>,
) -> Result<ToggleResult, StoreError> {
    store.writable()?;
    if !valid_verse(surah, ayah) {
        return Ok(ToggleResult::Removed {
            bookmarks: Vec::new(),
        });
    }
    let existing = rows_for(store, surah, ayah)?;
    if !existing.is_empty() {
        for row in &existing {
            delete_bookmark(store, &row.id)?;
        }
        return Ok(ToggleResult::Removed {
            bookmarks: existing,
        });
    }
    let folder = target_folder(store)?;
    let bookmark = add_bookmark(store, &folder.id, (surah, ayah, None, None), source_id)?;
    Ok(ToggleResult::Added { folder, bookmark })
}

/// "Save to…" checkbox: put this verse in (or take it out of) one folder.
pub fn set_in_folder(
    store: &mut AppStore,
    surah: i64,
    ayah: i64,
    folder_id: &str,
    on: bool,
    source_id: Option<String>,
) -> Result<(), StoreError> {
    if folder_by_id(store, folder_id)?.is_none() || !valid_verse(surah, ayah) {
        return Ok(());
    }
    let row = rows_for(store, surah, ayah)?
        .into_iter()
        .find(|b| b.folder_id == folder_id);
    match (on, row) {
        (true, None) => {
            add_bookmark(store, folder_id, (surah, ayah, None, None), source_id)?;
            remember_folder(store, Some(folder_id))
        }
        (false, Some(row)) => delete_bookmark(store, &row.id),
        _ => Ok(()),
    }
}

/// Range bookmark (snippets): toggles in the default folder.
pub fn toggle_range(
    store: &mut AppStore,
    surah: i64,
    ayah: i64,
    end_surah: i64,
    end_ayah: i64,
) -> Result<(), StoreError> {
    let existing: Vec<Bookmark> = bookmarks(store)?
        .into_iter()
        .filter(|b| same_range(b, surah, ayah, end_surah, end_ayah))
        .collect();
    if !existing.is_empty() {
        for row in existing {
            delete_bookmark(store, &row.id)?;
        }
        return Ok(());
    }
    let default_id = default_folder_id(store);
    add_bookmark(
        store,
        &default_id,
        (surah, ayah, Some(end_surah), Some(end_ayah)),
        None,
    )?;
    Ok(())
}

pub fn remove(store: &mut AppStore, bookmark_id: &str) -> Result<Option<Bookmark>, StoreError> {
    let row = bookmarks(store)?.into_iter().find(|b| b.id == bookmark_id);
    if let Some(row) = &row {
        delete_bookmark(store, &row.id)?;
    }
    Ok(row)
}

/// Bulk remove. Returns the rows as they were, for [`restore`] on Undo.
pub fn remove_many(store: &mut AppStore, ids: &[String]) -> Result<Vec<Bookmark>, StoreError> {
    let mut removed = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        if seen.insert(id.clone())
            && let Some(row) = remove(store, id)?
        {
            removed.push(row);
        }
    }
    Ok(removed)
}

/// Bulk move. Rows already in the target are skipped. Returns rows as they were.
pub fn move_many(
    store: &mut AppStore,
    ids: &[String],
    folder_id: &str,
) -> Result<Vec<Bookmark>, StoreError> {
    if folder_by_id(store, folder_id)?.is_none() {
        return Ok(Vec::new());
    }
    let mut before = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(row) = bookmarks(store)?.into_iter().find(|b| &b.id == id) else {
            continue;
        };
        if row.folder_id == folder_id {
            continue;
        }
        move_to_folder(store, &row.id, folder_id)?;
        before.push(row);
    }
    Ok(before)
}

/// Undo for removals and moves: same ids, fresh times, newer stamps than the tombstones.
pub fn restore(store: &mut AppStore, rows: &[Bookmark]) -> Result<(), StoreError> {
    for row in rows {
        let updated_at = store.now_iso();
        put_bookmark(
            store,
            Bookmark {
                deleted: false,
                updated_at,
                ..row.clone()
            },
        )?;
    }
    Ok(())
}

pub fn move_to_folder(
    store: &mut AppStore,
    bookmark_id: &str,
    folder_id: &str,
) -> Result<(), StoreError> {
    let live = bookmarks(store)?;
    let Some(row) = live.iter().find(|b| b.id == bookmark_id).cloned() else {
        return Ok(());
    };
    if row.folder_id == folder_id || folder_by_id(store, folder_id)?.is_none() {
        return Ok(());
    }
    let moved = Bookmark {
        folder_id: folder_id.to_owned(),
        ..row.clone()
    };
    let twin = live
        .iter()
        .any(|b| b.folder_id == folder_id && live_key(b) == live_key(&moved));
    if twin {
        delete_bookmark(store, &row.id)?;
    } else {
        let updated_at = store.now_iso();
        put_bookmark(
            store,
            Bookmark {
                updated_at,
                ..moved
            },
        )?;
    }
    remember_folder(store, Some(folder_id))
}

pub fn create_folder(
    store: &mut AppStore,
    name: &str,
) -> Result<Option<BookmarkFolder>, StoreError> {
    let Some(name) = valid_folder_name(name) else {
        return Ok(None);
    };
    let now = store.now_iso();
    let id = store.new_id();
    let folder = BookmarkFolder {
        name,
        created_at: now.clone(),
        updated_at: now,
        is_default: false,
        ..seed_default(id)
    };
    Ok(Some(put_folder(store, folder)?))
}

pub fn rename_folder(store: &mut AppStore, id: &str, name: &str) -> Result<(), StoreError> {
    let (Some(name), Some(folder)) = (valid_folder_name(name), folder_by_id(store, id)?) else {
        return Ok(());
    };
    if folder.name == name {
        return Ok(());
    }
    let updated_at = store.now_iso();
    put_folder(
        store,
        BookmarkFolder {
            name,
            updated_at,
            ..folder
        },
    )?;
    Ok(())
}

pub fn set_folder_position(
    store: &mut AppStore,
    id: &str,
    position: Option<i64>,
) -> Result<(), StoreError> {
    let Some(folder) = folder_by_id(store, id)? else {
        return Ok(());
    };
    if folder.position == position {
        return Ok(());
    }
    let updated_at = store.now_iso();
    put_folder(
        store,
        BookmarkFolder {
            position,
            updated_at,
            ..folder
        },
    )?;
    Ok(())
}

/// Delete a folder and every bookmark in it (never the default). Returns what went, for Undo.
pub fn delete_folder(store: &mut AppStore, id: &str) -> Result<Option<DeletedFolder>, StoreError> {
    store.writable()?;
    let Some(folder) = folder_by_id(store, id)? else {
        return Ok(None);
    };
    if folder.is_default {
        return Ok(None);
    }
    let in_folder = bookmarks_in(store, id)?;
    if store.account_id().is_none() {
        let conn = store.owner_conn();
        let all = load_all::<Bookmark>(conn)?;
        let result = in_tx(conn, |tx| {
            delete(tx, BookmarkFolder::DOMAIN, id)?;
            for b in all.iter().filter(|b| b.folder_id == id) {
                delete(tx, Bookmark::DOMAIN, &b.id)?;
            }
            Ok(())
        });
        store.track(result)?;
    } else {
        // Rows in it follow the folder's stamp, here and on the server.
        let updated_at = store.now_iso();
        put_folder(
            store,
            BookmarkFolder {
                deleted: true,
                updated_at,
                ..folder.clone()
            },
        )?;
    }
    if kv_get(store.owner_conn(), LAST_FOLDER_KEY)?.as_deref() == Some(id) {
        remember_folder(store, None)?;
    }
    Ok(Some(DeletedFolder {
        folder,
        bookmarks: in_folder,
    }))
}

pub fn restore_folder(store: &mut AppStore, deleted: &DeletedFolder) -> Result<(), StoreError> {
    let updated_at = store.now_iso();
    put_folder(
        store,
        BookmarkFolder {
            deleted: false,
            updated_at,
            ..deleted.folder.clone()
        },
    )?;
    restore(store, &deleted.bookmarks)
}

/// Import (F6). Only ids this store lacks are added, each through the normal
/// write path (fresh stamps and `dirty`). A folder id this store lacks (or
/// has deleted) files into the default folder; a verse already live in that
/// folder is skipped.
pub fn import_rows(
    store: &mut AppStore,
    incoming_folders: &[BookmarkFolder],
    incoming: &[Bookmark],
) -> Result<ImportCounts, StoreError> {
    store.writable()?;
    let mut counts = ImportCounts::default();
    for folder in incoming_folders {
        let known = all_folders(store)?.iter().any(|f| f.id == folder.id);
        let Some(name) = valid_folder_name(&folder.name) else {
            continue;
        };
        if folder.is_default || known {
            continue;
        }
        let updated_at = store.now_iso();
        put_folder(
            store,
            BookmarkFolder {
                name,
                deleted: false,
                name_stamp: None,
                position_stamp: None,
                deleted_stamp: None,
                updated_at,
                dirty: false,
                rejected: false,
                position: None,
                ..folder.clone()
            },
        )?;
        if folder.position.is_some() {
            set_folder_position(store, &folder.id, folder.position)?;
        }
        counts.folders += 1;
    }
    let fallback = default_folder_id(store);
    for row in incoming {
        let stored = all_bookmarks(store)?;
        if stored.iter().any(|b| b.id == row.id) {
            continue;
        }
        let target = all_folders(store)?
            .into_iter()
            .find(|f| f.id == row.folder_id && !f.deleted)
            .map_or_else(|| fallback.clone(), |f| f.id);
        let updated_at = store.now_iso();
        let candidate = Bookmark {
            folder_id: target,
            stamp: None,
            deleted: false,
            dirty: false,
            rejected: false,
            updated_at,
            ..row.clone()
        };
        let twin = stored
            .iter()
            .any(|b| !b.deleted && live_key(b) == live_key(&candidate));
        if twin {
            continue;
        }
        put_bookmark(store, candidate)?;
        counts.bookmarks += 1;
    }
    Ok(counts)
}

// --- sync ---

fn capped_ids(store: &AppStore) -> HashSet<String> {
    store
        .sync_memory
        .capped
        .get(&SyncDomain::Bookmarks)
        .cloned()
        .unwrap_or_default()
}

/// Rows the next round sends: folders first, so a new bookmark finds its folder.
/// A capped row waits behind the others.
pub fn dirty_rows(store: &AppStore) -> Result<Vec<Value>, StoreError> {
    let capped = capped_ids(store);
    let order = |ids: Vec<(String, Value)>| -> Vec<Value> {
        let (later, now): (Vec<_>, Vec<_>) =
            ids.into_iter().partition(|(id, _)| capped.contains(id));
        now.into_iter().chain(later).map(|(_, v)| v).collect()
    };
    let folders = all_folders(store)?
        .into_iter()
        .filter(|f| f.dirty && !f.rejected)
        .map(|f| {
            let wire = json!({
                "kind": "folder.upsert", "id": f.id, "nameStamp": f.name_stamp, "name": f.name,
                "positionStamp": f.position_stamp, "position": f.position,
                "deletedStamp": f.deleted_stamp, "deleted": f.deleted, "updatedAt": f.updated_at,
            });
            (f.id, wire)
        })
        .collect();
    let marks = all_bookmarks(store)?
        .into_iter()
        .filter(|b| b.dirty && !b.rejected)
        .map(|b| {
            let wire = json!({
                "kind": "bookmark.upsert", "id": b.id, "stamp": b.stamp, "folderId": b.folder_id,
                "surah": b.surah, "ayah": b.ayah, "endSurah": b.end_surah, "endAyah": b.end_ayah,
                "sourceId": b.source_id, "deleted": b.deleted, "updatedAt": b.updated_at,
            });
            (b.id, wire)
        })
        .collect();
    let mut out = order(folders);
    out.extend(order(marks));
    out.truncate(SYNC_PAGE);
    Ok(out)
}

/// A folder tombstone from a pull: stamps and identity only.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderTombstone {
    id: String,
    name_stamp: Option<String>,
    position_stamp: Option<String>,
    deleted_stamp: Option<String>,
}

fn decode_tombstone(v: &Value) -> Option<FolderTombstone> {
    if !bool_true(v, "deleted") {
        return None;
    }
    Some(FolderTombstone {
        id: str_of(v, "id")?.to_owned(),
        name_stamp: opt_str(v, "nameStamp"),
        position_stamp: opt_str(v, "positionStamp"),
        deleted_stamp: opt_str(v, "deletedStamp"),
    })
}

fn apply_result(
    result: &Value,
    sent: Option<&Value>,
    capped: &mut HashSet<String>,
    folders: &mut BTreeMap<String, BookmarkFolder>,
    marks: &mut BTreeMap<String, Bookmark>,
) {
    let Some(sent) = sent else {
        return;
    };
    let Some(id) = str_of(result, "id") else {
        return;
    };
    let status = str_of(result, "status").unwrap_or_default();
    capped.remove(id);
    if status == "capped" {
        capped.insert(id.to_owned());
        return;
    }
    let stamps = result.get("stamps").filter(|s| s.is_object());
    let stamp = |k: &str| stamps.and_then(|s| str_of(s, k));
    if str_of(sent, "kind") == Some("bookmark.upsert") {
        let Some(mine) = marks.get_mut(id) else {
            return;
        };
        if mine.stamp.as_deref() != str_of(sent, "stamp") {
            return;
        }
        match (status, stamps) {
            ("rejected", _) => mine.rejected = true,
            (_, None) => {
                mine.deleted = true;
                mine.dirty = false;
            }
            (_, Some(_)) => {
                mine.stamp = lower_of(stamp("row"), &mine.stamp);
                mine.dirty = false;
                mine.rejected = false;
            }
        }
        return;
    }
    let Some(mine) = folders.get_mut(id) else {
        return;
    };
    let unchanged = mine.name_stamp.as_deref() == str_of(sent, "nameStamp")
        && mine.position_stamp.as_deref() == str_of(sent, "positionStamp")
        && mine.deleted_stamp.as_deref() == str_of(sent, "deletedStamp");
    if !unchanged {
        return;
    }
    match (status, stamps) {
        ("rejected", _) => mine.rejected = true,
        (_, None) => {
            mine.deleted = true;
            mine.dirty = false;
        }
        (_, Some(_)) => {
            mine.name_stamp = lower_of(stamp("name"), &mine.name_stamp);
            mine.position_stamp = lower_of(stamp("position"), &mine.position_stamp);
            mine.deleted_stamp = lower_of(stamp("deleted"), &mine.deleted_stamp);
            mine.dirty = false;
            mine.rejected = false;
        }
    }
}

/// Clean tombstones beyond the cap go, oldest first. Live and dirty rows never do.
fn trim_tombstones<T: Clone>(
    rows: Vec<T>,
    spare: impl Fn(&T) -> bool,
    updated: impl Fn(&T) -> String,
) -> Vec<T> {
    let mut tombstones: Vec<(String, usize)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| spare(r))
        .map(|(i, r)| (updated(r), i))
        .collect();
    if tombstones.len() <= TOMBSTONE_CAP {
        return rows;
    }
    tombstones.sort();
    let dropped: HashSet<usize> = tombstones[..tombstones.len() - TOMBSTONE_CAP]
        .iter()
        .map(|(_, i)| *i)
        .collect();
    rows.into_iter()
        .enumerate()
        .filter(|(i, _)| !dropped.contains(i))
        .map(|(_, r)| r)
        .collect()
}

/// Apply one round durably. `dirty` clears only for a row whose current
/// stamps are the ones sent. A row missing from a response means nothing:
/// only a tombstone deletes.
pub(crate) fn apply_round(
    store: &mut AppStore,
    round: &Value,
    sent: &[Value],
) -> Result<RoundApplied, StoreError> {
    let folders_raw = round.get("folders").and_then(Value::as_array);
    let marks_raw = round.get("bookmarks").and_then(Value::as_array);
    let (Some(folders_raw), Some(marks_raw)) = (folders_raw, marks_raw) else {
        return Err(StoreError::Unreadable(
            "bookmarks sync returned an unreadable round".into(),
        ));
    };
    store.writable()?;
    let meta = PageMeta::decode(round);
    if let Some(server) = meta.server_time {
        store.on_server_time(server);
    }
    let conn = store.owner_conn();
    let position = Position::load(conn, SyncDomain::Bookmarks)?;
    let verdict = PageVerdict::of(&position, &meta);
    let before_folders: Vec<BookmarkFolder> = load_all(conn)?;
    let before_marks: Vec<Bookmark> = load_all(conn)?;
    let mut folders: BTreeMap<String, BookmarkFolder> = before_folders
        .iter()
        .map(|f| (f.id.clone(), f.clone()))
        .collect();
    let mut marks: BTreeMap<String, Bookmark> = before_marks
        .iter()
        .map(|b| (b.id.clone(), b.clone()))
        .collect();

    let sent_by_id: HashMap<&str, &Value> = sent
        .iter()
        .filter_map(|s| Some((str_of(s, "id")?, s)))
        .collect();
    let mut capped = capped_ids(store);
    for result in round
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let sent_row = str_of(result, "id").and_then(|id| sent_by_id.get(id).copied());
        apply_result(result, sent_row, &mut capped, &mut folders, &mut marks);
    }
    store
        .sync_memory
        .capped
        .insert(SyncDomain::Bookmarks, capped);

    let mut seen = store
        .sync_memory
        .full_pull_seen
        .remove(&SyncDomain::Bookmarks);
    for row in folders_raw.iter().filter_map(decode_folder) {
        for stamp in [&row.name_stamp, &row.position_stamp, &row.deleted_stamp] {
            store.observe(stamp.as_deref());
        }
        if let Some(seen) = seen.as_mut() {
            seen.insert(row.id.clone());
        }
        let merged = match folders.get(&row.id) {
            Some(mine) => merge_folder_row(mine, &row),
            None => row,
        };
        folders.insert(merged.id.clone(), merged);
    }
    for tombstone in folders_raw.iter().filter_map(decode_tombstone) {
        store.observe(tombstone.deleted_stamp.as_deref());
        let Some(mine) = folders.get_mut(&tombstone.id) else {
            continue;
        };
        if compare_stamps(
            mine.deleted_stamp.as_deref(),
            tombstone.deleted_stamp.as_deref(),
        )
        .is_gt()
        {
            continue;
        }
        mine.deleted = true;
        mine.deleted_stamp = tombstone.deleted_stamp;
        mine.name_stamp = tombstone.name_stamp.or(mine.name_stamp.take());
        mine.position_stamp = tombstone.position_stamp.or(mine.position_stamp.take());
    }
    for row in marks_raw.iter().filter_map(decode_bookmark) {
        store.observe(row.stamp.as_deref());
        if let Some(seen) = seen.as_mut() {
            seen.insert(row.id.clone());
        }
        let merged = match marks.get(&row.id) {
            Some(mine) => merge_bookmark_row(mine, &row),
            None => row,
        };
        marks.insert(merged.id.clone(), merged);
    }

    if verdict.below_floor {
        seen = Some(HashSet::new());
    }
    if verdict.restored {
        // Rows and `rev` went backwards: push everything again, which heals what the restore lost.
        folders.values_mut().for_each(|f| f.dirty = true);
        marks.values_mut().for_each(|b| b.dirty = true);
    }
    if !verdict.reset() && !meta.more {
        if let Some(seen) = seen.take() {
            // Past the floor the server no longer holds tombstones for what is gone.
            for f in folders
                .values_mut()
                .filter(|f| !f.dirty && !seen.contains(&f.id))
            {
                f.deleted = true;
            }
            for b in marks
                .values_mut()
                .filter(|b| !b.dirty && !seen.contains(&b.id))
            {
                b.deleted = true;
            }
        }
    }
    if let Some(seen) = seen {
        store
            .sync_memory
            .full_pull_seen
            .insert(SyncDomain::Bookmarks, seen);
    }

    let after_folders = trim_tombstones(
        folders.into_values().collect(),
        |f: &BookmarkFolder| f.deleted && !f.dirty,
        |f| f.updated_at.clone(),
    );
    let after_marks = trim_tombstones(
        marks.into_values().collect(),
        |b: &Bookmark| b.deleted && !b.dirty,
        |b| b.updated_at.clone(),
    );
    let next = verdict.next(&position, &meta);
    let result = in_tx(store.owner_conn(), |tx| {
        save_diff(tx, &before_folders, &after_folders)?;
        save_diff(tx, &before_marks, &after_marks)?;
        // `since` is saved with the rows it covers, so an interrupted pull resumes.
        next.save(tx, SyncDomain::Bookmarks)
    });
    store.track(result)?;
    store.persist_clock()?;
    store.mark_seen()?;

    let progressed = round
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|r| matches!(str_of(r, "status"), Some("applied" | "superseded")));
    let more_dirty = sent.len() >= SYNC_PAGE && progressed && !dirty_rows(store)?.is_empty();
    Ok(RoundApplied {
        more: verdict.reset() || meta.more || more_dirty,
        settings: Vec::new(),
    })
}

/// Sign-in: device rows join the account; the device default maps onto the
/// account default; each row stamps from its own time so a newer edit
/// elsewhere wins. Returns how many bookmarks joined.
pub(crate) fn absorb_local(store: &mut AppStore) -> Result<usize, StoreError> {
    let local_folders: Vec<BookmarkFolder> = load_all(store.local_conn())?;
    let local_marks: Vec<Bookmark> = load_all(store.local_conn())?;
    if local_folders.is_empty() && local_marks.is_empty() {
        return Ok(0);
    }
    let local_default = default_folder_id_for(None);
    let account_default = default_folder_id(store);
    let at = |iso: &str| ms_from_iso(iso).unwrap_or(0);
    let mut incoming_folders = Vec::new();
    for folder in &local_folders {
        if folder.is_default {
            if folder.name != DEFAULT_FOLDER_NAME {
                // A renamed device default carries its name to the account default.
                let stamp = store.stamp_at(at(&folder.updated_at))?;
                incoming_folders.push(BookmarkFolder {
                    name: folder.name.clone(),
                    updated_at: folder.updated_at.clone(),
                    name_stamp: stamp,
                    dirty: true,
                    ..seed_default(account_default.clone())
                });
            }
            continue;
        }
        let stamp = store.stamp_at(at(&folder.updated_at))?;
        incoming_folders.push(BookmarkFolder {
            name_stamp: stamp.clone(),
            position_stamp: stamp,
            dirty: true,
            ..folder.clone()
        });
    }
    let mut incoming_marks = Vec::new();
    for row in &local_marks {
        let stamp = store.stamp_at(at(&row.updated_at))?;
        let folder_id = if row.folder_id == local_default {
            account_default.clone()
        } else {
            row.folder_id.clone()
        };
        incoming_marks.push(Bookmark {
            folder_id,
            stamp,
            dirty: true,
            ..row.clone()
        });
    }
    let conn = store.owner_conn();
    let before_folders: Vec<BookmarkFolder> = load_all(conn)?;
    let before_marks: Vec<Bookmark> = load_all(conn)?;
    let merge_into = |before: &[BookmarkFolder], incoming: Vec<BookmarkFolder>| {
        let mut by_id: BTreeMap<String, BookmarkFolder> =
            before.iter().map(|f| (f.id.clone(), f.clone())).collect();
        for row in incoming {
            let merged = match by_id.get(&row.id) {
                Some(mine) => merge_folder_row(mine, &row),
                None => row,
            };
            by_id.insert(merged.id.clone(), merged);
        }
        by_id.into_values().collect::<Vec<_>>()
    };
    let after_folders = merge_into(&before_folders, incoming_folders);
    let mut by_id: BTreeMap<String, Bookmark> = before_marks
        .iter()
        .map(|b| (b.id.clone(), b.clone()))
        .collect();
    for row in incoming_marks {
        let merged = match by_id.get(&row.id) {
            Some(mine) => merge_bookmark_row(mine, &row),
            None => row,
        };
        by_id.insert(merged.id.clone(), merged);
    }
    let after_marks: Vec<Bookmark> = by_id.into_values().collect();
    let result = in_tx(conn, |tx| {
        save_diff(tx, &before_folders, &after_folders)?;
        save_diff(tx, &before_marks, &after_marks)
    });
    store.track(result)?;
    let local = store.local_conn();
    let result = in_tx(local, |tx| {
        delete_domain(tx, BookmarkFolder::DOMAIN)?;
        delete_domain(tx, Bookmark::DOMAIN)?;
        super::rows::kv_remove(tx, LAST_FOLDER_KEY)
    });
    store.track(result)?;
    Ok(local_marks.len())
}

/// The wire form of a stored folder, for export (no stamps or flags).
pub(crate) fn folder_export(f: &BookmarkFolder) -> Value {
    json!({
        "id": f.id, "name": f.name, "isDefault": f.is_default, "position": opt_json(f.position),
        "createdAt": f.created_at, "updatedAt": f.updated_at,
    })
}

/// The wire form of a stored bookmark, for export (no stamp or flags).
pub(crate) fn bookmark_export(b: &Bookmark) -> Value {
    json!({
        "id": b.id, "folderId": b.folder_id, "surah": b.surah, "ayah": b.ayah,
        "endSurah": b.end_surah, "endAyah": b.end_ayah, "sourceId": b.source_id,
        "createdAt": b.created_at, "updatedAt": b.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::memory_store;

    #[test]
    fn default_folder_ids_match_the_server() {
        assert_eq!(
            default_folder_id_for(Some(1)),
            "9c314774-c392-5a9a-b569-79b8e7e31226"
        );
        assert_eq!(
            default_folder_id_for(None),
            "e520c7f8-36a1-5ef3-8a07-7a2c672d0d76"
        );
    }

    #[test]
    fn signed_out_folders_and_toggles() {
        let (mut store, _) = memory_store(1);
        assert_eq!(folders(&store).unwrap().len(), 1);
        let ToggleResult::Added { folder, .. } = toggle(&mut store, 2, 255, None).unwrap() else {
            panic!("expected add");
        };
        assert!(folder.is_default);
        let work = create_folder(&mut store, "  Work ").unwrap().unwrap();
        assert_eq!(work.name, "Work");
        set_in_folder(&mut store, 2, 255, &work.id, true, None).unwrap();
        assert_eq!(folders_of(&store, 2, 255).unwrap().len(), 2);
        // Taps go to the last folder used.
        assert_eq!(target_folder(&store).unwrap().id, work.id);
        let ToggleResult::Removed { bookmarks: gone } = toggle(&mut store, 2, 255, None).unwrap()
        else {
            panic!("expected remove");
        };
        assert_eq!(gone.len(), 2);
        assert!(!is_bookmarked(&store, 2, 255).unwrap());
        restore(&mut store, &gone).unwrap();
        assert_eq!(folders_of(&store, 2, 255).unwrap().len(), 2);
        // Deleting a folder takes its bookmarks; never the default.
        let deleted = delete_folder(&mut store, &work.id).unwrap().unwrap();
        assert_eq!(deleted.bookmarks.len(), 1);
        let default_id = default_folder_id(&store);
        assert!(delete_folder(&mut store, &default_id).unwrap().is_none());
        restore_folder(&mut store, &deleted).unwrap();
        assert_eq!(count_in(&store, &work.id).unwrap(), 1);
        // Moving onto a folder that already holds the verse collapses into one row.
        let default_row = rows_for(&store, 2, 255)
            .unwrap()
            .into_iter()
            .find(|b| b.folder_id != work.id)
            .unwrap();
        move_to_folder(&mut store, &default_row.id, &work.id).unwrap();
        assert_eq!(rows_for(&store, 2, 255).unwrap().len(), 1);
    }

    #[test]
    fn a_tombstone_beats_the_live_copy_at_equal_stamps_and_unknown_fields_survive() {
        let (mut store, _) = memory_store(2);
        store.sign_in(1, "dev").unwrap();
        let ToggleResult::Added { bookmark, .. } = toggle(&mut store, 1, 1, None).unwrap() else {
            panic!()
        };
        let mut record = bookmark.encode();
        record.row["futureField"] = json!({"x": 1});
        super::super::rows::put(store.owner_conn(), Bookmark::DOMAIN, &record).unwrap();
        let live = all_bookmarks(&store).unwrap().remove(0);
        assert_eq!(live.extra.get("futureField"), Some(&json!({"x": 1})));
        let dead = Bookmark {
            deleted: true,
            ..live.clone()
        };
        assert!(merge_bookmark_row(&live, &dead).deleted);
        assert!(merge_bookmark_row(&dead, &live).deleted);
        assert!(
            merge_bookmark_row(&live, &dead)
                .extra
                .contains_key("futureField")
        );
    }
}
