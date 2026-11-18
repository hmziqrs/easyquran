//! Notes (PARITY F2): device-owned verse annotations, never synced, never
//! sent. Mirrors the web's `annotations.svelte.ts` + the notes half of
//! `reader-persistence.svelte.ts` (EC-17):
//!
//! - One envelope per owner under `easyquran.notes.{owner}` (owner = the
//!   signed-in user id, else `local`), shaped
//!   `{ "v": 1, "notes": { "S:A": text }, "meta": { "S:A": { "updatedAt": ms, "sourceId": id|null } } }`,
//!   so a data export and the web's storage read the same thing.
//! - The owner is unknown until auth settles: notes written before then
//!   are merged into the first owner's envelope; after that an owner
//!   switch parks the outgoing notes and loads the incoming owner's (never
//!   merged across owners).
//! - Pages call [`NotesStore::sync_owner`] from an effect that reads the
//!   auth phase (the web's `startNotesOwnerSync`); every write also syncs.
//!
//! Storage is [`NshKv`] (in memory after first load, written off the
//! engine thread by the NSH backend), so reads and writes here are cheap.

use std::collections::BTreeMap;

use eq_core::{
    api::auth_state::AuthPhase,
    kv::KeyValue,
    store::export::{ExportNote, NotesPort},
};
use nsh_core::prelude::*;
use serde_json::{Map, Value, json};

use crate::services::storage::NshKv;

pub const NOTES_SCHEMA_VERSION: i64 = 1;
/// The owner of notes written while signed out.
pub const LOCAL_OWNER: &str = "local";

/// The storage key of `owner`'s notes.
pub fn notes_key_for(owner: &str) -> String {
    format!("easyquran.notes.{owner}")
}

/// `S:A` for a verse.
pub fn verse_key(surah: i64, ayah: i64) -> String {
    format!("{surah}:{ayah}")
}

/// `S:A` -> coordinates inside the Quran's bounds (`parseVerseKey`).
pub fn parse_verse_key(key: &str) -> Option<(i64, i64)> {
    eq_core::store::sessions::parse_verse_key(key)
}

/// Per-note context: when it was last written, in which translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMeta {
    /// Epoch ms.
    pub updated_at: i64,
    /// `None`: written in the Arabic reader.
    pub source_id: Option<String>,
}

/// One owner's notes, keyed by `S:A`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NotesBook {
    pub notes: BTreeMap<String, String>,
    pub meta: BTreeMap<String, NoteMeta>,
}

/// One saved note with its coordinates (the library's row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEntry {
    pub key: String,
    pub surah: i64,
    pub ayah: i64,
    pub text: String,
    /// `None` for notes written before timestamps existed.
    pub updated_at: Option<i64>,
    pub source_id: Option<String>,
}

/// What a removed note held, so Undo puts it back exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSnapshot {
    pub text: String,
    pub meta: Option<NoteMeta>,
}

