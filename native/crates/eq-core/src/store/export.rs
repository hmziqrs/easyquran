//! Personal data export and import (PARITY F6 data layer; web
//! `lib/storage/data-export.ts`). One versioned JSON file.
//!
//! Export leaves out device ids, sync stamps and `dirty`/`rejected` flags
//! (contracts/devices.md D5): those belong to this install. Import never
//! overwrites a row by id; it adds only ids this device lacks, through the
//! normal write paths, so the added rows get stamps and `dirty` like any
//! other write. Settings are a document applied whole by the app, which
//! keeps consent off. Notes live in the reader's store, so the caller hands
//! them in and takes them back.

use serde_json::{Value, json};

use super::{
    AppStore, StoreError, bookmarks,
    home::{self, CardCatalog},
    rows::{int_in, opt_str},
    sessions,
};
use crate::sync::clock::iso_from_ms;

pub const EXPORT_FORMAT: &str = "easyquran.export";
/// Highest export version this build reads. A newer file is refused.
pub const EXPORT_VERSION: i64 = 1;
const NOTE_TEXT_MAX: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportNote {
    pub surah: i64,
    pub ayah: i64,
    pub text: String,
    pub source_id: Option<String>,
    pub updated_at: Option<i64>,
}

impl ExportNote {
    fn to_json(&self) -> Value {
        json!({
            "surah": self.surah, "ayah": self.ayah, "text": self.text,
            "sourceId": self.source_id, "updatedAt": self.updated_at,
        })
    }
}

/// Builds the export file of everything this device holds for the current
/// owner. `settings` is the app's settings document; `notes` the reader's notes.
pub fn build_export(
    store: &AppStore,
    settings: &Value,
    notes: &[ExportNote],
    catalog: &dyn CardCatalog,
) -> Result<Value, StoreError> {
    let folders: Vec<Value> = bookmarks::folders(store)?
        .iter()
        .map(bookmarks::folder_export)
        .collect();
    let marks: Vec<Value> = bookmarks::bookmarks(store)?
        .iter()
        .map(bookmarks::bookmark_export)
        .collect();
    let threads: Vec<Value> = sessions::all(store)?
        .iter()
        .filter(|t| t.state != "deleted")
        .map(sessions::thread_export)
        .collect();
    Ok(json!({
        "format": EXPORT_FORMAT,
        "version": EXPORT_VERSION,
        "exportedAt": iso_from_ms(store.local_now()),
        "settings": settings,
        "folders": folders,
        "bookmarks": marks,
        "threads": threads,
        "notes": notes.iter().map(ExportNote::to_json).collect::<Vec<_>>(),
        "cards": home::cards(store, catalog)?,
    }))
}

/// Why a file was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseFailure {
    NotExport,
    Malformed,
    FutureVersion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedImport {
    pub settings: Value,
    pub folders: Vec<bookmarks::BookmarkFolder>,
    pub bookmarks: Vec<bookmarks::Bookmark>,
    pub threads: Vec<sessions::ReadingThread>,
    pub notes: Vec<ExportNote>,
    pub cards: Vec<Value>,
    /// Rows that failed their decoder and were dropped.
    pub invalid: usize,
}

fn decode_note(v: &Value) -> Option<ExportNote> {
    let text = v
        .get("text")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty() && t.chars().count() <= NOTE_TEXT_MAX)?;
    Some(ExportNote {
        surah: int_in(v, "surah", 1, 114)?,
        ayah: int_in(v, "ayah", 1, 286)?,
        text: text.to_owned(),
        source_id: opt_str(v, "sourceId"),
        updated_at: int_in(v, "updatedAt", 0, i64::MAX >> 11),
    })
}

/// Structure first (format, version, arrays), then each row. Bad rows drop;
/// a bad file is refused.
pub fn parse_export(
    raw: &Value,
    catalog: &dyn CardCatalog,
    now_ms: i64,
) -> Result<ParsedImport, ParseFailure> {
    if !raw.is_object() {
        return Err(ParseFailure::Malformed);
    }
    if raw.get("format").and_then(Value::as_str) != Some(EXPORT_FORMAT) {
        return Err(ParseFailure::NotExport);
    }
    let version = raw
        .get("version")
        .and_then(Value::as_i64)
        .filter(|v| *v >= 1)
        .ok_or(ParseFailure::Malformed)?;
    if version > EXPORT_VERSION {
        return Err(ParseFailure::FutureVersion);
    }
    let settings = raw.get("settings").filter(|s| s.is_object()).cloned();
    let list = |k: &str| raw.get(k).and_then(Value::as_array);
    let (Some(settings), Some(f), Some(b), Some(t), Some(n), Some(c)) = (
        settings,
        list("folders"),
        list("bookmarks"),
        list("threads"),
        list("notes"),
        list("cards"),
    ) else {
        return Err(ParseFailure::Malformed);
    };
    let folders: Vec<_> = f.iter().filter_map(bookmarks::decode_folder).collect();
    let marks: Vec<_> = b.iter().filter_map(bookmarks::decode_bookmark).collect();
    let threads: Vec<_> = t
        .iter()
        .filter_map(sessions::decode_thread)
        .filter(|t| t.state != "deleted")
        .collect();
    let notes: Vec<_> = n.iter().filter_map(decode_note).collect();
    let cards = home::decode_cards(
        catalog,
        &json!({ "v": home::CARDS_SCHEMA_VERSION, "cards": c }),
        now_ms,
    );
    let invalid = (f.len() - folders.len())
        + (b.len() - marks.len())
        + (t.len() - threads.len())
        + (n.len() - notes.len())
        + c.len().saturating_sub(cards.len());
    Ok(ParsedImport {
        settings,
        folders,
        bookmarks: marks,
        threads,
        notes,
        cards,
        invalid,
    })
}