fn decode_meta(raw: &Value) -> Option<NoteMeta> {
    let entry = raw.as_object()?;
    let updated_at = entry.get("updatedAt")?.as_f64()?;
    if !(0.0..=9_007_199_254_740_991.0).contains(&updated_at) {
        return None;
    }
    Some(NoteMeta {
        updated_at: updated_at as i64,
        source_id: entry
            .get("sourceId")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// The envelope (`decodeNotes`); `None` for anything but a v1 object.
pub fn decode_notes(raw: &str) -> Option<NotesBook> {
    let stored: Value = serde_json::from_str(raw).ok()?;
    let stored = stored.as_object()?;
    if stored.get("v").and_then(Value::as_i64) != Some(NOTES_SCHEMA_VERSION) {
        return None;
    }
    let notes = stored
        .get("notes")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_str().map(|t| (k.clone(), t.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    let meta = stored
        .get("meta")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| decode_meta(v).map(|m| (k.clone(), m)))
                .collect()
        })
        .unwrap_or_default();
    Some(NotesBook { notes, meta })
}

/// The envelope as stored.
pub fn encode_notes(book: &NotesBook) -> String {
    let notes: Map<String, Value> = book
        .notes
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    let meta: Map<String, Value> = book
        .meta
        .iter()
        .map(|(k, m)| {
            (
                k.clone(),
                json!({ "updatedAt": m.updated_at, "sourceId": m.source_id }),
            )
        })
        .collect();
    json!({ "v": NOTES_SCHEMA_VERSION, "notes": notes, "meta": meta }).to_string()
}

impl NotesBook {
    /// Every note with text, in mushaf order.
    pub fn entries(&self) -> Vec<NoteEntry> {
        let mut out: Vec<NoteEntry> = self
            .notes
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .filter_map(|(key, text)| {
                let (surah, ayah) = parse_verse_key(key)?;
                let meta = self.meta.get(key);
                Some(NoteEntry {
                    key: key.clone(),
                    surah,
                    ayah,
                    text: text.clone(),
                    updated_at: meta.map(|m| m.updated_at),
                    source_id: meta.and_then(|m| m.source_id.clone()),
                })
            })
            .collect();
        out.sort_by(|a, b| (a.surah, a.ayah).cmp(&(b.surah, b.ayah)));
        out
    }

    /// Writes one note; empty text removes it entirely.
    fn commit(&mut self, key: &str, text: &str, meta: Option<NoteMeta>, now: i64) {
        if text.is_empty() {
            self.notes.remove(key);
            self.meta.remove(key);
            return;
        }
        self.notes.insert(key.to_owned(), text.to_owned());
        self.meta.insert(
            key.to_owned(),
            meta.unwrap_or(NoteMeta {
                updated_at: now,
                source_id: None,
            }),
        );
    }

    /// The export rows (`ExportNote`), mushaf order.
    pub fn export(&self) -> Vec<ExportNote> {
        self.entries()
            .into_iter()
            .map(|e| ExportNote {
                surah: e.surah,
                ayah: e.ayah,
                text: e.text,
                source_id: e.source_id,
                updated_at: e.updated_at,
            })
            .collect()
    }
}

fn now_ms() -> i64 {
    nsh_core::time::unix_ms() as i64
}

/// One owner's notes on a key-value store, for the data import
/// (`eq_core::store::export::apply_import`): each `set_note` writes through.
pub struct NotesFile<'a> {
    kv: &'a dyn KeyValue,
    owner: String,
    book: NotesBook,
}

impl<'a> NotesFile<'a> {
    pub fn open(kv: &'a dyn KeyValue, owner: &str) -> Self {
        let book = kv
            .get(&notes_key_for(owner))
            .and_then(|raw| decode_notes(&raw))
            .unwrap_or_default();
        Self {
            kv,
            owner: owner.to_owned(),
            book,
        }
    }

    pub fn book(&self) -> &NotesBook {
        &self.book
    }
}

impl NotesPort for NotesFile<'_> {
    fn note(&self, surah: i64, ayah: i64) -> String {
        self.book
            .notes
            .get(&verse_key(surah, ayah))
            .cloned()
            .unwrap_or_default()
    }

    fn set_note(&mut self, surah: i64, ayah: i64, text: &str, source_id: Option<&str>) {
        let key = verse_key(surah, ayah);
        let meta = NoteMeta {
            updated_at: now_ms(),
            source_id: source_id.map(str::to_owned),
        };
        self.book.commit(&key, text, Some(meta), now_ms());
        // Full storage keeps the in-memory book, as the web does.
        let _ = self
            .kv
            .set(&notes_key_for(&self.owner), &encode_notes(&self.book));
        // The live store re-reads on its next owner sync.
        notes_store().reload_if_owner(&self.owner);
    }
}

/// The live notes of this surface.
#[derive(Default, Clone)]
struct NotesState {
    /// `None` until auth settles.
    owner: Option<String>,
    book: NotesBook,
}

fn state() -> Signal<NotesState> {
    global::<NotesState>()
}

/// The owner a phase resolves to; `None` while auth is unresolved.
pub fn owner_for(phase: &AuthPhase) -> Option<String> {
    match phase {
        AuthPhase::Unknown => None,
        _ => Some(
            phase
                .user()
                .map_or_else(|| LOCAL_OWNER.to_owned(), |u| u.id.to_string()),
        ),
    }
}

/// The notes store handle (a surface-wide global; no context needed).
#[derive(Clone, Copy)]
pub struct NotesStore;

/// The notes store.
pub fn notes_store() -> NotesStore {
    NotesStore
}

impl NotesStore {
    fn load(owner: &str) -> NotesBook {
        NshKv
            .get(&notes_key_for(owner))
            .and_then(|raw| decode_notes(&raw))
            .unwrap_or_default()
    }

    fn persist(st: &NotesState) {
        if let Some(owner) = &st.owner {
            let _ = NshKv.set(&notes_key_for(owner), &encode_notes(&st.book));
        }
    }