/// Totals for the toast.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    pub skipped: usize,
    pub invalid: usize,
}

/// The reader's notes, which the import writes through.
pub trait NotesPort {
    /// The note on a verse, empty when none.
    fn note(&self, surah: i64, ayah: i64) -> String;
    fn set_note(&mut self, surah: i64, ayah: i64, text: &str, source_id: Option<&str>);
}

/// Adds what this device lacks. The caller applies `parsed.settings` itself
/// (consent forced off) before or after this call.
pub fn apply_import(
    store: &mut AppStore,
    parsed: &ParsedImport,
    notes: &mut dyn NotesPort,
    catalog: &dyn CardCatalog,
) -> Result<ImportSummary, StoreError> {
    let from_bookmarks = bookmarks::import_rows(store, &parsed.folders, &parsed.bookmarks)?;
    let threads = sessions::import_threads(store, &parsed.threads)?;
    let mut added_notes = 0;
    for note in &parsed.notes {
        if !notes.note(note.surah, note.ayah).is_empty() {
            continue;
        }
        notes.set_note(note.surah, note.ayah, &note.text, note.source_id.as_deref());
        added_notes += 1;
    }
    let mut added_cards = 0;
    let known: Vec<String> = home::cards(store, catalog)?
        .iter()
        .filter_map(|c| c.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect();
    for card in &parsed.cards {
        let id = card.get("id").and_then(Value::as_str).unwrap_or_default();
        if known.iter().any(|k| k == id) {
            continue;
        }
        if !home::add_card(store, catalog, card)? {
            break;
        }
        added_cards += 1;
    }
    let offered = parsed.folders.len()
        + parsed.bookmarks.len()
        + parsed.threads.len()
        + parsed.notes.len()
        + parsed.cards.len();
    let added =
        from_bookmarks.folders + from_bookmarks.bookmarks + threads + added_notes + added_cards;
    Ok(ImportSummary {
        added,
        skipped: offered.saturating_sub(added),
        invalid: parsed.invalid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::memory_store;
    use std::collections::HashMap;

    struct All;
    impl CardCatalog for All {
        fn known_source(&self, _: &str) -> bool {
            true
        }
        fn known_snippet(&self, _: &str) -> bool {
            true
        }
    }

    #[derive(Default)]
    struct Notes(HashMap<(i64, i64), String>);
    impl NotesPort for Notes {
        fn note(&self, s: i64, a: i64) -> String {
            self.0.get(&(s, a)).cloned().unwrap_or_default()
        }
        fn set_note(&mut self, s: i64, a: i64, text: &str, _: Option<&str>) {
            self.0.insert((s, a), text.into());
        }
    }

    #[test]
    fn export_drops_device_ids_and_stamps_and_import_adds_only_new_ids() {
        let (mut a, _) = memory_store(1);
        a.sign_in(1, "device-a").unwrap();
        bookmarks::toggle(&mut a, 2, 255, Some("en.sahih".into())).unwrap();
        let target = sessions::ReaderTarget {
            scope: "surah:2".into(),
            surah: 2,
            ayah: 1,
            exact: false,
            bound_end: None,
            source_id: None,
        };
        sessions::enter(&mut a, &target, None, false).unwrap();
        sessions::note_read(&mut a, 2, 5).unwrap();
        let notes = [ExportNote {
            surah: 1,
            ayah: 1,
            text: "n".into(),
            source_id: None,
            updated_at: None,
        }];
        let file = build_export(&a, &json!({"v": 1}), &notes, &All).unwrap();
        let text = file.to_string();
        assert!(!text.contains("device-a"));
        assert!(!text.contains("Stamp"));
        assert!(!text.contains("\"stamp\""));
        assert!(!text.contains("dirty"));

        let (mut b, _) = memory_store(2);
        b.sign_in(2, "device-b").unwrap();
        let parsed = parse_export(&file, &All, 0).unwrap();
        let mut sink = Notes::default();
        let summary = apply_import(&mut b, &parsed, &mut sink, &All).unwrap();
        assert_eq!(
            summary.added, 3,
            "default folder skipped; bookmark, thread, note added"
        );
        let thread = sessions::all(&b).unwrap().remove(0);
        assert_eq!(thread.device_id.as_deref(), Some("device-b"));
        assert!(thread.dirty);
        // A second import adds nothing.
        let again = apply_import(&mut b, &parsed, &mut sink, &All).unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(
            parse_export(&json!({"format": "x"}), &All, 0),
            Err(ParseFailure::NotExport)
        );
        assert_eq!(
            parse_export(&json!({"format": EXPORT_FORMAT, "version": 2}), &All, 0),
            Err(ParseFailure::FutureVersion)
        );
    }
}