    /// Re-resolves the owner from `phase` (call from an effect that reads
    /// the auth phase). Pre-auth notes merge into the first owner; an owner
    /// switch parks the outgoing notes and loads the incoming owner's.
    pub fn sync_owner(self, phase: &AuthPhase) {
        let Some(next) = owner_for(phase) else {
            return;
        };
        let current = state().with_untracked(|s| s.owner.clone());
        if current.as_deref() == Some(next.as_str()) {
            return;
        }
        state().update(|st| {
            if st.owner.is_none() {
                let pre_auth = std::mem::take(&mut st.book);
                st.owner = Some(next.clone());
                st.book = Self::load(&next);
                if !pre_auth.notes.is_empty() {
                    st.book.notes.extend(pre_auth.notes);
                    st.book.meta.extend(pre_auth.meta);
                    Self::persist(st);
                }
                return;
            }
            Self::persist(st);
            st.owner = Some(next.clone());
            st.book = Self::load(&next);
        });
    }

    fn reload_if_owner(self, owner: &str) {
        if state().with_untracked(|s| s.owner.as_deref() == Some(owner)) {
            state().update(|st| st.book = Self::load(owner));
        }
    }

    /// The note on a verse, `""` when none (tracked).
    pub fn note(self, surah: i64, ayah: i64) -> String {
        state().with(|s| {
            s.book
                .notes
                .get(&verse_key(surah, ayah))
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Whether the verse has a non-blank note (tracked).
    pub fn has_note(self, surah: i64, ayah: i64) -> bool {
        !self.note(surah, ayah).trim().is_empty()
    }

    /// The note's context (tracked).
    pub fn meta(self, surah: i64, ayah: i64) -> Option<NoteMeta> {
        state().with(|s| s.book.meta.get(&verse_key(surah, ayah)).cloned())
    }

    /// Every note with text, mushaf order (tracked).
    pub fn entries(self) -> Vec<NoteEntry> {
        state().with(|s| s.book.entries())
    }

    /// How many notes there are (tracked).
    pub fn count(self) -> usize {
        state().with(|s| s.book.notes.values().filter(|t| !t.is_empty()).count())
    }

    /// The export rows of the current owner (untracked).
    pub fn export(self) -> Vec<ExportNote> {
        state().with_untracked(|s| s.book.export())
    }

    /// Saves the note text; `source_id` is the translation being read
    /// (`None`: Arabic). Empty text removes the note.
    pub fn set_note(self, surah: i64, ayah: i64, text: &str, source_id: Option<String>) {
        let key = verse_key(surah, ayah);
        let meta = (!text.is_empty()).then(|| NoteMeta {
            updated_at: now_ms(),
            source_id,
        });
        state().update(|st| {
            st.book.commit(&key, text, meta, now_ms());
            Self::persist(st);
        });
    }

    /// Removes a note and returns what it held, for Undo.
    pub fn remove_note(self, surah: i64, ayah: i64) -> Option<NoteSnapshot> {
        let key = verse_key(surah, ayah);
        let snapshot = state().with_untracked(|s| {
            s.book.notes.get(&key).map(|text| NoteSnapshot {
                text: text.clone(),
                meta: s.book.meta.get(&key).cloned(),
            })
        })?;
        state().update(|st| {
            st.book.commit(&key, "", None, now_ms());
            Self::persist(st);
        });
        Some(snapshot)
    }

    /// Puts back a removed note with its original timestamp and source.
    pub fn restore_note(self, surah: i64, ayah: i64, snapshot: NoteSnapshot) {
        let key = verse_key(surah, ayah);
        state().update(|st| {
            st.book
                .commit(&key, &snapshot.text, snapshot.meta.clone(), now_ms());
            Self::persist(st);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_in_the_web_shape() {
        let raw = r#"{"v":1,"notes":{"2:255":"Throne","x":"bad"},"meta":{"2:255":{"updatedAt":5,"sourceId":"en.sahih"},"1:1":{"updatedAt":"no"}}}"#;
        let book = decode_notes(raw).unwrap();
        assert_eq!(book.notes.get("2:255").map(String::as_str), Some("Throne"));
        assert_eq!(book.meta.len(), 1);
        let entries = book.entries();
        assert_eq!(entries.len(), 1, "malformed keys are not listed");
        assert_eq!(entries[0].source_id.as_deref(), Some("en.sahih"));
        let again = decode_notes(&encode_notes(&book)).unwrap();
        assert_eq!(again, book);
        assert!(decode_notes(r#"{"v":2,"notes":{}}"#).is_none());
    }
}
